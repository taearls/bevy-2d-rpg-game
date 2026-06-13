//! Headless smoke test proving the test harness works: a minimal `App`
//! with the game plugin updates once without panicking.

use bevy::prelude::*;
use bevy_2d_rpg_game::game::GamePlugin;

#[test]
fn app_builds_and_updates_headless() {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, GamePlugin));
    app.update();
}
