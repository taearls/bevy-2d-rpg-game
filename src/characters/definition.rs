//! Serde-deserializable character templates.
//!
//! These are the data-driven roster definitions loaded from RON assets in
//! Phase 3. Defining them as plain structs here keeps the domain layer testable
//! before the asset pipeline exists. Field defaults mirror the Godot
//! `CombatStats.cs` exports (`MaxHealth` 100, `Attack` 10, `Defense` 5).

use serde::{Deserialize, Serialize};

/// Combat stat block for a character template. Any field omitted from the RON
/// source falls back to the Godot default.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CombatStatsDef {
    #[serde(default = "default_max_health")]
    pub max_health: i32,
    #[serde(default = "default_attack")]
    pub attack: i32,
    #[serde(default = "default_defense")]
    pub defense: i32,
}

impl Default for CombatStatsDef {
    fn default() -> Self {
        Self {
            max_health: default_max_health(),
            attack: default_attack(),
            defense: default_defense(),
        }
    }
}

fn default_max_health() -> i32 {
    100
}

fn default_attack() -> i32 {
    10
}

fn default_defense() -> i32 {
    5
}

/// A named, data-driven character template (the player or an enemy archetype).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CharacterDef {
    pub display_name: String,
    #[serde(default)]
    pub stats: CombatStatsDef,
}
