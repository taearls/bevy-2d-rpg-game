//! Headless turn-state + action-menu coverage, mirroring the Godot
//! `ActionMenuTest` and the menu-related `BattleSceneTest` cases.
//!
//! These build a minimal `App` with the turn-state machine and menu systems
//! wired, then drive it by pressing keys on the `ButtonInput<KeyCode>` resource
//! and calling `app.update()` — exactly how the issue specifies simulating
//! input. Assertions are ECS facts: the [`MenuSelection`] index, cursor
//! `Visibility`, label `Text`, the [`TurnPhase`] state, drained [`LogMessage`]s,
//! and the presence of the [`Defending`] marker.
//!
//! `StatesPlugin` is added explicitly: it is NOT part of `MinimalPlugins`, so
//! without it `init_state` / `OnEnter` transitions never run.

use bevy::prelude::*;
use bevy::state::app::StatesPlugin;

use bevy_2d_rpg_game::battle::menu::{
    ActionMenuPanel, CycleDirection, MenuCursor, MenuLabel, MenuRow, MenuSelection, cycle_index,
    spawn_action_menu,
};
use bevy_2d_rpg_game::battle::messages::LogMessage;
use bevy_2d_rpg_game::battle::state::{BattleSet, TurnPhase};
use bevy_2d_rpg_game::characters::components::{
    CombatStats, DamageVariance, Defending, DisplayName, Health, Player,
};

use bevy_2d_rpg_game::battle::menu::{menu_input, on_enter_player_turn, update_menu_highlight};

/// Build a headless battle app with the turn state, action menu, and a player
/// entity named `player_name`. No renderer or asset loading — the menu UI is
/// plain `Node`/`Text` entities the systems mutate in place.
fn menu_app(player_name: &str) -> App {
    let mut app = App::new();
    // `StatesPlugin` is not part of `MinimalPlugins` and is required for
    // `init_state` / `OnEnter` transitions to run. We deliberately do NOT add
    // `InputPlugin`: its `PreUpdate` keyboard system overwrites and clears
    // `ButtonInput<KeyCode>` from `KeyboardInput` events every frame, which would
    // wipe a manually-pressed key before `menu_input` (in `Update`) reads it.
    // Inserting the resource directly and managing it in `press` keeps the
    // simulated press alive for the frame the system runs.
    app.add_plugins((MinimalPlugins, StatesPlugin))
        .init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<MenuSelection>()
        .init_state::<TurnPhase>()
        .add_message::<LogMessage>()
        .configure_sets(
            Update,
            (
                BattleSet::Input,
                BattleSet::Resolve,
                BattleSet::Cleanup,
                BattleSet::Ui,
            )
                .chain(),
        )
        .add_systems(Startup, spawn_action_menu)
        .add_systems(OnEnter(TurnPhase::PlayerTurn), on_enter_player_turn)
        .add_systems(
            Update,
            (
                menu_input
                    .in_set(BattleSet::Input)
                    .run_if(in_state(TurnPhase::PlayerTurn)),
                update_menu_highlight.in_set(BattleSet::Ui),
            ),
        );

    app.world_mut().spawn((
        Player,
        DisplayName(player_name.to_string()),
        Health::full(100),
        CombatStats {
            attack: 10,
            defense: 5,
        },
        DamageVariance::default(),
    ));

    // One update runs Startup (spawning the menu) and the initial
    // `OnEnter(PlayerTurn)` transition (highlighting row 0).
    app.update();
    app
}

/// Press `key`, run one frame so `menu_input` observes the `just_pressed` edge,
/// then release so the next press of the same key registers a fresh edge.
///
/// Without `InputPlugin` nothing manages the input for us. `release` is required
/// (not just `clear`): `press` only raises a `just_pressed` edge for a key that
/// was not already held, so the key must be released between simulated presses.
/// `reset` then drops the lingering `just_released` edge so it can't be observed
/// next frame.
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
    // A confirm queues a `NextState<TurnPhase>`; the `StateTransition` schedule
    // applies it on the *following* `App::update`. Run one input-free frame so
    // the new phase (and its `OnEnter` systems) is in effect when we assert.
    // Harmless for navigation presses, which queue no transition.
    app.update();
}

fn selection(app: &mut App) -> Option<usize> {
    app.world().resource::<MenuSelection>().highlighted
}

fn current_phase(app: &mut App) -> TurnPhase {
    *app.world().resource::<State<TurnPhase>>().get()
}

/// Collect the (index, text) of every menu label, sorted by row index.
fn labels(app: &mut App) -> Vec<(usize, String)> {
    let mut found: Vec<(usize, String)> = app
        .world_mut()
        .query::<(&MenuLabel, &Text)>()
        .iter(app.world())
        .map(|(label, text)| (label.0, text.0.clone()))
        .collect();
    found.sort_by_key(|(index, _)| *index);
    found
}

/// Drain the pending `LogMessage`s into plain strings.
fn drain_log(app: &mut App) -> Vec<String> {
    let messages = app.world_mut().resource_mut::<Messages<LogMessage>>();
    let mut cursor = messages.get_cursor();
    cursor
        .read(&messages)
        .map(|LogMessage(text)| text.clone())
        .collect()
}

// --- cycle_index parity (mirrors ActionMenuTest cycling cases) ---

#[test]
fn cycle_index_matches_godot_wrap_semantics() {
    // forward, backward, wrap both ways, from-unhighlighted → 0, single item.
    assert_eq!(cycle_index(Some(0), CycleDirection::Down, 4), 1);
    assert_eq!(cycle_index(Some(3), CycleDirection::Down, 4), 0);
    assert_eq!(cycle_index(Some(0), CycleDirection::Up, 4), 3);
    assert_eq!(cycle_index(None, CycleDirection::Down, 4), 0);
    assert_eq!(cycle_index(None, CycleDirection::Up, 4), 0);
    assert_eq!(cycle_index(Some(0), CycleDirection::Down, 1), 0);
}

// --- Menu structure ---

#[test]
fn menu_has_four_rows_with_expected_labels() {
    let mut app = menu_app("Hero");

    let panels = app
        .world_mut()
        .query_filtered::<Entity, With<ActionMenuPanel>>()
        .iter(app.world())
        .count();
    assert_eq!(panels, 1, "exactly one action-menu panel");

    let rows = app
        .world_mut()
        .query::<&MenuRow>()
        .iter(app.world())
        .count();
    assert_eq!(rows, 4, "four selectable rows");

    assert_eq!(
        labels(&mut app),
        vec![
            (0, "Fight".to_string()),
            (1, "Items".to_string()),
            (2, "Defend".to_string()),
            (3, "Flee".to_string()),
        ]
    );
}

#[test]
fn row_zero_highlighted_on_player_turn_start() {
    let mut app = menu_app("Hero");
    assert_eq!(selection(&mut app), Some(0));
}

#[test]
fn cursor_visible_on_exactly_one_row() {
    let mut app = menu_app("Hero");

    let visible: Vec<usize> = app
        .world_mut()
        .query::<(&MenuCursor, &Visibility)>()
        .iter(app.world())
        .filter(|(_, vis)| matches!(vis, Visibility::Visible))
        .map(|(cursor, _)| cursor.0)
        .collect();

    assert_eq!(
        visible,
        vec![0],
        "only the highlighted row shows the cursor"
    );
}

// --- Keyboard navigation ---

#[test]
fn arrow_down_cycles_forward_with_wrap() {
    let mut app = menu_app("Hero");
    assert_eq!(selection(&mut app), Some(0));

    press(&mut app, KeyCode::ArrowDown);
    assert_eq!(selection(&mut app), Some(1));

    press(&mut app, KeyCode::ArrowDown);
    press(&mut app, KeyCode::ArrowDown);
    assert_eq!(selection(&mut app), Some(3));

    // Wrap past the last row back to the first.
    press(&mut app, KeyCode::ArrowDown);
    assert_eq!(selection(&mut app), Some(0));
}

#[test]
fn arrow_up_wraps_backward() {
    let mut app = menu_app("Hero");
    assert_eq!(selection(&mut app), Some(0));

    press(&mut app, KeyCode::ArrowUp);
    assert_eq!(selection(&mut app), Some(3), "up from row 0 wraps to last");
}

#[test]
fn cursor_follows_highlight_after_navigation() {
    let mut app = menu_app("Hero");
    press(&mut app, KeyCode::ArrowDown);
    press(&mut app, KeyCode::ArrowDown);
    assert_eq!(selection(&mut app), Some(2));

    let visible: Vec<usize> = app
        .world_mut()
        .query::<(&MenuCursor, &Visibility)>()
        .iter(app.world())
        .filter(|(_, vis)| matches!(vis, Visibility::Visible))
        .map(|(cursor, _)| cursor.0)
        .collect();
    assert_eq!(visible, vec![2]);
}

// --- Confirm dispatch (mirrors BattleSceneTest menu action cases) ---

#[test]
fn fight_enters_targeting() {
    let mut app = menu_app("Hero");
    // Row 0 = Fight is highlighted at turn start.
    press(&mut app, KeyCode::Enter);
    assert_eq!(current_phase(&mut app), TurnPhase::Targeting);
}

#[test]
fn items_logs_and_ends_player_turn() {
    let mut app = menu_app("Hero");
    press(&mut app, KeyCode::ArrowDown); // → Items (row 1)
    press(&mut app, KeyCode::Enter);

    assert_eq!(current_phase(&mut app), TurnPhase::EnemyTurn);
    assert_eq!(drain_log(&mut app), vec!["Hero uses an item!".to_string()]);
}

#[test]
fn defend_inserts_marker_queues_message_and_ends_turn() {
    let mut app = menu_app("Hero");
    press(&mut app, KeyCode::ArrowDown);
    press(&mut app, KeyCode::ArrowDown); // → Defend (row 2)
    press(&mut app, KeyCode::Enter);

    assert_eq!(current_phase(&mut app), TurnPhase::EnemyTurn);
    assert_eq!(drain_log(&mut app), vec!["Hero is defending!".to_string()]);

    let defenders = app
        .world_mut()
        .query_filtered::<Entity, (With<Player>, With<Defending>)>()
        .iter(app.world())
        .count();
    assert_eq!(
        defenders, 1,
        "Defend inserts the Defending marker on the player"
    );
}

#[test]
fn flee_logs_and_ends_player_turn() {
    let mut app = menu_app("Hero");
    press(&mut app, KeyCode::ArrowUp); // wrap up to Flee (row 3)
    press(&mut app, KeyCode::Enter);

    assert_eq!(current_phase(&mut app), TurnPhase::EnemyTurn);
    assert_eq!(
        drain_log(&mut app),
        vec!["Hero attempts to flee!".to_string()]
    );
}

// --- Defending lifecycle ---

#[test]
fn defending_marker_cleared_on_reentering_player_turn() {
    let mut app = menu_app("Hero");
    // Defend → EnemyTurn, marker present.
    press(&mut app, KeyCode::ArrowDown);
    press(&mut app, KeyCode::ArrowDown);
    press(&mut app, KeyCode::Enter);
    assert_eq!(current_phase(&mut app), TurnPhase::EnemyTurn);

    // Return to the player turn: OnEnter clears Defending and re-highlights row 0.
    app.world_mut()
        .resource_mut::<NextState<TurnPhase>>()
        .set(TurnPhase::PlayerTurn);
    app.update();

    assert_eq!(current_phase(&mut app), TurnPhase::PlayerTurn);
    assert_eq!(selection(&mut app), Some(0));
    let defenders = app
        .world_mut()
        .query_filtered::<Entity, With<Defending>>()
        .iter(app.world())
        .count();
    assert_eq!(defenders, 0, "Defending is removed OnEnter(PlayerTurn)");
}

#[test]
fn input_ignored_outside_player_turn() {
    let mut app = menu_app("Hero");
    // Leave the player turn via Fight.
    press(&mut app, KeyCode::Enter);
    assert_eq!(current_phase(&mut app), TurnPhase::Targeting);

    // Navigation keys must not change the selection while in Targeting.
    let before = selection(&mut app);
    press(&mut app, KeyCode::ArrowDown);
    assert_eq!(
        selection(&mut app),
        before,
        "menu input is gated to PlayerTurn"
    );
}
