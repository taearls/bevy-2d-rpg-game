use bevy::prelude::*;

use crate::battle::BattlePlugin;
use crate::characters::CharactersPlugin;
use crate::main_menu::MainMenuPlugin;
use crate::state::GameState;

/// Root plugin for the game: sets the clear color, initialises the top-level
/// [`GameState`], spawns the shared 2D camera, and wires in the main-menu,
/// character-asset, and battle plugins (the last pulls in the HUD/log UI).
///
/// The game boots into [`GameState::MainMenu`]; the menu's "New Game" switches
/// to [`GameState::InBattle`], at which point the battle systems (all gated on
/// that state) spawn the combatants and battle UI.
pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ClearColor(Color::srgb(0.18, 0.18, 0.24)))
            .init_state::<GameState>()
            .add_systems(Startup, spawn_camera)
            .add_plugins((MainMenuPlugin, CharactersPlugin, BattlePlugin));

        // The egui debug inspector (right-click a sprite to inspect it) is
        // compiled in only under the `debug-inspector` feature, so default/release
        // builds and headless tests never link egui.
        #[cfg(feature = "debug-inspector")]
        app.add_plugins(crate::debug::DebugPlugin);
    }
}

/// Spawn the single 2D camera, shared by the main menu UI and the battle, so it
/// outlives the menu→battle transition (the battle no longer spawns its own).
fn spawn_camera(mut commands: Commands) {
    commands.spawn(Camera2d);
}
