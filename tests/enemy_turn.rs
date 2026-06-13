//! Headless enemy-turn coverage, mirroring the Godot `BattleScene`
//! `ProcessEnemyAttacks` behaviour.
//!
//! Builds a minimal `App` with the enemy-turn queue + tick, the combat resolver,
//! and `check_battle_end`, then drives it under
//! `TimeUpdateStrategy::ManualDuration` so virtual time advances a fixed step per
//! `app.update()`. That makes the "first attack immediate, then 1.0 s gaps"
//! pacing deterministic and assertable without a renderer or a real clock.
//! Assertions are ECS facts: the `DamageDealt` stream (who attacked, for how
//! much), player `Health`, the `TurnPhase` state, and the `BattleResult` resource.

use std::time::Duration;

use bevy::prelude::*;
use bevy::state::app::StatesPlugin;
use bevy::time::TimeUpdateStrategy;

use bevy_2d_rpg_game::battle::enemy_turn::{
    ENEMY_ATTACK_INTERVAL, EnemyTurnQueue, on_enter_enemy_turn, tick_enemy_turn,
};
use bevy_2d_rpg_game::battle::menu::{MenuSelection, on_enter_player_turn};
use bevy_2d_rpg_game::battle::messages::LogMessage;
use bevy_2d_rpg_game::battle::rng::DamageRng;
use bevy_2d_rpg_game::battle::state::{BattleResult, BattleSet, TurnPhase};
use bevy_2d_rpg_game::characters::components::{
    CombatStats, DamageVariance, Defending, DisplayName, Enemy, Health, Player,
};
use bevy_2d_rpg_game::combat::events::{AttackRequested, DamageDealt};
use bevy_2d_rpg_game::combat::resolve::{apply_attacks, check_battle_end, on_died_hide_sprite};

/// A step that divides `ENEMY_ATTACK_INTERVAL` evenly, so four updates make
/// exactly one 1.0 s gap and the timer fires on a known frame.
const STEP: Duration = Duration::from_millis(250);

/// Build a headless enemy-turn app: the turn-state machine, the enemy queue +
/// tick, the resolver, and `check_battle_end`, all under manual virtual time.
/// Returns the app, the player entity, and the enemy entities (index order).
fn enemy_turn_app(
    player_hp: i32,
    enemy_hp: &[i32],
    player_defending: bool,
) -> (App, Entity, Vec<Entity>) {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, StatesPlugin))
        .insert_resource(TimeUpdateStrategy::ManualDuration(STEP))
        .init_state::<TurnPhase>()
        .init_resource::<EnemyTurnQueue>()
        // `on_enter_player_turn` reads the menu selection; supply it so the
        // PlayerTurn transition (the "Defend clears" case) does not panic.
        .init_resource::<MenuSelection>()
        .add_message::<LogMessage>()
        .add_message::<AttackRequested>()
        .add_message::<DamageDealt>()
        // Seed 0 fixes the variance stream; the wide-open [1.0, 1.0] bands below
        // pin variance to 1.0 regardless, so damage is exact.
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
        .add_systems(OnEnter(TurnPhase::EnemyTurn), on_enter_enemy_turn)
        .add_systems(
            Update,
            (
                tick_enemy_turn
                    .in_set(BattleSet::Input)
                    .run_if(in_state(TurnPhase::EnemyTurn)),
                apply_attacks.in_set(BattleSet::Resolve),
                check_battle_end
                    .in_set(BattleSet::Cleanup)
                    .run_if(on_message::<DamageDealt>),
            ),
        );

    let mut player = app.world_mut().spawn((
        Player,
        DisplayName("Hero".to_string()),
        CombatStats {
            attack: 50,
            defense: 0,
        },
        DamageVariance { min: 1.0, max: 1.0 },
        Health::full(player_hp),
        Visibility::Visible,
    ));
    if player_defending {
        player.insert(Defending);
    }
    let player = player.id();

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
                    // Pin variance to 1.0 so each enemy hit is exactly its
                    // (possibly halved) attack value.
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

    (app, player, enemies)
}

/// Advance one virtual `STEP`, then return the attacks that landed this frame.
fn step(app: &mut App) -> Vec<DamageDealt> {
    app.update();
    app.world_mut()
        .resource_mut::<Messages<DamageDealt>>()
        .drain()
        .collect()
}

fn enter_enemy_turn(app: &mut App) {
    app.world_mut()
        .resource_mut::<NextState<TurnPhase>>()
        .set(TurnPhase::EnemyTurn);
}

fn phase(app: &App) -> TurnPhase {
    *app.world().resource::<State<TurnPhase>>().get()
}

fn player_hp(app: &App, player: Entity) -> i32 {
    app.world().entity(player).get::<Health>().unwrap().current
}

/// Each alive enemy attacks exactly once per round; a dead enemy is skipped.
#[test]
fn each_alive_enemy_attacks_once_dead_skipped() {
    // Middle enemy starts dead (0 HP); it must never attack.
    let (mut app, _player, enemies) = enemy_turn_app(500, &[80, 0, 80], false);
    enter_enemy_turn(&mut app);

    let mut attackers: Vec<Entity> = Vec::new();
    // 12 steps is comfortably past two 1.0 s gaps plus the immediate first hit.
    for _ in 0..12 {
        for dealt in step(&mut app) {
            attackers.push(dealt.attacker);
        }
    }

    assert!(
        attackers.contains(&enemies[0]) && attackers.contains(&enemies[2]),
        "both living enemies attack"
    );
    assert!(
        !attackers.contains(&enemies[1]),
        "the dead enemy never attacks"
    );
    assert_eq!(
        attackers.iter().filter(|&&e| e == enemies[0]).count(),
        1,
        "enemy 0 attacks exactly once"
    );
    assert_eq!(
        attackers.iter().filter(|&&e| e == enemies[2]).count(),
        1,
        "enemy 2 attacks exactly once"
    );
    // Turn complete ⇒ back to the player.
    assert_eq!(phase(&app), TurnPhase::PlayerTurn);
}

/// The first enemy attacks immediately on entering the turn; the second does not
/// land until a full `ENEMY_ATTACK_INTERVAL` of virtual time has passed.
#[test]
fn second_attack_waits_one_interval() {
    let (mut app, _player, enemies) = enemy_turn_app(500, &[80, 80], false);
    enter_enemy_turn(&mut app);

    // Frame 1: the transition fires the immediate first attack.
    let first = step(&mut app);
    assert_eq!(first.len(), 1, "exactly one immediate attack");
    assert_eq!(first[0].attacker, enemies[0]);
    let first_at = app.world().resource::<Time>().elapsed();

    // Step until the second enemy acts, then check it did not happen until a full
    // `ENEMY_ATTACK_INTERVAL` of virtual time had passed since the first attack.
    let interval = Duration::from_secs_f32(ENEMY_ATTACK_INTERVAL);
    let mut second_at = None;
    for _ in 0..16 {
        let dealt = step(&mut app);
        if let Some(attack) = dealt.first() {
            assert_eq!(attack.attacker, enemies[1], "the second enemy acts next");
            second_at = Some(app.world().resource::<Time>().elapsed());
            break;
        }
    }
    let second_at = second_at.expect("the second enemy eventually attacks");
    let gap = second_at.saturating_sub(first_at);
    assert!(
        gap >= interval,
        "no second attack before {ENEMY_ATTACK_INTERVAL} s of virtual time (waited {gap:?})"
    );
}

/// Defend halves the incoming attack for the whole enemy turn, then clears on
/// the return to the player turn so the next turn lands full damage.
#[test]
fn defend_halves_for_one_turn_then_clears() {
    // One enemy (attack 10). Defending should make the first hit 5, not 10.
    let (mut app, player, _enemies) = enemy_turn_app(100, &[80], true);
    enter_enemy_turn(&mut app);

    // Run the turn out and back to the player.
    for _ in 0..3 {
        step(&mut app);
    }
    assert_eq!(
        player_hp(&app, player),
        95,
        "a defended 10-attack hit deals 5"
    );
    assert_eq!(phase(&app), TurnPhase::PlayerTurn);
    assert!(
        app.world().entity(player).get::<Defending>().is_none(),
        "Defending clears on OnEnter(PlayerTurn)"
    );

    // A second enemy turn — no longer defending — lands the full 10.
    enter_enemy_turn(&mut app);
    for _ in 0..3 {
        step(&mut app);
    }
    assert_eq!(
        player_hp(&app, player),
        85,
        "the undefended follow-up hit deals the full 10"
    );
}

/// A player death mid-queue stops the remaining attacks and ends the battle in
/// defeat ("Game Over!", `BattleResult { victory: false }`, `BattleOver`).
#[test]
fn player_death_midqueue_ends_battle() {
    // Player at 15 HP, three enemies hitting for 10 each: hit 1 → 5, hit 2 → 0
    // (dead). The third enemy must never attack.
    let (mut app, player, enemies) = enemy_turn_app(15, &[80, 80, 80], false);
    enter_enemy_turn(&mut app);

    let mut attackers: Vec<Entity> = Vec::new();
    for _ in 0..12 {
        for dealt in step(&mut app) {
            attackers.push(dealt.attacker);
        }
    }

    assert_eq!(player_hp(&app, player), 0, "the player is defeated");
    assert_eq!(phase(&app), TurnPhase::BattleOver, "the battle ends");
    assert!(
        !app.world().resource::<BattleResult>().victory,
        "a player defeat is not a victory"
    );
    assert_eq!(
        attackers,
        vec![enemies[0], enemies[1]],
        "the third enemy never gets to act"
    );
}
