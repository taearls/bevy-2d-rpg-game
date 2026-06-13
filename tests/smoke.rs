//! Headless smoke test proving the test harness works: a minimal `App`
//! with the game plugin updates once without panicking.

use bevy::asset::AssetPlugin;
use bevy::input::InputPlugin;
use bevy::prelude::*;
use bevy::sprite_render::ColorMaterial;
use bevy::state::app::StatesPlugin;
use bevy_2d_rpg_game::game::GamePlugin;

#[test]
fn app_builds_and_updates_headless() {
    let mut app = App::new();
    // `GamePlugin` needs more than `MinimalPlugins`: the `CharacterDef` asset +
    // loader require `AssetPlugin`; `BattlePlugin::init_state` requires
    // `StatesPlugin`; and `menu_input` reads `ButtonInput<KeyCode>` from
    // `InputPlugin`. All three ship inside `DefaultPlugins` in the real binary
    // but must be added explicitly here. One `update()` does not give the async
    // loader time to finish, so the spawn system (gated on a loaded roster)
    // stays dormant — this remains a pure "harness builds" check, no renderer.
    app.add_plugins((
        MinimalPlugins,
        AssetPlugin::default(),
        StatesPlugin,
        InputPlugin,
        GamePlugin,
    ))
    // `Image` must be registered so a sprite handle can be minted if the roster
    // load happens to finish within this frame — without the renderer, nothing
    // else pulls the type in. `Mesh` and `ColorMaterial` back the Phase 5
    // selection-indicator entity spawned at startup; `DefaultPlugins` registers
    // them via the 2D mesh/material plugins, so they must be added by hand here.
    .init_asset::<Image>()
    .init_asset::<Mesh>()
    .init_asset::<ColorMaterial>();
    app.update();
}
