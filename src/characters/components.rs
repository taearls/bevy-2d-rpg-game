//! ECS components describing a battle character's identity and combat profile.
//!
//! These are plain data components — the spawning, rendering, and combat
//! systems that consume them arrive in later phases. Mirrors the per-character
//! fields of the Godot `BattleCharacter.cs` / `CombatStats.cs` originals.

use bevy::prelude::*;

/// Marks the single player-controlled character.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Player;

/// Marks an enemy combatant and records its slot in the spawned row. `index`
/// runs `0..enemy_count` left-to-right and drives layout, enemy-turn ordering,
/// and the Godot `EnemyIndex` parity used by targeting.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Enemy {
    pub index: usize,
}

/// Human-readable name shown in the HUD and battle log (e.g. `"Goblin A"`).
#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub struct DisplayName(pub String);

/// Current and maximum hit points. `current` is clamped to `0..=max` by the
/// combat systems; this component itself imposes no invariant on construction.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
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

    /// Mirrors Godot `BattleCharacter.IsAlive => CurrentHealth > 0`.
    #[must_use]
    pub fn is_alive(&self) -> bool {
        self.current > 0
    }
}

/// Offensive and defensive stats feeding the damage formula.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct CombatStats {
    pub attack: i32,
    pub defense: i32,
}

/// Per-character multiplicative damage spread. A roll is sampled uniformly from
/// `[min, max]` each time the character deals damage. Defaults match the Godot
/// `BattleCharacter` exports (0.8 / 1.2).
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct DamageVariance {
    pub min: f32,
    pub max: f32,
}

impl Default for DamageVariance {
    fn default() -> Self {
        Self { min: 0.8, max: 1.2 }
    }
}
