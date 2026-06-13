//! Full battle-loop integration, mirroring the Godot `BattleSceneTest`
//! orchestration: one complete round drives `PlayerTurn → Targeting → EnemyTurn
//! → PlayerTurn`.
//!
//! This wires the *whole* turn machine — menu input, targeting, the enemy-turn
//! queue and tick, and combat resolution — into one headless `App` and plays it
//! with simulated key presses and `TimeUpdateStrategy::ManualDuration` virtual
//! time, exactly as the unit-level harnesses do, but end to end. Entities are
//! spawned directly (no renderer / asset loading); the assertions are the
//! `TurnPhase` transitions and the surviving `Health` on both sides.
//!
//! As in the menu/targeting tests, `InputPlugin` is omitted (it would wipe a
//! manually-pressed key) and `StatesPlugin` is added explicitly.

use std::time::Duration;

use bevy::prelude::*;
use bevy::state::app::StatesPlugin;
use bevy::time::TimeUpdateStrategy;

use bevy_2d_rpg_game::battle::enemy_turn::{EnemyTurnQueue, on_enter_enemy_turn, tick_enemy_turn};
use bevy_2d_rpg_game::battle::menu::{MenuSelection, menu_input, on_enter_player_turn};
use bevy_2d_rpg_game::battle::messages::LogMessage;
use bevy_2d_rpg_game::battle::rng::DamageRng;
use bevy_2d_rpg_game::battle::state::{BattleSet, TurnPhase};
use bevy_2d_rpg_game::battle::targeting::{
    SelectedTarget, on_enter_targeting, on_exit_targeting, targeting_input,
};
use bevy_2d_rpg_game::characters::components::{
    CombatStats, DamageVariance, DisplayName, Enemy, Health, Player,
};
use bevy_2d_rpg_game::combat::events::{AttackRequested, DamageDealt};
use bevy_2d_rpg_game::combat::resolve::{apply_attacks, check_battle_end, on_died_hide_sprite};

const STEP: Duration = Duration::from_millis(250);

/// Build the full headless battle app, starting in `PlayerTurn`, with one player
/// and `enemy_hp.len()` enemies. Returns the app, player entity, and enemies.
fn battle_app(player_hp: i32, enemy_hp: &[i32]) -> (App, Entity, Vec<Entity>) {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, StatesPlugin))
        .insert_resource(TimeUpdateStrategy::ManualDuration(STEP))
        .init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<MenuSelection>()
        .init_resource::<SelectedTarget>()
        .init_resource::<EnemyTurnQueue>()
        .init_state::<TurnPhase>()
        .add_message::<LogMessage>()
        .add_message::<AttackRequested>()
        .add_message::<DamageDealt>()
        .insert_resource(DamageRng::from_seed(0))
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
            // Enough attack to wound but not one-shot a healthy enemy.
            CombatStats {
                attack: 30,
                defense: 0,
            },
            DamageVariance { min: 1.0, max: 1.0 },
            Health::full(player_hp),
            Visibility::Visible,
        ))
        .id();

    let enemies: Vec<Entity> = enemy_hp
        .iter()
        .enumerate()
        .map(|(index, &hp)| {
            app.world_mut()
                .spawn((
                    Enemy { index },
                    DisplayName(format!("Goblin {index}")),
                    CombatStats {
                        attack: 10,
                        defense: 0,
                    },
                    DamageVariance { min: 1.0, max: 1.0 },
                    Health {
                        current: hp,
                        max: 80,
                    },
                    Visibility::Visible,
                ))
                .id()
        })
        .collect();

    // One update runs the initial OnEnter(PlayerTurn) (highlight row 0).
    app.update();
    (app, player, enemies)
}

/// Press `key`, run a frame so the input edge is seen, then release + run an
/// input-free frame so any queued `NextState` transition takes effect.
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

fn phase(app: &App) -> TurnPhase {
    *app.world().resource::<State<TurnPhase>>().get()
}

fn hp(app: &App, entity: Entity) -> i32 {
    app.world().entity(entity).get::<Health>().unwrap().current
}

/// A full round: open on the player turn, Fight → confirm an attack → enemies
/// take their turn → control returns to the player, with both sides alive.
#[test]
fn full_round_returns_to_player_turn() {
    // Two enemies (80 HP) survive a 30 hit; the player (100 HP) survives the
    // enemies' 10s — so the round loops cleanly back to the player.
    let (mut app, player, enemies) = battle_app(100, &[80, 80]);
    assert_eq!(phase(&app), TurnPhase::PlayerTurn);

    // Fight (row 0) → Targeting.
    press(&mut app, KeyCode::Enter);
    assert_eq!(phase(&app), TurnPhase::Targeting);

    // Confirm the attack on the homed-onto first enemy → resolves → EnemyTurn.
    press(&mut app, KeyCode::Enter);
    assert_eq!(hp(&app, enemies[0]), 50, "the player's 30 hit lands");
    assert_eq!(phase(&app), TurnPhase::EnemyTurn);

    // Let the enemy queue play out under virtual time; both enemies attack.
    for _ in 0..12 {
        app.update();
    }

    assert_eq!(
        phase(&app),
        TurnPhase::PlayerTurn,
        "the round loops back to the player turn"
    );
    assert_eq!(
        hp(&app, player),
        80,
        "two enemy 10-hits brought the player from 100 to 80"
    );
    // The player turn re-homed: row 0 is highlighted again.
    assert_eq!(app.world().resource::<MenuSelection>().highlighted, Some(0));
}

/// Killing every enemy on the player's attack ends the battle in victory without
/// ever reaching the enemy turn.
#[test]
fn killing_last_enemy_wins_before_enemy_turn() {
    // A single 20-HP enemy dies to the 30 hit.
    let (mut app, _player, enemies) = battle_app(100, &[20]);

    press(&mut app, KeyCode::Enter); // Fight → Targeting
    press(&mut app, KeyCode::Enter); // confirm → kill → Victory

    assert_eq!(hp(&app, enemies[0]), 0);
    assert_eq!(phase(&app), TurnPhase::BattleOver);
}
