use bevy::prelude::*;

use crate::battle::BattlePlugin;
use crate::characters::CharactersPlugin;
use crate::game_over::GameOverPlugin;
use crate::main_menu::MainMenuPlugin;
use crate::map::MapPlugin;
use crate::progress::{PlayerProgress, seed_player_progress};
use crate::state::GameState;

/// Root plugin for the game: sets the clear color, initialises the top-level
/// [`GameState`], spawns the shared 2D camera, and wires in the main-menu, map,
/// game-over, character-asset, and battle plugins (the last pulls in the HUD/log
/// UI).
///
/// The game boots into [`GameState::MainMenu`]; the menu's "New Game" drops the
/// player onto the [`GameState::Map`], where a random encounter switches to
/// [`GameState::InBattle`]. Winning returns to the map (with hit points carried
/// over by [`PlayerProgress`]); losing moves to [`GameState::GameOver`]. The
/// battle systems are all gated on `InBattle`, so they spawn the combatants and
/// UI only while a fight is on screen.
pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ClearColor(Color::srgb(0.18, 0.18, 0.24)))
            .init_state::<GameState>()
            .init_resource::<PlayerProgress>()
            .add_systems(Startup, spawn_camera)
            .add_systems(Update, seed_player_progress)
            .add_plugins((
                MainMenuPlugin,
                MapPlugin,
                GameOverPlugin,
                CharactersPlugin,
                BattlePlugin,
            ));

        // The egui debug inspector (right-click a sprite to inspect it) is
        // compiled in only under the `debug-inspector` feature, so default/release
        // builds and headless tests never link egui.
        #[cfg(feature = "debug-inspector")]
        app.add_plugins(crate::debug::DebugPlugin);
    }
}

/// Spawn the single 2D camera, shared by every screen (menu, map, battle,
/// game-over), so it outlives all the state transitions (no screen spawns its
/// own).
fn spawn_camera(mut commands: Commands) {
    commands.spawn(Camera2d);
}
