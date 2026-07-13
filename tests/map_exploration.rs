//! Headless coverage for the overworld map: avatar movement, the distance-based
//! random encounter, and the HP readout.
//!
//! Mirrors the other suites' approach — a minimal `App` with the map systems and
//! the top-level [`GameState`] wired, driven by pressing keys and advancing
//! virtual time. `InputPlugin` is omitted so a manually-pressed key survives the
//! frame the `Update` systems read it, and the [`MapRng`] is pinned so the rolled
//! encounter distance — and thus when the fight starts — is deterministic.

use std::time::Duration;

use bevy::prelude::*;
use bevy::state::app::StatesPlugin;
use bevy::time::TimeUpdateStrategy;

use aliasing::map::{EncounterTracker, MapPlayer, MapRng, check_encounter, move_player, setup_map};
use aliasing::state::GameState;

/// A coarse virtual frame; a few of these cover several seconds of walking.
const STEP: Duration = Duration::from_millis(100);

/// Build a headless map app pinned to `seed`, settled into `GameState::Map` so
/// `OnEnter(Map)` has spawned the avatar and rolled the encounter distance.
fn map_app(seed: u64) -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, StatesPlugin))
        .init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<EncounterTracker>()
        .insert_resource(MapRng::from_seed(seed))
        .insert_resource(TimeUpdateStrategy::ManualDuration(STEP))
        .init_state::<GameState>()
        .add_systems(OnEnter(GameState::Map), setup_map)
        .add_systems(
            Update,
            (move_player, check_encounter)
                .chain()
                .run_if(in_state(GameState::Map)),
        );

    app.world_mut()
        .resource_mut::<NextState<GameState>>()
        .set(GameState::Map);
    app.update();
    app
}

fn current_state(app: &App) -> GameState {
    *app.world().resource::<State<GameState>>().get()
}

fn avatar_pos(app: &mut App) -> Vec2 {
    let mut q = app
        .world_mut()
        .query_filtered::<&Transform, With<MapPlayer>>();
    q.single(app.world()).unwrap().translation.truncate()
}

#[test]
fn entering_map_spawns_one_avatar_and_arms_an_encounter() {
    let mut app = map_app(1);
    let mut q = app.world_mut().query_filtered::<Entity, With<MapPlayer>>();
    assert_eq!(q.iter(app.world()).count(), 1, "exactly one map avatar");
    assert!(
        app.world().resource::<EncounterTracker>().threshold > 0.0,
        "an encounter distance is rolled on entering the map"
    );
}

#[test]
fn holding_a_key_moves_the_avatar() {
    let mut app = map_app(1);
    let start = avatar_pos(&mut app);

    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::ArrowRight);
    app.update();

    let moved = avatar_pos(&mut app);
    assert!(
        moved.x > start.x,
        "holding Right walks the avatar rightward"
    );
    assert!(
        (moved.y - start.y).abs() < f32::EPSILON,
        "no vertical drift"
    );
}

#[test]
fn walking_far_enough_starts_a_battle() {
    let mut app = map_app(1);
    assert_eq!(current_state(&app), GameState::Map);

    // Hold a direction and let virtual time advance until the accumulated travel
    // crosses the rolled threshold and the encounter fires.
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::ArrowUp);

    let mut entered_battle = false;
    for _ in 0..2000 {
        app.update();
        if current_state(&app) == GameState::InBattle {
            entered_battle = true;
            break;
        }
    }
    assert!(
        entered_battle,
        "walking past the encounter distance starts a battle"
    );
}
