//! Turn-flow state machine and the chained battle system sets.
//!
//! Mirrors the Godot `TurnState` enum that gated `BattleScene._UnhandledInput`:
//! input is only accepted in the phase that owns it, so "battle over disables
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
#[derive(States, Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum TurnPhase {
    #[default]
    PlayerTurn,
    Targeting,
    EnemyTurn,
    BattleOver,
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
