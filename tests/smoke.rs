//! Headless smoke test proving the test harness works: a minimal `App`
//! with the game plugin updates once without panicking.

use bevy::asset::AssetPlugin;
use bevy::prelude::*;
use bevy_2d_rpg_game::game::GamePlugin;

#[test]
fn app_builds_and_updates_headless() {
    let mut app = App::new();
    // `GamePlugin` registers the `CharacterDef` asset + loader, so the asset
    // infrastructure must be present. One `update()` does not give the async
    // loader time to finish, so the spawn system (gated on a loaded roster)
    // stays dormant — this remains a pure "harness builds" check, no renderer.
    app.add_plugins((MinimalPlugins, AssetPlugin::default(), GamePlugin));
    app.update();
}
