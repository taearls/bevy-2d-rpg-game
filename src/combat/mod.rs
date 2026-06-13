//! Combat: the pure damage formula, the combat event types, and the resolution
//! systems that turn queued attacks into health changes and deaths.
//!
//! Split so the math ([`damage`]) stays unit-testable without an ECS world, the
//! event vocabulary ([`events`]) is shared by producers (targeting, enemy turn)
//! and consumers (HUD, log), and the resolution plumbing ([`resolve`]) lives on
//! its own. The systems are wired into the chained [`BattleSet`]s by
//! [`BattlePlugin`].
//!
//! [`BattleSet`]: crate::battle::state::BattleSet
//! [`BattlePlugin`]: crate::battle::BattlePlugin

pub mod damage;
pub mod events;
pub mod resolve;

pub use damage::compute_damage;
pub use events::{AttackRequested, DamageDealt, Died};
pub use resolve::{apply_attacks, check_battle_end, on_died_hide_sprite};
