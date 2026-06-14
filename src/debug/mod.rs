//! Debug inspector — a right-click-to-inspect overlay, compiled in only under
//! the `debug-inspector` cargo feature.
//!
//! Instead of dumping the whole entity tree, this wires `bevy_egui`'s
//! `EguiPlugin` plus a small picking layer: right-click (or Control+left-click on
//! a trackpad) any sprite in the viewport and an egui window shows just that
//! entity's components (via `bevy-inspector-egui`'s `ui_for_entity`). The panel is
//! sticky — it stays on the last-clicked entity until you click another or press
//! Escape. Every Godot `[Export(Range)]` tuning knob (`BattleLayout`, `UiConfig`,
//! `DamageVariance`, `Health`, `CombatStats`, …) is registered for reflection by
//! its owning plugin, so the per-entity panel can edit those values live.
//!
//! The whole module is `#[cfg(feature = "debug-inspector")]` at the call site in
//! [`GamePlugin`](crate::game::GamePlugin), so a default `cargo build` compiles
//! egui out entirely — tests and release binaries never pull it in.

use bevy::prelude::*;
use bevy::render::RenderApp;
use bevy_inspector_egui::bevy_egui::{
    EguiContext, EguiPlugin, EguiPrimaryContextPass, PrimaryEguiContext,
};
use bevy_inspector_egui::bevy_inspector;

/// The entity whose component inspector is currently shown, or `None` when the
/// panel is dismissed. Set by a right-click (or Control+left-click) on a sprite,
/// cleared by Escape or the window's close button.
#[derive(Resource, Debug, Default)]
pub struct InspectedEntity(pub Option<Entity>);

/// Marker for sprites we've already made pickable, so [`arm_sprite_picking`]
/// doesn't re-insert `Pickable` every frame.
#[derive(Component, Debug)]
struct InspectArmed;

/// Adds the right-click-to-inspect overlay.
///
/// Only added under the `debug-inspector` feature; see the module docs. Pulls in
/// `EguiPlugin`, arms every sprite for picking, routes right-clicks into
/// [`InspectedEntity`], and renders the per-entity panel in the egui pass.
///
/// `EguiPlugin` requires the renderer (it registers shader assets into the
/// `RenderApp` sub-app), so when no `RenderApp` is present — a headless app built
/// on `MinimalPlugins`, as the smoke test does — this plugin is a no-op rather
/// than panicking. That is the correct behaviour anyway: with no window there is
/// nothing to click and nowhere to draw, so `cargo test --all-features` stays
/// green.
pub struct DebugPlugin;

impl Plugin for DebugPlugin {
    fn build(&self, app: &mut App) {
        if app.get_sub_app(RenderApp).is_none() {
            return;
        }

        app.init_resource::<InspectedEntity>()
            .add_plugins(EguiPlugin::default())
            .add_observer(on_right_click_inspect)
            .add_systems(Update, (arm_sprite_picking, clear_on_escape))
            .add_systems(EguiPrimaryContextPass, inspected_entity_ui);
    }
}

/// Make every world sprite pickable so a right-click can hit it. Inserts
/// `Pickable` once per sprite (tracked by [`InspectArmed`]); newly-spawned
/// sprites are armed on the next frame.
fn arm_sprite_picking(
    mut commands: Commands,
    sprites: Query<Entity, (With<Sprite>, Without<InspectArmed>)>,
) {
    for entity in &sprites {
        commands
            .entity(entity)
            .insert((Pickable::default(), InspectArmed));
    }
}

/// Global observer: select an entity for inspection on a right-click (secondary
/// button) **or** a Control+left-click. The latter is needed because macOS
/// delivers Control-click to the app as a *primary* button with Control held, not
/// as a secondary button — so a trackpad user without a dedicated right button can
/// still inspect. A plain left-click (no Control) is ignored here so targeting
/// keeps the primary button.
///
/// We only accept clicks that resolved to a `Sprite` entity. A single click fires
/// a `Pointer<Click>` for every picked entity under the cursor, including the
/// window-backed entity from the window picking backend; without this filter that
/// window pick (processed after the sprite's) would overwrite the selection, so
/// every inspect would show the window instead of the sprite clicked.
fn on_right_click_inspect(
    click: On<Pointer<Click>>,
    keys: Res<ButtonInput<KeyCode>>,
    sprites: Query<(), With<Sprite>>,
    mut inspected: ResMut<InspectedEntity>,
) {
    let entity = click.event().entity;
    if sprites.get(entity).is_err() {
        return;
    }
    let button = click.event().button;
    let ctrl_held = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    let inspect_requested =
        button == PointerButton::Secondary || (button == PointerButton::Primary && ctrl_held);
    if inspect_requested {
        inspected.0 = Some(entity);
    }
}

/// Dismiss the inspector panel when Escape is pressed.
fn clear_on_escape(keys: Res<ButtonInput<KeyCode>>, mut inspected: ResMut<InspectedEntity>) {
    if keys.just_pressed(KeyCode::Escape) {
        inspected.0 = None;
    }
}

/// egui pass: if an entity is selected (and still alive), show a window with its
/// component inspector. A despawned selection clears itself so the panel never
/// dangles on a dead entity.
fn inspected_entity_ui(world: &mut World) {
    let Some(entity) = world.resource::<InspectedEntity>().0 else {
        return;
    };
    // The selection may have been despawned (e.g. a defeated enemy); drop it.
    if world.get_entity(entity).is_err() {
        world.resource_mut::<InspectedEntity>().0 = None;
        return;
    }

    let egui_context = world
        .query_filtered::<&mut EguiContext, With<PrimaryEguiContext>>()
        .single(world);
    let Ok(egui_context) = egui_context else {
        return;
    };
    let mut egui_context = egui_context.clone();

    let mut open = true;
    bevy_inspector_egui::egui::Window::new(format!("Inspect {entity}"))
        .open(&mut open)
        .default_size((320.0, 400.0))
        .show(egui_context.get_mut(), |ui| {
            bevy_inspector_egui::egui::ScrollArea::both().show(ui, |ui| {
                bevy_inspector::ui_for_entity(world, entity, ui);
                ui.allocate_space(ui.available_size());
            });
        });

    // The window's close button (the `open` flag) dismisses the panel too.
    if !open {
        world.resource_mut::<InspectedEntity>().0 = None;
    }
}
