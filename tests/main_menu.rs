//! Headless coverage for the start-up main menu: structure, keyboard
//! navigation, and confirm dispatch.
//!
//! Mirrors the `action_menu` suite's approach — a minimal `App` with the menu
//! systems and the top-level [`GameState`] wired, driven by pressing keys on the
//! `ButtonInput<KeyCode>` resource. Assertions are ECS facts: the
//! [`MainMenuSelection`] index, cursor `Visibility`, label `Text`, the
//! [`GameState`], and the presence of the [`MainMenuRoot`].
//!
//! `StatesPlugin` is added explicitly (it is not part of `MinimalPlugins`), and
//! `InputPlugin` is deliberately omitted so a manually-pressed key survives the
//! frame the `Update` systems read it.

use bevy::prelude::*;
use bevy::scene::ScenePlugin;
use bevy::state::app::StatesPlugin;

use aliasing::main_menu::{
    MainMenuCursor, MainMenuLabel, MainMenuRoot, MainMenuSelection, despawn_main_menu,
    main_menu_input, spawn_main_menu, update_main_menu_highlight,
};
use aliasing::progress::PlayerProgress;
use aliasing::state::GameState;

/// Build a headless app with the top-level state and main-menu systems wired,
/// then run one frame so the initial `OnEnter(MainMenu)` spawns the menu and
/// highlights row 0.
fn menu_app() -> App {
    let mut app = App::new();
    // `AssetPlugin` + `ScenePlugin` back the `bsn!` + `spawn_scene` the menu is
    // now built with (provided by `DefaultPlugins` in the real binary).
    app.add_plugins((
        MinimalPlugins,
        AssetPlugin::default(),
        ScenePlugin,
        StatesPlugin,
    ))
    .init_resource::<ButtonInput<KeyCode>>()
    .init_resource::<MainMenuSelection>()
    .init_resource::<PlayerProgress>()
    .init_state::<GameState>()
    .add_systems(OnEnter(GameState::MainMenu), spawn_main_menu)
    .add_systems(OnExit(GameState::MainMenu), despawn_main_menu)
    .add_systems(
        Update,
        (main_menu_input, update_main_menu_highlight).run_if(in_state(GameState::MainMenu)),
    );

    app.update();
    app
}

/// Press `key`, run a frame so `main_menu_input` sees the `just_pressed` edge,
/// then release/reset, then run a settling frame so any queued `GameState`
/// transition (and its `OnEnter`/`OnExit` systems) applies before we assert.
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

fn selection(app: &App) -> Option<usize> {
    app.world().resource::<MainMenuSelection>().highlighted
}

fn current_state(app: &App) -> GameState {
    *app.world().resource::<State<GameState>>().get()
}

/// Collect the (index, text) of every option label, sorted by row index.
fn labels(app: &mut App) -> Vec<(usize, String)> {
    let mut found: Vec<(usize, String)> = app
        .world_mut()
        .query::<(&MainMenuLabel, &Text)>()
        .iter(app.world())
        .map(|(label, text)| (label.0, text.0.clone()))
        .collect();
    found.sort_by_key(|(index, _)| *index);
    found
}

#[test]
fn menu_has_three_rows_with_expected_labels() {
    let mut app = menu_app();
    assert_eq!(
        labels(&mut app),
        vec![
            (0, "New Game".to_string()),
            (1, "Options".to_string()),
            (2, "Credits".to_string()),
        ]
    );
}

#[test]
fn row_zero_highlighted_on_enter() {
    let app = menu_app();
    assert_eq!(selection(&app), Some(0));
}

#[test]
fn cursor_visible_on_exactly_one_row() {
    let mut app = menu_app();
    let visible: Vec<usize> = app
        .world_mut()
        .query::<(&MainMenuCursor, &Visibility)>()
        .iter(app.world())
        .filter(|(_, vis)| !matches!(vis, Visibility::Hidden))
        .map(|(cursor, _)| cursor.0)
        .collect();
    assert_eq!(
        visible,
        vec![0],
        "only the highlighted row shows the cursor"
    );
}

#[test]
fn arrow_down_cycles_forward_with_wrap() {
    let mut app = menu_app();
    assert_eq!(selection(&app), Some(0));

    press(&mut app, KeyCode::ArrowDown);
    assert_eq!(selection(&app), Some(1));

    press(&mut app, KeyCode::ArrowDown);
    assert_eq!(selection(&app), Some(2));

    // Wrap past the last row back to the first.
    press(&mut app, KeyCode::ArrowDown);
    assert_eq!(selection(&app), Some(0));
}

#[test]
fn arrow_up_wraps_backward() {
    let mut app = menu_app();
    press(&mut app, KeyCode::ArrowUp);
    assert_eq!(selection(&app), Some(2), "up from row 0 wraps to last");
}

#[test]
fn new_game_enters_map_and_tears_down_the_menu() {
    let mut app = menu_app();
    // Row 0 = New Game is highlighted on entry.
    press(&mut app, KeyCode::Enter);

    assert_eq!(
        current_state(&app),
        GameState::Map,
        "New Game drops the player onto the overworld map"
    );
    let roots = app
        .world_mut()
        .query_filtered::<Entity, With<MainMenuRoot>>()
        .iter(app.world())
        .count();
    assert_eq!(roots, 0, "OnExit(MainMenu) despawns the menu UI");
}

#[test]
fn options_stays_on_the_menu() {
    let mut app = menu_app();
    press(&mut app, KeyCode::ArrowDown); // → Options (row 1)
    press(&mut app, KeyCode::Enter);

    assert_eq!(
        current_state(&app),
        GameState::MainMenu,
        "Options is non-functional and leaves the player on the menu"
    );
}

#[test]
fn credits_stays_on_the_menu() {
    let mut app = menu_app();
    press(&mut app, KeyCode::ArrowUp); // wrap up to Credits (row 2)
    press(&mut app, KeyCode::Enter);

    assert_eq!(
        current_state(&app),
        GameState::MainMenu,
        "Credits is non-functional and leaves the player on the menu"
    );
}
