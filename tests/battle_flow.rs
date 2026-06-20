//! Full-loop battle orchestration, the Bevy analogue of the Godot
//! `BattleSceneTest` end-to-end round-trip.
//!
//! Where the per-phase suites (`action_menu`, `targeting`, `enemy_turn`) each
//! exercise one slice in isolation, this test wires the **real** systems of all
//! three phases into one `App` and drives a complete turn cycle by pressing keys
//! and advancing virtual time:
//!
//! `PlayerTurn` → Fight (Enter) → `Targeting` → confirm (Enter) → player attack
//! resolves → `EnemyTurn` → each enemy attacks on the timed queue → queue empties
//! → back to `PlayerTurn`.
//!
//! Entities are spawned directly (no async asset loader) so the loop is
//! deterministic and renderer-free, exactly as the targeting suite does. Virtual
//! time is driven by [`TimeUpdateStrategy::ManualDuration`] so the enemy-turn
//! gaps are exact. The systems under test are the production ones — `menu_input`,
//! `targeting_input`, `tick_enemy_turn`, `apply_attacks`, `check_battle_end` —
//! assembled with the same `BattleSet` chain and state wiring as `battle::plugin`.

use std::time::Duration;

use bevy::prelude::*;
use bevy::state::app::StatesPlugin;
use bevy::time::TimeUpdateStrategy;

use bevy_2d_rpg_game::battle::enemy_turn::{
    ATTACK_INTERVAL, EnemyTurnQueue, on_enter_enemy_turn, tick_enemy_turn,
};
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

/// A 100 ms virtual frame — fine-grained enough that the enemy-turn interval
/// spans many of them.
const STEP: Duration = Duration::from_millis(100);

/// Build the full battle app starting in `PlayerTurn`, wiring every phase's real
/// systems. Spawns one player (attack 50) and `enemy_count` enemies (attack 10,
/// 200 HP each so a single player blow never wins outright — we want the loop to
/// reach the enemy turn). Returns the app, the enemy entities, and the player.
fn battle_app(enemy_count: usize) -> (App, Vec<Entity>, Entity) {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, StatesPlugin))
        .init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<MenuSelection>()
        .init_resource::<LogView>()
        .init_resource::<SelectedTarget>()
        .init_resource::<EnemyTurnQueue>()
        .init_resource::<BattleResult>()
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
                apply_attacks.in_set(BattleSet::Resolve),
                check_battle_end
                    .in_set(BattleSet::Cleanup)
                    .run_if(on_message::<DamageDealt>),
            ),
        );

    let player = app
        .world_mut()
        .spawn((
            Player,
            DisplayName("Hero".to_string()),
            CombatStats {
                attack: 50,
                defense: 0,
            },
            DamageVariance { min: 1.0, max: 1.0 },
            Health::full(100),
        ))
        .id();

    let enemies: Vec<Entity> = (0..enemy_count)
        .map(|index| {
            app.world_mut()
                .spawn((
                    Enemy { index },
                    DisplayName(format!("Goblin {index}")),
                    CombatStats {
                        attack: 10,
                        defense: 0,
                    },
                    DamageVariance { min: 1.0, max: 1.0 },
                    // High HP so one player attack can't clear the row — the loop
                    // must pass through the enemy turn.
                    Health::full(200),
                    Visibility::Visible,
                ))
                .id()
        })
        .collect();

    // Settle into PlayerTurn so `OnEnter(PlayerTurn)` highlights row 0 (Fight).
    app.world_mut()
        .resource_mut::<NextState<TurnPhase>>()
        .set(TurnPhase::PlayerTurn);
    app.update();

    (app, enemies, player)
}

/// Press `key` for one frame (so the `just_pressed` edge is seen), then release
/// and reset, then run a settling frame so any queued state transition applies.
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

fn current_phase(app: &App) -> TurnPhase {
    *app.world().resource::<State<TurnPhase>>().get()
}

fn health_of(app: &App, entity: Entity) -> i32 {
    app.world().entity(entity).get::<Health>().unwrap().current
}

/// Advance virtual time by `count` frames.
fn advance(app: &mut App, count: u32) {
    for _ in 0..count {
        app.update();
    }
}

/// Frames that just exceed one `ATTACK_INTERVAL` (10 at STEP = 100 ms).
fn steps_per_interval() -> u32 {
    (ATTACK_INTERVAL.as_millis() as u32).div_ceil(STEP.as_millis() as u32)
}

/// One full cycle round-trips through every phase and lands back in `PlayerTurn`
/// with both sides having taken their hits — the `BattleSceneTest` orchestration
/// equivalent.
#[test]
fn full_round_trips_player_turn_to_enemy_turn_and_back() {
    let (mut app, enemies, player) = battle_app(2);
    assert_eq!(current_phase(&app), TurnPhase::PlayerTurn);

    // Fight is row 0 (highlighted by OnEnter): Enter confirms → Targeting.
    press(&mut app, KeyCode::Enter);
    assert_eq!(current_phase(&app), TurnPhase::Targeting);

    // Enter again confirms the attack on the first enemy. The attack resolves
    // this frame; `check_battle_end` (enemies remain) routes us to EnemyTurn.
    press(&mut app, KeyCode::Enter);
    assert_eq!(current_phase(&app), TurnPhase::EnemyTurn);
    assert_eq!(
        health_of(&app, enemies[0]),
        200 - 50,
        "the player's 50-damage attack landed on enemy 0"
    );

    // Enemy turn: the first enemy attacks immediately on entering the turn, the
    // second a full interval later. Advance enough frames to drain both.
    advance(&mut app, steps_per_interval() + 1);
    assert_eq!(
        health_of(&app, player),
        100 - 20,
        "both enemies hit the player for 10 each"
    );

    // Queue drained → the turn hands back. A couple of settling frames let the
    // empty-queue tick set `PlayerTurn` and the transition apply.
    advance(&mut app, 2);
    assert_eq!(
        current_phase(&app),
        TurnPhase::PlayerTurn,
        "the cycle returns to the player"
    );
}

/// A single-enemy battle still completes the loop: Fight → confirm → the lone
/// enemy attacks once → back to `PlayerTurn`.
#[test]
fn single_enemy_round_trip() {
    let (mut app, enemies, player) = battle_app(1);

    press(&mut app, KeyCode::Enter); // Fight → Targeting
    press(&mut app, KeyCode::Enter); // confirm → attack → EnemyTurn
    assert_eq!(current_phase(&app), TurnPhase::EnemyTurn);
    assert_eq!(health_of(&app, enemies[0]), 200 - 50);

    // The lone enemy's immediate attack already landed on entering EnemyTurn.
    assert_eq!(health_of(&app, player), 100 - 10);

    advance(&mut app, 2);
    assert_eq!(current_phase(&app), TurnPhase::PlayerTurn);
}
