//! Common imports for the game's gameplay code.
//!
//! Re-exports the high-traffic types — the shared combat-entity components, the
//! combat event vocabulary, the top-level [`GameState`], and the pure damage
//! formula — so feature modules can `use crate::prelude::*;` instead of naming
//! each module path. Keep this list small and genuinely cross-cutting; types used
//! by a single feature belong with that feature, not here.

pub use crate::combat::{AttackRequested, DamageDealt, Died, compute_damage};
pub use crate::components::{
    CombatStats, DamageVariance, Defending, DisplayName, Enemy, EnemyHealthBar, Health, Player,
    Targeted,
};
pub use crate::state::GameState;
