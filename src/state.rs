//! Top-level application state: which screen the game is currently on.
//!
//! Sits above the battle's own [`TurnPhase`](crate::battle::state::TurnPhase):
//! `GameState` decides *which* screen is on display (the start-up menu, the
//! overworld map, an active fight, or the game-over screen), while `TurnPhase`
//! drives the turn flow *within* a battle. Keeping them as two separate states
//! lets the battle systems gate on [`GameState::InBattle`] so none of them run —
//! and no battle UI is spawned — while another screen is up.

use bevy::prelude::*;

/// Which screen the game is showing.
///
/// The flow is: [`MainMenu`](Self::MainMenu) → New Game → [`Map`](Self::Map) →
/// (a random encounter) → [`InBattle`](Self::InBattle). Winning a battle returns
/// to [`Map`](Self::Map) with the player's hit points carried over; losing one
/// moves to [`GameOver`](Self::GameOver), whose menu either restarts the game
/// (back to [`Map`](Self::Map) at full health) or returns to the title screen.
///
/// - [`MainMenu`](Self::MainMenu) (default): the start-up menu with New Game /
///   Options / Credits. The map and battle systems are dormant.
/// - [`Map`](Self::Map): the explorable overworld; the player walks around and a
///   battle may start at random.
/// - [`InBattle`](Self::InBattle): an active battle, entered from a map
///   encounter, which spawns the combatants and battle UI.
/// - [`GameOver`](Self::GameOver): shown after a defeat, offering "Restart Game"
///   and "Return to Title Screen".
#[derive(States, Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum GameState {
    #[default]
    MainMenu,
    Map,
    InBattle,
    GameOver,
}

/// Whether a debug tool is currently capturing keyboard input, in which case
/// gameplay input (the battle menu, targeting, …) is suppressed so the two don't
/// both act on the same keypress.
///
/// Always compiled, in every build, so gameplay code can gate on it without a
/// `#[cfg(feature = "debug-overlay")]` dance — but it is only ever *set* true by
/// the debug-overlay entity inspector (which is modal while an entity is
/// selected). In default/release/wasm builds nothing flips it, so it stays
/// `false` and gameplay input is never gated. See
/// [`debug_capturing_input`](Self::active) for the `run_if` condition.
#[derive(Resource, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DebugInputCapture(pub bool);

impl DebugInputCapture {
    /// `run_if` condition: true while a debug tool owns the keyboard. Gameplay
    /// input systems gate on its negation (`not(DebugInputCapture::active)`).
    #[must_use]
    pub fn active(capture: Res<Self>) -> bool {
        capture.0
    }
}
