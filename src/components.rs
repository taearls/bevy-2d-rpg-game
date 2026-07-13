//! Shared ECS component vocabulary for combat entities — identity, stats, and
//! battle-state markers used across the `battle`, `combat`, and `map` features.
//!
//! These are plain data components consumed by the spawning, rendering, and
//! combat systems in those modules.

use bevy::prelude::*;

/// Marks the single player-controlled character.
#[derive(Component, Reflect, Debug, Clone, Copy, PartialEq, Eq)]
#[reflect(Component)]
pub struct Player;

/// Marks an enemy combatant and records its slot in the spawned row. `index`
/// runs `0..enemy_count` left-to-right and drives layout, enemy-turn ordering,
/// and the target index used by targeting.
///
/// `FromTemplate` lets it be written as `Enemy { index }` in a `bsn!` scene.
#[derive(Component, Reflect, Debug, Clone, Copy, PartialEq, Eq, FromTemplate)]
#[reflect(Component)]
pub struct Enemy {
    pub index: usize,
}

/// Human-readable name shown in the HUD and battle log (e.g. `"Goblin A"`).
#[derive(Component, Reflect, Debug, Clone, PartialEq, Eq, FromTemplate)]
#[reflect(Component)]
pub struct DisplayName(pub String);

/// Current and maximum hit points. `current` is clamped to `0..=max` by the
/// combat systems; this component itself imposes no invariant on construction.
#[derive(Component, Reflect, Debug, Clone, Copy, PartialEq, Eq, FromTemplate)]
#[reflect(Component)]
pub struct Health {
    pub current: i32,
    pub max: i32,
}

impl Health {
    /// Create a character at full health.
    #[must_use]
    pub fn full(max: i32) -> Self {
        Self { current: max, max }
    }

    /// A character is alive while its current health is above zero.
    #[must_use]
    pub fn is_alive(&self) -> bool {
        self.current > 0
    }
}

/// Marks the player as defending for the upcoming enemy turn. Inserted by the
/// Defend action and removed `OnEnter(PlayerTurn)`. Phase 6 halves an incoming
/// attack's value while this marker is present, applied before the damage
/// formula.
#[derive(Component, Reflect, Debug, Clone, Copy, PartialEq, Eq)]
#[reflect(Component)]
pub struct Defending;

/// Marks the enemy currently under the targeting cursor. Drives the yellow
/// sprite tint and is the one entity the selection indicator sits above. Exactly
/// one alive enemy carries it while in [`Targeting`](crate::battle::state::TurnPhase::Targeting);
/// it is removed when targeting ends (confirm or cancel).
#[derive(Component, Reflect, Debug, Clone, Copy, PartialEq, Eq)]
#[reflect(Component)]
pub struct Targeted;

/// The world-space mini HP bar's fill quad, parented under an enemy sprite and
/// scaled along X by the owner's health fraction. `owner` is the enemy whose
/// [`Health`] drives the fill, kept on the component so the HUD can scale each
/// fill against the right entity without walking the parent hierarchy.
/// `FromTemplate` lets the `bsn!` macro accept an entity reference (`#enemy`) for
/// the `owner` field when the HP bar is spawned inside the enemy's scene.
#[derive(Component, Reflect, Debug, Clone, Copy, PartialEq, Eq, FromTemplate)]
#[reflect(Component)]
pub struct EnemyHealthBar {
    pub owner: Entity,
}

/// Offensive and defensive stats feeding the damage formula.
#[derive(Component, Reflect, Debug, Clone, Copy, PartialEq, Eq, FromTemplate)]
#[reflect(Component)]
pub struct CombatStats {
    pub attack: i32,
    pub defense: i32,
}

/// Per-character multiplicative damage spread. A roll is sampled uniformly from
/// `[min, max]` each time the character deals damage. Seeded from the character
/// template's `damage_variance` (see [`DamageVarianceDef`]); the `Default`
/// (0.8 / 1.2) is used only as a non-spawn fallback (e.g. the debug inspector).
///
/// [`DamageVarianceDef`]: crate::characters::definition::DamageVarianceDef
#[derive(Component, Reflect, Debug, Clone, Copy, PartialEq, FromTemplate)]
#[reflect(Component)]
pub struct DamageVariance {
    pub min: f32,
    pub max: f32,
}

impl Default for DamageVariance {
    fn default() -> Self {
        Self { min: 0.8, max: 1.2 }
    }
}
