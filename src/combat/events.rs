//! Combat event types.
//!
//! Two flavours, split along Bevy's message/observer line:
//!
//! - **Frame-buffered [`Message`]s** for "something happened" streams that any
//!   system can drain next frame: [`AttackRequested`] (an attack to resolve) and
//!   [`DamageDealt`] (the result, for HUD/log consumers).
//! - **Immediate [`EntityEvent`]** for an entity-scoped reaction that must run
//!   the instant it fires: [`Died`], observed to hide the defeated sprite.
//!
//! Messages and observers are despawn-safe by construction, so there is nothing
//! to unsubscribe.

use bevy::prelude::*;

/// A request to resolve one attack from `attacker` against `target`.
///
/// Written by the targeting confirm (and, in Phase 6, the enemy-turn queue) and
/// drained by [`apply_attacks`](super::resolve::apply_attacks). Carrying both
/// entities keeps the resolver a pure function of the world — it never needs to
/// know who queued the attack.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct AttackRequested {
    pub attacker: Entity,
    pub target: Entity,
}

/// The outcome of a resolved attack: `target` took `amount` damage from
/// `attacker`.
///
/// Emitted by [`apply_attacks`](super::resolve::apply_attacks) after `Health`
/// has been mutated, so HUD and log consumers (Phase 7) can react without
/// re-deriving the formula.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct DamageDealt {
    pub attacker: Entity,
    pub target: Entity,
    pub amount: i32,
}

/// Fired the instant a combatant's health reaches zero, targeting the entity
/// that died.
///
/// An [`EntityEvent`] rather than a buffered message so the reaction
/// (hiding the sprite, and in Phase 6 short-circuiting to game over on player
/// death) runs immediately as part of resolution, not a frame later. Triggered
/// by [`apply_attacks`](super::resolve::apply_attacks); observed by
/// [`on_died_hide_sprite`](super::resolve::on_died_hide_sprite).
#[derive(EntityEvent, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Died {
    /// The entity that died (the event target).
    pub entity: Entity,
}
