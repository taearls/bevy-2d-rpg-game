//! The overworld map: a walkable space the player roams between battles, where a
//! random encounter eventually drops them into a fight.
//!
//! Deliberately generic-but-functional: the "map" is a checkerboard of tiles, the
//! player is a single sprite moved with the arrow keys or WASD, and an encounter
//! is triggered purely by distance travelled. Every map entity is tagged
//! [`DespawnOnExit(Map)`](DespawnOnExit) so a battle (or a return to the title)
//! tears the whole scene down, and it is rebuilt fresh on the next
//! `OnEnter(Map)` — including a re-rolled encounter distance, so each stint on
//! the map walks a new amount before the next fight.
//!
//! The encounter roll ([`encounter_triggered`]) and the input-to-direction
//! mapping ([`movement_direction`]) are pure functions so the wander-then-fight
//! loop can be asserted headlessly without a renderer or an input device.

use bevy::prelude::*;
use rand::Rng;
use rand_chacha::ChaCha8Rng;
use rand_chacha::rand_core::SeedableRng;

use crate::battle::spawn::Roster;
use crate::characters::definition::CharacterDef;
use crate::progress::PlayerProgress;
use crate::state::GameState;

/// Player walking speed on the map, in world units per second.
const MOVE_SPEED: f32 = 260.0;

/// Half-extent of the walkable area, in world units (origin-centred). The avatar
/// is clamped inside `[-x, x] × [-y, y]` so it never wanders off-screen. Kept a
/// little inside the 1152×648 window so the sprite stays fully visible.
const MAP_HALF_EXTENT: Vec2 = Vec2::new(540.0, 270.0);

/// Side length of one background tile, in world units.
const TILE_SIZE: f32 = 72.0;

/// The two checkerboard tile colours.
const TILE_COLOR_A: Color = Color::srgb(0.20, 0.38, 0.22);
const TILE_COLOR_B: Color = Color::srgb(0.16, 0.32, 0.18);

/// Size of the player's avatar sprite, in world units.
const AVATAR_SIZE: Vec2 = Vec2::new(40.0, 56.0);
/// Avatar tint (no character art needed — the map is intentionally generic).
const AVATAR_COLOR: Color = Color::srgb(0.85, 0.78, 0.35);

/// Inclusive band of travel distance (world units) rolled for the next
/// encounter. With [`MOVE_SPEED`] this is roughly 2.5–6 seconds of walking.
const ENCOUNTER_DISTANCE_MIN: f32 = 650.0;
const ENCOUNTER_DISTANCE_MAX: f32 = 1500.0;

/// Marks the player's map avatar (distinct from the battle [`Player`]).
///
/// [`Player`]: crate::components::Player
#[derive(Component, Debug)]
pub struct MapPlayer;

/// The on-screen HP readout shown while exploring.
#[derive(Component, Debug)]
pub struct MapHpText;

/// RNG governing how far the player walks before the next random encounter.
/// Entropy-seeded for live play; tests seed it so the encounter distance — and
/// thus when the fight starts — is deterministic.
#[derive(Resource, Debug)]
pub struct MapRng(pub ChaCha8Rng);

impl MapRng {
    /// Seed deterministically from an integer (used in tests).
    #[must_use]
    pub fn from_seed(seed: u64) -> Self {
        Self(ChaCha8Rng::seed_from_u64(seed))
    }

    /// Seed from OS entropy for live play.
    #[must_use]
    pub fn from_entropy() -> Self {
        Self(ChaCha8Rng::from_os_rng())
    }
}

impl Default for MapRng {
    fn default() -> Self {
        Self::from_entropy()
    }
}

/// Tracks progress toward the next random encounter.
///
/// `distance` accumulates the world-space distance the avatar has walked since
/// the last reset; once it reaches `threshold` an encounter fires. Both are
/// reset (and `threshold` re-rolled) on every `OnEnter(Map)`.
#[derive(Resource, Debug, Default)]
pub struct EncounterTracker {
    /// Distance walked since the last reset, in world units.
    pub distance: f32,
    /// Distance that triggers the next encounter, rolled from [`MapRng`].
    pub threshold: f32,
}

/// Wires the overworld map: builds the scene on entering [`GameState::Map`] and
/// runs movement, the encounter check, and the HP readout while it is up. Every
/// spawned entity is `DespawnOnExit(Map)`, so leaving the map (into a battle or
/// back to the title) cleans the scene up automatically.
pub(crate) fn plugin(app: &mut App) {
    app.init_resource::<MapRng>()
        .init_resource::<EncounterTracker>()
        .add_systems(OnEnter(GameState::Map), setup_map)
        .add_systems(
            Update,
            (move_player, check_encounter, update_map_hp_text)
                .chain()
                .run_if(in_state(GameState::Map)),
        );
}

/// The unit movement direction for the given input axes.
///
/// `horizontal` is `right − left` and `vertical` is `up − down` (each held key
/// contributing ±1, computed by the caller), so opposing keys cancel to a zero
/// axis. Returns [`Vec2::ZERO`] for no input and a normalised vector otherwise,
/// so diagonal movement is not faster than orthogonal. Pure so the
/// normalisation and zero cases can be asserted without an input device.
#[must_use]
pub fn movement_direction(horizontal: i32, vertical: i32) -> Vec2 {
    Vec2::new(horizontal as f32, vertical as f32).normalize_or_zero()
}

/// Whether enough distance has been walked to trigger an encounter.
#[must_use]
pub fn encounter_triggered(distance: f32, threshold: f32) -> bool {
    distance >= threshold
}

/// Roll the distance the player must walk before the next encounter.
fn roll_encounter_distance(rng: &mut ChaCha8Rng) -> f32 {
    rng.random_range(ENCOUNTER_DISTANCE_MIN..=ENCOUNTER_DISTANCE_MAX)
}

/// `OnEnter(Map)`: build the checkerboard, the avatar, and the HUD, and arm a
/// fresh encounter distance.
///
/// Resets the [`EncounterTracker`] so returning from a battle starts a new count
/// rather than instantly re-triggering. The avatar always spawns at the centre.
pub fn setup_map(
    mut commands: Commands,
    mut rng: ResMut<MapRng>,
    mut tracker: ResMut<EncounterTracker>,
) {
    tracker.distance = 0.0;
    tracker.threshold = roll_encounter_distance(&mut rng.0);

    // Checkerboard background. Each tile is its own sprite tagged for despawn on
    // leaving the map; the grid is sized to blanket the walkable extent plus a
    // tile of margin so the edges never show a gap.
    let cols = (MAP_HALF_EXTENT.x / TILE_SIZE).ceil() as i32 + 1;
    let rows = (MAP_HALF_EXTENT.y / TILE_SIZE).ceil() as i32 + 1;
    for ty in -rows..=rows {
        for tx in -cols..=cols {
            let color = if (tx + ty).rem_euclid(2) == 0 {
                TILE_COLOR_A
            } else {
                TILE_COLOR_B
            };
            commands.spawn((
                Sprite::from_color(color, Vec2::splat(TILE_SIZE)),
                Transform::from_xyz(tx as f32 * TILE_SIZE, ty as f32 * TILE_SIZE, -1.0),
                DespawnOnExit(GameState::Map),
            ));
        }
    }

    // The player's avatar, drawn above the tiles.
    commands.spawn((
        MapPlayer,
        Sprite::from_color(AVATAR_COLOR, AVATAR_SIZE),
        Transform::from_xyz(0.0, 0.0, 0.0),
        DespawnOnExit(GameState::Map),
    ));

    // A short instruction line, top-centre.
    commands.spawn((
        Text::new("Explore with arrow keys / WASD - a battle may find you."),
        TextFont {
            font_size: FontSize::Px(20.0),
            ..default()
        },
        TextColor(Color::WHITE),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(16.0),
            left: Val::Px(16.0),
            ..default()
        },
        DespawnOnExit(GameState::Map),
    ));

    // The HP readout, top-right; filled in by `update_map_hp_text`.
    commands.spawn((
        MapHpText,
        Text::new(""),
        TextFont {
            font_size: FontSize::Px(24.0),
            ..default()
        },
        TextColor(Color::WHITE),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(16.0),
            right: Val::Px(16.0),
            ..default()
        },
        DespawnOnExit(GameState::Map),
    ));
}

/// `Update` (in `Map`): move the avatar from held keys, clamp it to the walkable
/// area, and accumulate the distance walked toward the next encounter.
pub fn move_player(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mut tracker: ResMut<EncounterTracker>,
    mut avatar: Query<&mut Transform, With<MapPlayer>>,
) {
    let Ok(mut transform) = avatar.single_mut() else {
        return;
    };

    let horizontal = i32::from(keys.any_pressed([KeyCode::ArrowRight, KeyCode::KeyD]))
        - i32::from(keys.any_pressed([KeyCode::ArrowLeft, KeyCode::KeyA]));
    let vertical = i32::from(keys.any_pressed([KeyCode::ArrowUp, KeyCode::KeyW]))
        - i32::from(keys.any_pressed([KeyCode::ArrowDown, KeyCode::KeyS]));
    let direction = movement_direction(horizontal, vertical);
    if direction == Vec2::ZERO {
        return;
    }

    let step = direction * MOVE_SPEED * time.delta_secs();
    // Accumulate the intended travel — even when clamped against an edge — so the
    // player can never get encounter-stuck walking into a boundary.
    tracker.distance += step.length();
    let new_pos = (transform.translation.truncate() + step).clamp(
        -MAP_HALF_EXTENT + AVATAR_SIZE / 2.0,
        MAP_HALF_EXTENT - AVATAR_SIZE / 2.0,
    );
    transform.translation.x = new_pos.x;
    transform.translation.y = new_pos.y;
}

/// `Update` (in `Map`): start a battle once the player has walked far enough.
pub fn check_encounter(
    tracker: Res<EncounterTracker>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    if encounter_triggered(tracker.distance, tracker.threshold) {
        info!("A wild encounter begins!");
        next_state.set(GameState::InBattle);
    }
}

/// `Update` (in `Map`): mirror the persisted player health into the HP readout.
pub fn update_map_hp_text(
    progress: Res<PlayerProgress>,
    roster: Option<Res<Roster>>,
    defs: Res<Assets<CharacterDef>>,
    mut text: Query<&mut Text, With<MapHpText>>,
) {
    let Ok(mut text) = text.single_mut() else {
        return;
    };
    let label = match progress.health {
        Some(health) => format!("HP: {}/{}", health.current.max(0), health.max),
        // Health not yet seeded: fall back to the hero template's max if it is
        // resident, otherwise a neutral placeholder.
        None => roster
            .and_then(|roster| defs.get(&roster.hero).map(|hero| hero.stats.max_health))
            .map_or_else(|| "HP: --".to_string(), |max| format!("HP: {max}/{max}")),
    };
    if text.0 != label {
        text.0 = label;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// No (or cancelled-out) input yields no movement.
    #[test]
    fn zero_axes_is_zero() {
        assert_eq!(movement_direction(0, 0), Vec2::ZERO);
    }

    /// A single axis gives a unit vector in that direction (Y-up).
    #[test]
    fn single_axis_is_unit() {
        assert_eq!(movement_direction(0, 1), Vec2::Y);
        assert_eq!(movement_direction(0, -1), -Vec2::Y);
        assert_eq!(movement_direction(1, 0), Vec2::X);
        assert_eq!(movement_direction(-1, 0), -Vec2::X);
    }

    /// Diagonal movement is normalised, so it is no faster than orthogonal.
    #[test]
    fn diagonal_is_normalised() {
        let dir = movement_direction(1, 1);
        assert!(
            (dir.length() - 1.0).abs() < 1e-6,
            "diagonal must be unit length"
        );
        assert!(dir.x > 0.0 && dir.y > 0.0);
    }

    /// The encounter fires only once the walked distance reaches the threshold.
    #[test]
    fn encounter_fires_at_threshold() {
        assert!(!encounter_triggered(0.0, 100.0));
        assert!(!encounter_triggered(99.9, 100.0));
        assert!(encounter_triggered(100.0, 100.0));
        assert!(encounter_triggered(150.0, 100.0));
    }

    /// Rolled encounter distances stay within the configured band.
    #[test]
    fn rolled_distance_is_in_band() {
        let mut rng = ChaCha8Rng::seed_from_u64(7);
        for _ in 0..200 {
            let d = roll_encounter_distance(&mut rng);
            assert!((ENCOUNTER_DISTANCE_MIN..=ENCOUNTER_DISTANCE_MAX).contains(&d));
        }
    }
}
