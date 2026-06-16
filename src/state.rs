//! Top-level application state: which screen the game is currently on.
//!
//! Sits above the battle's own [`TurnPhase`](crate::battle::state::TurnPhase):
//! `GameState` decides *whether* a battle is on screen at all (the start-up menu
//! vs. an active fight), while `TurnPhase` drives the turn flow *within* a
//! battle. Keeping them as two separate states lets the battle systems gate on
//! [`GameState::InBattle`] so none of them run — and no battle UI is spawned —
//! while the main menu is up.

use bevy::prelude::*;

/// Which screen the game is showing.
///
/// - [`MainMenu`](Self::MainMenu) (default): the start-up menu with New Game /
///   Options / Credits. The battle systems and UI are dormant.
/// - [`InBattle`](Self::InBattle): an active battle; entered from the menu's
///   "New Game", which spawns the combatants and battle UI.
#[derive(States, Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum GameState {
    #[default]
    MainMenu,
    InBattle,
}
