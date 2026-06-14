//! The battle HUD: the player's name + HP fill bar, the dynamic alive-enemy
//! name labels with the targeting highlight, and the world-space enemy mini HP
//! bars beneath each sprite.
//!
//! Bevy port of the Godot `BattleUI` widget half. Where the original pushed
//! updates from a `HealthUpdated` signal, here each refresher is a system reading
//! ECS state directly: the player HUD and enemy bars react to `Changed<Health>`,
//! the enemy labels are rebuilt when the alive set changes, and the highlight
//! follows the live [`Targeted`] marker. The pure label helpers
//! ([`player_name_text`], [`hp_fill_fraction`]) are factored out so the
//! `BattleUITest` parity cases assert text and percentages without a renderer.

use bevy::prelude::*;

use crate::characters::components::{DisplayName, Enemy, EnemyHealthBar, Health, Player, Targeted};

/// Yellow used to highlight the enemy name label under the targeting cursor.
const HIGHLIGHT_COLOR: Color = Color::srgb(1.0, 1.0, 0.0);
/// White used for an un-highlighted enemy label and the player name.
const DEFAULT_COLOR: Color = Color::WHITE;

/// Background colour of an HP bar track (player and enemy alike).
const HP_TRACK_COLOR: Color = Color::srgb(0.25, 0.05, 0.05);
/// Fill colour of an HP bar.
const HP_FILL_COLOR: Color = Color::srgb(0.85, 0.15, 0.15);

/// Background of the full-width bottom info pane — the Godot `menu_panel`
/// `StyleBoxFlat` (`bg_color = (0.08, 0.08, 0.08, 0.7)`).
const INFO_PANE_COLOR: Color = Color::srgba(0.08, 0.08, 0.08, 0.7);
/// Height of the info pane (the Godot `MenuPanel` `offset_top = -160`).
const INFO_PANE_HEIGHT: f32 = 160.0;

/// World-space size of an enemy mini HP bar (the Godot `enemy_health_bar`
/// `custom_minimum_size` of 48×6).
const ENEMY_BAR_SIZE: Vec2 = Vec2::new(48.0, 6.0);
/// How far above the enemy sprite origin the mini HP bar sits.
const ENEMY_BAR_Y: f32 = 70.0;

/// Root of the bottom HUD bar (absolute, full-width). Holds the player HUD on the
/// right and the enemy label column on the left.
#[derive(Component, Debug)]
pub struct HudRoot;

/// The player's name `Text`. Its colour is fixed; only its text changes,
/// gaining the "(defeated)" suffix on death.
#[derive(Component, Debug)]
pub struct PlayerNameLabel;

/// The inner fill of the player's HP bar; its `width` is set to
/// `Val::Percent(100 * current / max)` each time the player's [`Health`] changes.
#[derive(Component, Debug)]
pub struct PlayerHpFill;

/// The column container that holds the dynamic enemy name labels (the Godot
/// `_enemyInfoContainer`).
#[derive(Component, Debug)]
pub struct EnemyLabelContainer;

/// One dynamic enemy name label, tagged with the enemy entity it names so the
/// highlight system can match it against the current [`Targeted`] enemy.
#[derive(Component, Debug, Clone, Copy)]
pub struct EnemyNameLabel(pub Entity);

/// The player's name as shown in the HUD: the bare name while alive, suffixed
/// `" (defeated)"` once dead. Pure port of the Godot
/// `player.IsAlive ? DisplayName : $"{DisplayName} (defeated)"`.
#[must_use]
pub fn player_name_text(name: &str, alive: bool) -> String {
    if alive {
        name.to_string()
    } else {
        format!("{name} (defeated)")
    }
}

/// Fraction of an HP bar to fill, in `0.0..=1.0` — shared by the player HP fill
/// and the enemy mini bars. Guards a zero or negative `max` (which a malformed
/// template could produce) by reading as empty rather than dividing by zero.
#[must_use]
pub fn hp_fill_fraction(current: i32, max: i32) -> f32 {
    if max <= 0 {
        return 0.0;
    }
    (current.max(0) as f32 / max as f32).clamp(0.0, 1.0)
}

/// Startup: spawn the static HUD tree — the bottom bar with the enemy-label
/// column on the left and the player name + HP track/fill on the right.
///
/// The enemy column starts empty; [`refresh_enemy_labels`] populates it from the
/// alive enemies once they spawn. The player widgets start blank and are filled
/// by [`refresh_player_hud`] on the first `Changed<Health>` (which fires the
/// frame the player spawns).
pub fn spawn_hud(mut commands: Commands) {
    // The Godot `MenuPanel`: a full-width `PanelContainer` anchored to the bottom
    // with a fixed height and the dark translucent background. Its single
    // `HBoxContainer` splits the row into enemy info (left) and player info
    // (right); `justify_content: SpaceBetween` reproduces the two
    // `size_flags_horizontal = 3` children pushing to the two edges.
    commands
        .spawn((
            HudRoot,
            Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(0.0),
                left: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Px(INFO_PANE_HEIGHT),
                // The Godot `menu_panel` content margins: 16 px left/right.
                padding: UiRect::axes(Val::Px(16.0), Val::Px(12.0)),
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(INFO_PANE_COLOR),
        ))
        .with_children(|root| {
            // Left: the dynamic enemy name column.
            root.spawn((
                EnemyLabelContainer,
                Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(4.0),
                    ..default()
                },
            ));

            // Right: the player name over a fixed-width HP track + fill, matching
            // the Godot `PlayerInfoContainer` (name right-aligned above a 200×12
            // `ProgressBar`).
            root.spawn(Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(4.0),
                width: Val::Px(200.0),
                align_items: AlignItems::FlexEnd,
                ..default()
            })
            .with_children(|column| {
                column.spawn((PlayerNameLabel, Text::new(""), TextColor(DEFAULT_COLOR)));
                // The HP track spans the column; the fill is a percentage-width
                // child so the player's health fraction maps straight to its
                // `width` (the Godot `ProgressBar.Value / MaxValue`).
                column
                    .spawn((
                        Node {
                            width: Val::Percent(100.0),
                            height: Val::Px(12.0),
                            ..default()
                        },
                        BackgroundColor(HP_TRACK_COLOR),
                    ))
                    .with_children(|track| {
                        track.spawn((
                            PlayerHpFill,
                            Node {
                                width: Val::Percent(100.0),
                                height: Val::Percent(100.0),
                                ..default()
                            },
                            BackgroundColor(HP_FILL_COLOR),
                        ));
                    });
            });
        });
}

/// `BattleSet::Ui`: on a player `Health` change, update the name label
/// (with the "(defeated)" suffix when dead) and set the HP fill width to the
/// health percentage. Mirrors Godot `UpdateHealth`'s player branch.
pub fn refresh_player_hud(
    player: Query<(&DisplayName, &Health), (With<Player>, Changed<Health>)>,
    mut name_label: Query<&mut Text, With<PlayerNameLabel>>,
    mut fill: Query<&mut Node, With<PlayerHpFill>>,
) {
    let Ok((DisplayName(name), health)) = player.single() else {
        return;
    };
    if let Ok(mut text) = name_label.single_mut() {
        text.0 = player_name_text(name, health.is_alive());
    }
    if let Ok(mut node) = fill.single_mut() {
        let fraction = hp_fill_fraction(health.current, health.max);
        node.width = Val::Percent(100.0 * fraction);
    }
}

/// `BattleSet::Ui`: rebuild the enemy name column whenever the set of alive
/// enemies changes — on death (one drops out) and on first spawn (they appear).
///
/// Rebuilds rather than diffs: the column is despawned-and-respawned from the
/// current alive enemies in layout order, so a defeated enemy's label simply
/// stops being re-created. Runs only when an enemy's [`Health`] changed, so a
/// steady state costs one cheap early-out. Mirrors the Godot
/// `ClearAndFreeChildren` + re-add of alive enemies.
pub fn refresh_enemy_labels(
    mut commands: Commands,
    changed: Query<(), (With<Enemy>, Changed<Health>)>,
    enemies: Query<(Entity, &Enemy, &DisplayName, &Health)>,
    container: Query<Entity, With<EnemyLabelContainer>>,
    existing: Query<Entity, With<EnemyNameLabel>>,
) {
    if changed.is_empty() {
        return;
    }
    let Ok(container) = container.single() else {
        return;
    };

    // Despawn the old labels (the Godot RemoveChild + Free), then re-add one per
    // alive enemy in layout order.
    for label in &existing {
        commands.entity(label).despawn();
    }

    let mut alive: Vec<(usize, Entity, String)> = enemies
        .iter()
        .filter(|(_, _, _, health)| health.is_alive())
        .map(|(entity, enemy, name, _)| (enemy.index, entity, name.0.clone()))
        .collect();
    alive.sort_by_key(|(index, _, _)| *index);

    commands.entity(container).with_children(|column| {
        for (_, entity, name) in alive {
            column.spawn((
                EnemyNameLabel(entity),
                Text::new(name),
                TextColor(DEFAULT_COLOR),
            ));
        }
    });
}

/// `BattleSet::Ui`: tint the enemy name label of the currently [`Targeted`] enemy
/// yellow and reset every other to white. Follows the live marker, so leaving
/// targeting (which removes the marker) clears the highlight with no extra
/// bookkeeping. Mirrors Godot `HighlightEnemyName` / `ClearEnemyHighlight`.
pub fn update_enemy_label_highlight(
    targeted: Query<Entity, With<Targeted>>,
    mut labels: Query<(&EnemyNameLabel, &mut TextColor)>,
) {
    let highlighted = targeted.single().ok();
    for (EnemyNameLabel(entity), mut color) in &mut labels {
        color.0 = if Some(*entity) == highlighted {
            HIGHLIGHT_COLOR
        } else {
            DEFAULT_COLOR
        };
    }
}

/// `BattleSet::Ui`: scale each enemy mini HP bar's fill to its owner's health
/// fraction whenever that enemy's [`Health`] changes.
///
/// The fill quad's base width is [`ENEMY_BAR_SIZE`]`.x`; scaling its X transform
/// by the fraction shrinks it from the centre. We re-anchor it so the bar drains
/// from the right (left-aligned), matching a conventional HP bar: the fill is
/// translated left by half the lost width. Reads `Changed<Health>` so a static
/// frame does no work.
pub fn sync_enemy_health_bars(
    changed: Query<(), (With<Enemy>, Changed<Health>)>,
    healths: Query<&Health>,
    mut bars: Query<(&EnemyHealthBar, &mut Transform)>,
) {
    if changed.is_empty() {
        return;
    }
    for (bar, mut transform) in &mut bars {
        let Ok(health) = healths.get(bar.owner) else {
            continue;
        };
        let fraction = hp_fill_fraction(health.current, health.max);
        transform.scale.x = fraction;
        // Keep the left edge pinned: as the fill shrinks by `(1 - fraction)` of
        // its width, shift its centre left by half that amount.
        let lost = ENEMY_BAR_SIZE.x * (1.0 - fraction);
        transform.translation.x = -lost / 2.0;
    }
}

/// Spawn an enemy's world-space mini HP bar as two child sprite quads — a dark
/// track and a red fill — positioned [`ENEMY_BAR_Y`] above the sprite origin.
///
/// Called from the enemy spawner so each enemy owns its bar. The fill carries an
/// [`EnemyHealthBar`] tagged with `owner` so [`sync_enemy_health_bars`] can scale
/// it against that enemy's health; the track is static. The fill's child
/// transform is relative to the enemy, so it rides along as the enemy moves.
pub fn spawn_enemy_health_bar(parent: &mut ChildSpawnerCommands, owner: Entity) {
    // Dark track behind the fill.
    parent.spawn((
        Sprite::from_color(HP_TRACK_COLOR, ENEMY_BAR_SIZE),
        Transform::from_xyz(0.0, ENEMY_BAR_Y, 0.5),
    ));
    // Red fill, full width at spawn (full health); scaled down by
    // `sync_enemy_health_bars` as the enemy takes damage.
    parent.spawn((
        EnemyHealthBar { owner },
        Sprite::from_color(HP_FILL_COLOR, ENEMY_BAR_SIZE),
        Transform::from_xyz(0.0, ENEMY_BAR_Y, 0.6),
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The player name gains the "(defeated)" suffix exactly when dead.
    #[test]
    fn player_name_suffixes_on_death() {
        assert_eq!(player_name_text("Hero", true), "Hero");
        assert_eq!(player_name_text("Hero", false), "Hero (defeated)");
    }

    /// The fill fraction tracks current/max and clamps the edges.
    #[test]
    fn fill_fraction_tracks_and_clamps() {
        assert!((hp_fill_fraction(100, 100) - 1.0).abs() < f32::EPSILON);
        assert!((hp_fill_fraction(50, 100) - 0.5).abs() < f32::EPSILON);
        assert!((hp_fill_fraction(0, 100) - 0.0).abs() < f32::EPSILON);
        // Over-full and negative are clamped into range.
        assert!((hp_fill_fraction(150, 100) - 1.0).abs() < f32::EPSILON);
        assert!((hp_fill_fraction(-10, 100) - 0.0).abs() < f32::EPSILON);
    }

    /// A non-positive max reads as empty rather than dividing by zero.
    #[test]
    fn fill_fraction_guards_zero_max() {
        assert!((hp_fill_fraction(10, 0) - 0.0).abs() < f32::EPSILON);
        assert!((hp_fill_fraction(10, -5) - 0.0).abs() < f32::EPSILON);
    }
}
