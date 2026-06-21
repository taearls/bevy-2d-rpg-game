//! Minimal in-window entity inspector — click a combatant sprite to select it,
//! then nudge its game stats live with the keyboard. Compiled in only under the
//! `debug-overlay` cargo feature (it lives in the `debug` module), so default,
//! release, and wasm builds never include it.
//!
//! Bevy's official `bevy_dev_tools` is read-only (FPS, UI outlines) — it has no
//! entity inspector. The reflection-driven editable tree only ever existed in the
//! community `bevy-inspector-egui` crate, which this project dropped during the
//! Bevy 0.19 migration. This is a deliberately small, egui-free replacement:
//! it edits a fixed allow-list of **gameplay** stats (never Bevy internals like
//! `Transform`/`Sprite`), which is exactly what's useful for balancing a battle
//! (e.g. drop an enemy's HP to 1 to test the victory path).
//!
//! ## Controls
//! - **Click** a sprite (player or enemy — picking is already enabled on enemies)
//!   to select it. The panel appears top-right.
//! - **Tab** / **Shift+Tab** — move the field cursor down / up.
//! - **`=`/`+`** and **Up**, **`-`** and **Down** — increment / decrement the
//!   focused field by its step (×10 while **Shift** is held).
//! - **Esc** — clear the selection (hide the panel).

use std::fmt::Write as _;

use bevy::prelude::*;

use crate::components::{CombatStats, DamageVariance, DisplayName, Enemy, Health, Player};

/// The entity the inspector is currently editing, plus which field row is
/// focused. `None` while nothing is selected (the panel is hidden).
#[derive(Resource, Default)]
struct Inspected {
    entity: Option<Entity>,
    /// Cursor into [`EDITABLE_FIELDS`]; clamped whenever the selection changes.
    field: usize,
}

/// One editable numeric stat: how to label it, read it, and write it back. Keeps
/// the editor a flat table over a *fixed* set of gameplay fields rather than a
/// generic reflection walk — the component vocabulary is small and stable, so a
/// typed table is both shorter and safe (it can't reach engine internals).
struct Field {
    label: &'static str,
    /// Read the current value as `f32` (uniform for `i32` and `f32` stats), or
    /// `None` if the selected entity lacks the owning component.
    get: fn(&Stats) -> Option<f32>,
    /// Apply a delta, clamping to the field's own invariants. No-op if the
    /// component is absent.
    add: fn(&mut StatsMut, f32),
    /// Step applied per keypress (×10 with Shift). Whole numbers for int stats,
    /// fractional for the variance spread.
    step: f32,
}

/// Borrowed, read-only view of the editable components on the inspected entity,
/// built from a plain query so the [`Field`] closures stay free of Bevy's
/// generated query-item lifetimes.
struct Stats<'a> {
    health: Option<&'a Health>,
    combat: Option<&'a CombatStats>,
    variance: Option<&'a DamageVariance>,
}

/// Mutable counterpart used when applying a delta.
struct StatsMut<'a> {
    health: Option<&'a mut Health>,
    combat: Option<&'a mut CombatStats>,
    variance: Option<&'a mut DamageVariance>,
}

/// The fixed, allow-listed set of editable fields, shown top-to-bottom. Order is
/// the Tab-cycle order. Health is clamped to `0..=max`; stats stay non-negative;
/// variance keeps `min <= max` and both non-negative.
const EDITABLE_FIELDS: &[Field] = &[
    Field {
        label: "Health.current",
        get: |i| i.health.map(|h| h.current as f32),
        add: |i, d| {
            if let Some(h) = i.health.as_deref_mut() {
                h.current = (h.current + d as i32).clamp(0, h.max);
            }
        },
        step: 1.0,
    },
    Field {
        label: "Health.max",
        get: |i| i.health.map(|h| h.max as f32),
        add: |i, d| {
            if let Some(h) = i.health.as_deref_mut() {
                h.max = (h.max + d as i32).max(1);
                h.current = h.current.min(h.max);
            }
        },
        step: 1.0,
    },
    Field {
        label: "CombatStats.attack",
        get: |i| i.combat.map(|s| s.attack as f32),
        add: |i, d| {
            if let Some(s) = i.combat.as_deref_mut() {
                s.attack = (s.attack + d as i32).max(0);
            }
        },
        step: 1.0,
    },
    Field {
        label: "CombatStats.defense",
        get: |i| i.combat.map(|s| s.defense as f32),
        add: |i, d| {
            if let Some(s) = i.combat.as_deref_mut() {
                s.defense = (s.defense + d as i32).max(0);
            }
        },
        step: 1.0,
    },
    Field {
        label: "DamageVariance.min",
        get: |i| i.variance.map(|v| v.min),
        add: |i, d| {
            if let Some(v) = i.variance.as_deref_mut() {
                v.min = (v.min + d).clamp(0.0, v.max);
            }
        },
        step: 0.05,
    },
    Field {
        label: "DamageVariance.max",
        get: |i| i.variance.map(|v| v.max),
        add: |i, d| {
            if let Some(v) = i.variance.as_deref_mut() {
                v.max = (v.max + d).max(v.min);
            }
        },
        step: 0.05,
    },
];

/// Marker for the inspector's text panel node, so the redraw system can target it.
#[derive(Component)]
struct InspectorPanel;

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<Inspected>()
        .add_observer(on_click_select)
        .add_systems(Startup, spawn_panel)
        .add_systems(
            Update,
            (
                make_combatants_pickable,
                clear_on_escape,
                edit_focused_field,
                redraw_panel,
            ),
        );
}

/// Make every combatant clickable for the inspector. Enemies already carry
/// [`Pickable`] (for battle targeting), but the player sprite does not — so
/// without this it could never be selected. Adding it here, gated behind the
/// debug feature, keeps the player pickable a debug-only concern and leaves the
/// gameplay spawn code untouched. Runs each frame but only touches the (few)
/// freshly-spawned combatants that still lack the component.
fn make_combatants_pickable(
    mut commands: Commands,
    newly: Query<Entity, (With<Health>, Without<Pickable>)>,
) {
    for entity in &newly {
        commands.entity(entity).insert(Pickable::default());
    }
}

/// Global picking observer: clicking an entity that carries any editable stat
/// component selects it for inspection and resets the field cursor.
///
/// `Pointer<Click>` is an auto-propagating `EntityEvent`, so a click on an enemy
/// sprite fires this observer once for the sprite *and* again for each ancestor
/// it bubbles to (ultimately the camera). We accept only the hit on an entity
/// that actually has inspectable stats and ignore the propagated ancestor hits —
/// otherwise the non-combatant ancestor would clobber the real selection. This
/// also scopes selection to combatants, matching the editor's stats-only remit.
///
/// Additive to the battle targeting observer on enemies — both fire; targeting
/// still only acts during its own phase, while the inspector records the
/// selection regardless of phase.
fn on_click_select(
    click: On<Pointer<Click>>,
    inspectable: Query<(), Or<(With<Health>, With<CombatStats>, With<DamageVariance>)>>,
    mut inspected: ResMut<Inspected>,
) {
    if inspectable.get(click.entity).is_err() {
        return; // propagated ancestor hit (e.g. camera) — not an inspectable entity
    }
    inspected.entity = Some(click.entity);
    inspected.field = 0;
}

/// Esc clears the selection, hiding the panel.
fn clear_on_escape(keys: Res<ButtonInput<KeyCode>>, mut inspected: ResMut<Inspected>) {
    if keys.just_pressed(KeyCode::Escape) {
        inspected.entity = None;
    }
}

/// Tab moves the field cursor; +/-/arrows nudge the focused field's value on the
/// inspected entity. All edits go through the [`Field`] table's clamping setters.
#[allow(clippy::type_complexity)]
fn edit_focused_field(
    keys: Res<ButtonInput<KeyCode>>,
    mut inspected: ResMut<Inspected>,
    mut query: Query<(
        Option<&mut Health>,
        Option<&mut CombatStats>,
        Option<&mut DamageVariance>,
    )>,
) {
    let Some(entity) = inspected.entity else {
        return;
    };

    // Field cursor: Tab forward, Shift+Tab back, wrapping.
    let shift = keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
    if keys.just_pressed(KeyCode::Tab) {
        let n = EDITABLE_FIELDS.len();
        inspected.field = if shift {
            (inspected.field + n - 1) % n
        } else {
            (inspected.field + 1) % n
        };
    }

    let dir = if keys.any_just_pressed([KeyCode::Equal, KeyCode::NumpadAdd, KeyCode::ArrowUp]) {
        1.0
    } else if keys.any_just_pressed([KeyCode::Minus, KeyCode::NumpadSubtract, KeyCode::ArrowDown]) {
        -1.0
    } else {
        return;
    };

    let Ok((health, combat, variance)) = query.get_mut(entity) else {
        return; // selected entity was despawned (e.g. enemy died)
    };
    let mut item = StatsMut {
        health: health.map(Mut::into_inner),
        combat: combat.map(Mut::into_inner),
        variance: variance.map(Mut::into_inner),
    };
    let field = &EDITABLE_FIELDS[inspected.field];
    let magnitude = if shift { 10.0 } else { 1.0 };
    (field.add)(&mut item, dir * field.step * magnitude);
}

/// Top-right text panel, spawned once and shown/hidden by [`redraw_panel`].
fn spawn_panel(mut commands: Commands) {
    commands.spawn((
        InspectorPanel,
        Text::default(),
        TextFont::from_font_size(16.0),
        TextColor(Color::srgb(0.9, 0.9, 0.6)),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(8.0),
            right: Val::Px(8.0),
            ..default()
        },
        GlobalZIndex(i32::MAX - 16),
        Visibility::Hidden,
    ));
}

/// Rebuild the panel text from the inspected entity each frame (cheap; the string
/// is tiny). Hidden when nothing is selected or the selection has despawned.
#[allow(clippy::too_many_arguments)]
fn redraw_panel(
    inspected: Res<Inspected>,
    items: Query<(
        Option<&Health>,
        Option<&CombatStats>,
        Option<&DamageVariance>,
    )>,
    names: Query<&DisplayName>,
    players: Query<(), With<Player>>,
    enemies: Query<&Enemy>,
    mut panel: Single<(&mut Text, &mut Visibility), With<InspectorPanel>>,
) {
    let (text, visibility) = &mut *panel;

    let Some(entity) = inspected.entity else {
        **visibility = Visibility::Hidden;
        return;
    };
    let Ok((health, combat, variance)) = items.get(entity) else {
        **visibility = Visibility::Hidden;
        return;
    };
    let item = Stats {
        health,
        combat,
        variance,
    };
    **visibility = Visibility::Inherited;

    let kind = if players.get(entity).is_ok() {
        "Player".to_string()
    } else if let Ok(e) = enemies.get(entity) {
        format!("Enemy #{}", e.index)
    } else {
        "Entity".to_string()
    };
    let name = names
        .get(entity)
        .map_or_else(|_| String::new(), |n| format!(" \"{}\"", n.0));

    let mut out = format!("[{kind}{name}]  {entity}\n");
    for (row, field) in EDITABLE_FIELDS.iter().enumerate() {
        let cursor = if row == inspected.field { ">" } else { " " };
        match (field.get)(&item) {
            // Whole numbers print without a decimal; the variance spread keeps two.
            Some(v) if field.step.fract() == 0.0 => {
                let _ = writeln!(out, "{cursor} {}: {v:.0}", field.label);
            }
            Some(v) => {
                let _ = writeln!(out, "{cursor} {}: {v:.2}", field.label);
            }
            None => {
                let _ = writeln!(out, "{cursor} {}: --", field.label);
            }
        }
    }
    out.push_str("\nTab cycle · +/- edit · Shift ×10 · Esc close");

    ***text = out;
}
