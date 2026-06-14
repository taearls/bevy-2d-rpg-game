//! Headless smoke test for the full battle plugin stack — the Bevy analogue of
//! the Godot `BattleTscnSmokeTest`.
//!
//! Builds an `App` with the real [`GamePlugin`] (which pulls in the character,
//! battle, and HUD/log plugins), pins the spawn RNG to a fixed seed, lets the
//! async `*.character.ron` loader finish, then runs ten frames and asserts the
//! whole stack survived: the player and at least one enemy actually spawned, and
//! nothing panicked along the way. No renderer is involved — only the asset,
//! state, and input infrastructure the plugins need.

use bevy::asset::AssetPlugin;
use bevy::input::InputPlugin;
use bevy::prelude::*;
use bevy::sprite_render::ColorMaterial;
use bevy::state::app::StatesPlugin;
use bevy_2d_rpg_game::battle::rng::SpawnRng;
use bevy_2d_rpg_game::characters::components::{Enemy, Player};
use bevy_2d_rpg_game::game::GamePlugin;

/// Build the headless battle app on a fixed seed.
///
/// `GamePlugin` needs more than `MinimalPlugins`: the `CharacterDef` asset +
/// loader require `AssetPlugin`; `BattlePlugin::init_state` requires
/// `StatesPlugin`; and the menu/targeting input systems read
/// `ButtonInput<KeyCode>` from `InputPlugin`. All three ship inside
/// `DefaultPlugins` in the real binary but must be added explicitly here. The
/// `Image`/`Mesh`/`ColorMaterial` assets back sprites and the selection
/// indicator; without the renderer nothing else registers them.
///
/// The caller pins the `SpawnRng` *after* the first `update()` (see
/// [`run_until_spawned`]): `load_roster` runs at `Startup` and inserts an
/// entropy-seeded `SpawnRng`, so overriding it before plugins build would just be
/// clobbered. Overriding it once Startup has run — but before the async roster
/// finishes loading and `spawn_battle` reads it — makes the roll deterministic.
fn battle_app() -> App {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        AssetPlugin::default(),
        StatesPlugin,
        InputPlugin,
        GamePlugin,
    ))
    .init_asset::<Image>()
    .init_asset::<Mesh>()
    .init_asset::<ColorMaterial>();
    app
}

/// Drive the app until the spawn system has run (player present) or `max_frames`
/// elapse, so the async roster load has time to finish before we assert. Returns
/// the number of frames actually run.
fn run_until_spawned(app: &mut App, max_frames: usize) -> usize {
    for frame in 1..=max_frames {
        app.update();
        let mut players = app.world_mut().query_filtered::<Entity, With<Player>>();
        if players.iter(app.world()).next().is_some() {
            return frame;
        }
    }
    max_frames
}

/// The full plugin stack builds, loads its roster, spawns a seeded battle, and
/// survives ten frames without panicking — with a player and at least one enemy
/// in the world afterwards.
#[test]
fn full_stack_spawns_seeded_battle_and_runs_ten_frames() {
    let mut app = battle_app();

    // First frame runs `Startup` (incl. `load_roster`, which inserts an entropy
    // `SpawnRng`); pin a fixed seed afterwards so the roster roll is deterministic
    // when `spawn_battle` later reads it. The async load is still in flight, so
    // the spawn has not consumed the RNG yet.
    app.update();
    app.insert_resource(SpawnRng::from_seed(42));

    // The async `*.character.ron` loader needs a few frames before the
    // roster-ready gate opens and `spawn_battle` runs; give it generous headroom.
    let frames_to_spawn = run_until_spawned(&mut app, 50);

    let mut players = app.world_mut().query_filtered::<Entity, With<Player>>();
    assert_eq!(
        players.iter(app.world()).count(),
        1,
        "exactly one player should spawn within {frames_to_spawn} frames"
    );
    let mut enemies = app.world_mut().query_filtered::<Entity, With<Enemy>>();
    let enemy_count = enemies.iter(app.world()).count();
    assert!(
        (1..=bevy_2d_rpg_game::battle::spawn::MAX_ENEMIES).contains(&enemy_count),
        "seed 42 should roll 1..=MAX_ENEMIES enemies, got {enemy_count}"
    );

    // Ten more frames of the live battle loop must not panic.
    for _ in 0..10 {
        app.update();
    }
}

/// `DebugPlugin` is a no-op without a `RenderApp`: on a headless `MinimalPlugins`
/// app it must build and run frames without pulling in `EguiPlugin` (which needs
/// the renderer) and without panicking. This locks in the `get_sub_app(RenderApp)`
/// early-return that keeps `cargo test --features debug-inspector` green; only
/// compiled when the feature is on, since `DebugPlugin` is otherwise absent.
#[cfg(feature = "debug-inspector")]
#[test]
fn debug_plugin_is_noop_when_headless() {
    use bevy_2d_rpg_game::debug::DebugPlugin;

    let mut app = App::new();
    app.add_plugins((MinimalPlugins, DebugPlugin));
    // A few frames with no renderer present must not panic.
    for _ in 0..3 {
        app.update();
    }
}
