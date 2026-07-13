//! Serde-deserializable character templates.
//!
//! These are the data-driven roster definitions loaded from RON assets
//! (`assets/characters/*.character.ron`) via the [`asset_loader`]. The RON
//! files are the source of truth: every field must be specified explicitly —
//! there are no serde defaults. `attack`/`defense` mirror the Godot
//! `CombatStats.cs` exports; `max_health` is tuned down from Godot's 100.
//! `damage_variance` mirrors the Godot `BattleCharacter` min/max exports.
//!
//! [`asset_loader`]: super::asset_loader

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Combat stat block for a character template. Every field must be present in
/// the RON source (see the module docs).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CombatStatsDef {
    pub max_health: i32,
    pub attack: i32,
    pub defense: i32,
}

/// Multiplicative damage spread for a character template — a roll is sampled
/// uniformly from `[min, max]` each time the character deals damage. Mirrors the
/// [`DamageVariance`](crate::components::DamageVariance) component; must be
/// present in the RON source (see the module docs).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct DamageVarianceDef {
    pub min: f32,
    pub max: f32,
}

/// A named, data-driven character template (the player or an enemy archetype),
/// loaded from a `*.character.ron` asset. `sprite` is the asset path of the
/// character's texture, relative to the `assets/` root (e.g.
/// `"sprites/hero.png"`), mirroring the Godot `CharacterData.Sprite` export.
#[derive(Asset, TypePath, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CharacterDef {
    pub display_name: String,
    pub sprite: String,
    pub stats: CombatStatsDef,
    pub damage_variance: DamageVarianceDef,
}
