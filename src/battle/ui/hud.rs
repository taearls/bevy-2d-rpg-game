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
/// How far above the enemy sprite origin the mini HP bar sits. Kept close to the
/// sprite so the bar reads as belonging to it.
const ENEMY_BAR_Y: f32 = 45.0;
/// How far above the HP bar the enemy name label floats, so the stack reads
/// name → bar → sprite from top to bottom.
const ENEMY_LABEL_Y: f32 = ENEMY_BAR_Y + 18.0;
/// Font size of the world-space enemy name label.
const ENEMY_LABEL_FONT_SIZE: f32 = 16.0;

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

/// One world-space enemy name label, floating above that enemy's HP bar. Tagged
/// with the enemy entity it names so the highlight system can match it against
/// the current [`Targeted`] enemy and the death system can drop it when the enemy
/// dies. Spawned as a child of the enemy (alongside the HP bar), so it rides
/// along with the sprite.
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

/// Startup: spawn the static HUD tree — the bottom info pane holding the player
/// name + HP track/fill, right-aligned.
///
/// Enemy names no longer live here: they float in world space above each enemy's
/// HP bar (see [`spawn_enemy_health_bar`]). The player widgets start blank and are
/// filled by [`refresh_player_hud`] on the first `Changed<Health>` (which fires
/// the frame the player spawns).
pub fn spawn_hud(mut commands: Commands) {
    // The Godot `MenuPanel`: a full-width `PanelContainer` anchored to the bottom
    // with a fixed height and the dark translucent background. The player info
    // sits at the right edge (`justify_content: FlexEnd`).
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
                justify_content: JustifyContent::FlexEnd,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(INFO_PANE_COLOR),
        ))
        .with_children(|root| {
            // The player name over a fixed-width HP track + fill, matching the
            // Godot `PlayerInfoContainer` (name right-aligned above a 200×12
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
    player: Query<
        (&DisplayName, &Health),
        // Also refresh on a name edit (e.g. from the debug inspector), not just a
        // health change, so a `DisplayName` tweak updates the on-screen label live.
        (With<Player>, Or<(Changed<Health>, Changed<DisplayName>)>),
    >,
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

/// `BattleSet::Ui`: drop a world-space enemy name label once its enemy dies, so a
/// defeated enemy keeps neither a name nor an HP bar floating over its sprite.
///
/// The label is spawned as a child of the enemy (see [`spawn_enemy_health_bar`]),
/// so it appears with the sprite and rides along with it; this system only needs
/// to remove it on death. Runs only when an enemy's [`Health`] changed, so a
/// steady state costs one cheap early-out. Replaces the Godot
/// `ClearAndFreeChildren` + re-add of alive enemies.
pub fn refresh_enemy_labels(
    mut commands: Commands,
    changed: Query<(), (With<Enemy>, Changed<Health>)>,
    healths: Query<&Health>,
    labels: Query<(Entity, &EnemyNameLabel)>,
) {
    if changed.is_empty() {
        return;
    }
    for (label, EnemyNameLabel(owner)) in &labels {
        // Despawn the label if its owner is gone or no longer alive.
        let dead = healths.get(*owner).is_ok_and(|h| !h.is_alive());
        if dead || healths.get(*owner).is_err() {
            commands.entity(label).despawn();
        }
    }
}

/// `BattleSet::Ui`: push an enemy's [`DisplayName`] into its world-space label
/// when the name changes — e.g. when edited in the debug inspector — so the
/// on-screen label tracks it live.
///
/// The label is a `Text2d` child carrying [`EnemyNameLabel`]`(owner)`; this maps
/// each changed enemy to its label by `owner` and rewrites the text. Gated on
/// `Changed<DisplayName>`, so a steady state does no work.
pub fn sync_enemy_label_text(
    enemies: Query<(Entity, &DisplayName), (With<Enemy>, Changed<DisplayName>)>,
    mut labels: Query<(&EnemyNameLabel, &mut Text2d)>,
) {
    if enemies.is_empty() {
        return;
    }
    for (owner, DisplayName(name)) in &enemies {
        for (EnemyNameLabel(label_owner), mut text) in &mut labels {
            if *label_owner == owner {
                text.0.clone_from(name);
            }
        }
    }
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

/// Spawn an enemy's world-space overlay — the name label, the mini HP bar (dark
/// track + red fill), stacked above the sprite as name → bar → sprite.
///
/// Called from the enemy spawner so each enemy owns these. The bar sits
/// [`ENEMY_BAR_Y`] above the sprite origin and the name [`ENEMY_LABEL_Y`] above
/// that. The fill carries an [`EnemyHealthBar`] tagged with `owner` so
/// [`sync_enemy_health_bars`] can scale it against that enemy's health; the name
/// carries an [`EnemyNameLabel`] for the targeting highlight and the death
/// despawn. All are children of the enemy, so they ride along with the sprite.
pub fn spawn_enemy_health_bar(parent: &mut ChildSpawnerCommands, owner: Entity, name: &str) {
    // Name label, floating above the HP bar.
    parent.spawn((
        EnemyNameLabel(owner),
        Text2d::new(name.to_string()),
        TextFont {
            font_size: ENEMY_LABEL_FONT_SIZE,
            ..default()
        },
        TextColor(DEFAULT_COLOR),
        Transform::from_xyz(0.0, ENEMY_LABEL_Y, 0.7),
    ));
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
