//! Headless coverage for leaving a finished battle: a victory returns to the map
//! with the player's surviving health persisted, a defeat goes to the game-over
//! screen.
//!
//! Builds on the `battle_flow` harness — the real turn-flow systems wired into one
//! `App` — but adds the top-level [`GameState`], [`PlayerProgress`], and the
//! production [`battle_over_input`] so the `BattleOver → Map / GameOver` hand-off
//! is exercised end to end. Stats are injected directly (no asset loader) and the
//! `BattleResult` is read from the real `check_battle_end`, so the only thing
//! these tests drive by hand is the player's Enter press on the result screen.

use std::time::Duration;

use bevy::prelude::*;
use bevy::state::app::StatesPlugin;
use bevy::time::TimeUpdateStrategy;

use bevy_2d_rpg_game::battle::battle_over_input;
use bevy_2d_rpg_game::battle::enemy_turn::{EnemyTurnQueue, on_enter_enemy_turn, tick_enemy_turn};
use bevy_2d_rpg_game::battle::menu::{LogView, MenuSelection, menu_input, on_enter_player_turn};
use bevy_2d_rpg_game::battle::messages::LogMessage;
use bevy_2d_rpg_game::battle::rng::DamageRng;
use bevy_2d_rpg_game::battle::state::{BattleResult, BattleSet, TurnPhase};
use bevy_2d_rpg_game::battle::targeting::{
    SelectedTarget, on_enter_targeting, on_exit_targeting, targeting_input,
};
use bevy_2d_rpg_game::combat::events::{AttackRequested, DamageDealt};
use bevy_2d_rpg_game::combat::resolve::{apply_attacks, check_battle_end, on_died_hide_sprite};
use bevy_2d_rpg_game::components::{
    CombatStats, DamageVariance, DisplayName, Enemy, Health, Player,
};
use bevy_2d_rpg_game::progress::PlayerProgress;
use bevy_2d_rpg_game::state::GameState;

const STEP: Duration = Duration::from_millis(100);

/// Wire the full battle loop *plus* the top-level state and the result hand-off.
/// The player one-shots the lone enemy (`player_attack` ≥ enemy HP) while the
/// enemy chips `enemy_attack` off the player, so the outcome is controlled by the
/// stats the caller passes.
fn outcome_app(player_hp: i32, player_attack: i32, enemy_hp: i32, enemy_attack: i32) -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, StatesPlugin))
        .init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<MenuSelection>()
        .init_resource::<LogView>()
        .init_resource::<SelectedTarget>()
        .init_resource::<EnemyTurnQueue>()
        .init_resource::<BattleResult>()
        .init_resource::<PlayerProgress>()
        .init_state::<GameState>()
        .init_state::<TurnPhase>()
        .add_message::<LogMessage>()
        .add_message::<AttackRequested>()
        .add_message::<DamageDealt>()
        .insert_resource(DamageRng::from_seed(0))
        .insert_resource(TimeUpdateStrategy::ManualDuration(STEP))
        .add_observer(on_died_hide_sprite)
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
        .add_systems(OnEnter(TurnPhase::PlayerTurn), on_enter_player_turn)
        .add_systems(OnEnter(TurnPhase::Targeting), on_enter_targeting)
        .add_systems(OnExit(TurnPhase::Targeting), on_exit_targeting)
        .add_systems(OnEnter(TurnPhase::EnemyTurn), on_enter_enemy_turn)
        .add_systems(
            Update,
            (
                menu_input
                    .in_set(BattleSet::Input)
                    .run_if(in_state(TurnPhase::PlayerTurn)),
                targeting_input
                    .in_set(BattleSet::Input)
                    .run_if(in_state(TurnPhase::Targeting)),
                tick_enemy_turn
                    .in_set(BattleSet::Input)
                    .run_if(in_state(TurnPhase::EnemyTurn)),
                battle_over_input.run_if(in_state(TurnPhase::BattleOver)),
                apply_attacks.in_set(BattleSet::Resolve),
                check_battle_end
                    .in_set(BattleSet::Cleanup)
                    .run_if(on_message::<DamageDealt>),
            ),
        );

    // Start in a battle (so `battle_over_input`'s real gate, `in_state(InBattle)`,
    // is exercised) on the player's turn.
    app.world_mut()
        .resource_mut::<NextState<GameState>>()
        .set(GameState::InBattle);

    app.world_mut().spawn((
        Player,
        DisplayName("Hero".to_string()),
        CombatStats {
            attack: player_attack,
            defense: 0,
        },
        DamageVariance { min: 1.0, max: 1.0 },
        Health::full(player_hp),
    ));
    app.world_mut().spawn((
        Enemy { index: 0 },
        DisplayName("Goblin".to_string()),
        CombatStats {
            attack: enemy_attack,
            defense: 0,
        },
        DamageVariance { min: 1.0, max: 1.0 },
        Health::full(enemy_hp),
        Visibility::Visible,
    ));

    app.world_mut()
        .resource_mut::<NextState<TurnPhase>>()
        .set(TurnPhase::PlayerTurn);
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

fn game_state(app: &App) -> GameState {
    *app.world().resource::<State<GameState>>().get()
}

fn turn_phase(app: &App) -> TurnPhase {
    *app.world().resource::<State<TurnPhase>>().get()
}

#[test]
fn victory_returns_to_map_and_persists_health() {
    // Player one-shots the enemy (60 ≥ 50 HP); the enemy never gets to swing, so
    // the player should finish at full health and that total should persist.
    let mut app = outcome_app(100, 60, 50, 10);

    press(&mut app, KeyCode::Enter); // Fight → Targeting
    press(&mut app, KeyCode::Enter); // confirm → kill enemy → BattleOver (victory)
    assert_eq!(turn_phase(&app), TurnPhase::BattleOver);
    assert!(
        app.world().resource::<BattleResult>().victory,
        "clearing the enemy is a victory"
    );

    // Acknowledge the result: Enter returns to the map and stores the health.
    press(&mut app, KeyCode::Enter);
    assert_eq!(game_state(&app), GameState::Map, "a win returns to the map");
    assert_eq!(
        app.world().resource::<PlayerProgress>().health,
        Some(Health {
            current: 100,
            max: 100
        }),
        "the surviving health is carried over"
    );
}

#[test]
fn defeat_goes_to_game_over() {
    // The player cannot kill the enemy in one hit (5 < 500 HP) and is so frail
    // (1 HP vs the enemy's 50 attack) that the enemy's swing is lethal.
    let mut app = outcome_app(1, 5, 500, 50);

    press(&mut app, KeyCode::Enter); // Fight → Targeting
    press(&mut app, KeyCode::Enter); // confirm → enemy turn

    // Let the enemy's attack land and the battle resolve to defeat.
    for _ in 0..5 {
        app.update();
        if turn_phase(&app) == TurnPhase::BattleOver {
            break;
        }
    }
    assert_eq!(turn_phase(&app), TurnPhase::BattleOver);
    assert!(
        !app.world().resource::<BattleResult>().victory,
        "the player falling is a defeat"
    );

    // Acknowledge the result: Enter moves to the game-over screen.
    press(&mut app, KeyCode::Enter);
    assert_eq!(
        game_state(&app),
        GameState::GameOver,
        "a loss goes to the game-over screen"
    );
}
