//! Turn-flow state machine and the chained battle system sets.
//!
//! Input is only accepted in the phase that owns it, so "battle over disables
//! input" falls out of `run_if(in_state(...))` for free, with no manual flag.

use bevy::prelude::*;

/// Which side is acting, used to gate input and drive `OnEnter`/`OnExit` setup.
///
/// - [`PlayerTurn`](Self::PlayerTurn) (default): the action menu accepts
///   keyboard navigation and confirmation.
/// - [`Targeting`](Self::Targeting): Fight was chosen; the player is picking an
///   enemy (player attack lands in Phase 5).
/// - [`EnemyTurn`](Self::EnemyTurn): enemies act in index order (Phase 6).
/// - [`BattleOver`](Self::BattleOver): victory or defeat; all input is disabled.
///   The win/loss outcome itself is carried by [`BattleResult`], kept off the
///   state enum so every `in_state(BattleOver)` gate stays a plain unit match.
#[derive(States, Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum TurnPhase {
    #[default]
    PlayerTurn,
    Targeting,
    EnemyTurn,
    BattleOver,
}

/// The outcome of a finished battle, set the frame the battle ends and read
/// thereafter (the Phase 7 HUD shows "Victory!" vs "Game Over!").
///
/// Carried as a resource rather than a field on [`TurnPhase::BattleOver`] so the
/// state enum stays unit-only — `in_state(BattleOver)` and the `OnEnter`/`OnExit`
/// wiring need no payload, while consumers that care about *which* ending
/// occurred read this. `victory` is `true` when the player cleared the enemies,
/// `false` when the player fell.
#[derive(Resource, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BattleResult {
    pub victory: bool,
}

/// The four phases every battle frame runs through, chained in `Update` so they
/// execute in a deterministic order regardless of system insertion order.
///
/// - [`Input`](Self::Input): keyboard nav, action confirmation, enemy-turn timer.
/// - [`Resolve`](Self::Resolve): apply queued attacks and mutate health (Phase 5/6).
/// - [`Cleanup`](Self::Cleanup): check for battle end (Phase 6).
/// - [`Ui`](Self::Ui): redraw cursor, HP bars, and the battle log from world state.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BattleSet {
    Input,
    Resolve,
    Cleanup,
    Ui,
}
