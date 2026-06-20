//! Turn-based RPG battle vertical slice — Bevy port of the Godot 4.6 C# original.
//!
//! Modules are exposed publicly so integration tests in `tests/` can build
//! headless `App`s against the same plugins the binary uses, and exercise the
//! pure domain logic directly.

pub mod battle;
pub mod characters;
pub mod combat;
pub mod components;
#[cfg(feature = "debug-inspector")]
pub mod debug;
pub mod game;
pub mod game_over;
pub mod main_menu;
pub mod map;
pub mod prelude;
pub mod progress;
pub mod state;
