//! Headless coverage for the game-over screen: structure, navigation, and the
//! two confirm dispatches (Restart Game / Return to Title Screen).
//!
//! Mirrors the `main_menu` suite — a minimal `App` with the game-over systems,
//! the top-level [`GameState`], and [`PlayerProgress`] wired, driven by pressing
//! keys. `InputPlugin` is omitted so a manually-pressed key survives the frame
//! the `Update` systems read it.

use bevy::prelude::*;
use bevy::state::app::StatesPlugin;

use bevy_2d_rpg_game::components::Health;
use bevy_2d_rpg_game::game_over::{
    GameOverLabel, GameOverRoot, GameOverSelection, game_over_input, spawn_game_over,
    update_game_over_highlight,
};
use bevy_2d_rpg_game::progress::PlayerProgress;
use bevy_2d_rpg_game::state::GameState;

/// Build a headless app sitting on the game-over screen, reached by transitioning
/// from a (defeat) battle so `OnEnter(GameOver)` has built the menu. Seeds
/// [`PlayerProgress`] with a damaged total so the restart-resets-it assertion is
/// meaningful.
fn game_over_app() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, StatesPlugin))
        .init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<GameOverSelection>()
        .insert_resource(PlayerProgress {
            health: Some(Health {
                current: 0,
                max: 120,
            }),
        })
        .init_state::<GameState>()
        .add_systems(OnEnter(GameState::GameOver), spawn_game_over)
        .add_systems(
            Update,
            (game_over_input, update_game_over_highlight).run_if(in_state(GameState::GameOver)),
        );

    app.world_mut()
        .resource_mut::<NextState<GameState>>()
        .set(GameState::GameOver);
    app.update();
    app
}

fn press(app: &mut App, key: KeyCode) {
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(key);
    app.update();
    {
        let mut input = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        input.release(key);
        input.reset(key);
    }
    app.update();
}

fn current_state(app: &App) -> GameState {
    *app.world().resource::<State<GameState>>().get()
}

fn labels(app: &mut App) -> Vec<(usize, String)> {
    let mut found: Vec<(usize, String)> = app
        .world_mut()
        .query::<(&GameOverLabel, &Text)>()
        .iter(app.world())
        .map(|(label, text)| (label.0, text.0.clone()))
        .collect();
    found.sort_by_key(|(index, _)| *index);
    found
}

#[test]
fn screen_has_expected_rows() {
    let mut app = game_over_app();
    assert_eq!(
        labels(&mut app),
        vec![
            (0, "Restart Game".to_string()),
            (1, "Return to Title Screen".to_string()),
        ]
    );
}

#[test]
fn restart_resets_health_and_returns_to_map() {
    let mut app = game_over_app();
    // Row 0 = Restart Game is highlighted on entry.
    press(&mut app, KeyCode::Enter);

    assert_eq!(
        current_state(&app),
        GameState::Map,
        "Restart drops the player back onto the map"
    );
    assert_eq!(
        app.world().resource::<PlayerProgress>().health,
        None,
        "Restart clears the carried-over health so the player respawns at full"
    );
    let roots = app
        .world_mut()
        .query_filtered::<Entity, With<GameOverRoot>>()
        .iter(app.world())
        .count();
    assert_eq!(roots, 0, "DespawnOnExit(GameOver) tears the screen down");
}

#[test]
fn return_to_title_goes_to_main_menu() {
    let mut app = game_over_app();
    press(&mut app, KeyCode::ArrowDown); // → Return to Title Screen (row 1)
    press(&mut app, KeyCode::Enter);

    assert_eq!(
        current_state(&app),
        GameState::MainMenu,
        "Return to Title Screen goes back to the main menu"
    );
}
