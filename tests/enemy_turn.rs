//! Headless enemy-turn coverage of the enemy-attack cases.
//!
//! Builds an `App` with the turn-state machine, the `OnEnter(EnemyTurn)` queue
//! build, the `tick_enemy_turn` releaser, and the combat resolver +
//! `check_battle_end`, then advances **virtual time** in fixed steps via
//! [`TimeUpdateStrategy::ManualDuration`] so every assertion lands on an exact
//! amount of elapsed time — no real clock, no flakiness. Assertions are ECS
//! facts: the player's `Health`, the `DamageDealt` stream, the `EnemyTurnQueue`
//! contents, and the resulting `TurnPhase`.
//!
//! As in the menu/targeting tests, `InputPlugin` is omitted (no keys are pressed
//! here) and `StatesPlugin` is added explicitly. The `Player` carries
//! `Defending` in the relevant cases to exercise the halving.

use std::time::Duration;

use bevy::prelude::*;
use bevy::state::app::StatesPlugin;
use bevy::time::TimeUpdateStrategy;

use aliasing::battle::enemy_turn::{
    ATTACK_INTERVAL, EnemyTurnQueue, on_enter_enemy_turn, tick_enemy_turn,
};
use aliasing::battle::menu::{MenuSelection, on_enter_player_turn};
use aliasing::battle::messages::LogMessage;
use aliasing::battle::rng::DamageRng;
use aliasing::battle::state::{BattleResult, BattleSet, TurnPhase};
use aliasing::combat::events::{AttackRequested, DamageDealt};
use aliasing::combat::resolve::{apply_attacks, check_battle_end, on_died_hide_sprite};
use aliasing::components::{
    CombatStats, DamageVariance, Defending, DisplayName, Enemy, Health, Player,
};

/// A frame's worth of virtual time small enough that one tick never crosses a
/// full [`ATTACK_INTERVAL`]. Used to prove the "no second attack before 1.0 s"
/// guarantee: many of these steps must pass before the next attack fires.
const STEP: Duration = Duration::from_millis(100);

/// Build a headless battle app in the `EnemyTurn` phase. Spawns `enemy_attacks`
/// enemies (index `0..n`, each with the given attack stat and full health) plus
/// one player at `player_hp` HP. Returns the app, the enemy entities, and the
/// player entity. The `OnEnter(EnemyTurn)` build has already run after the first
/// `update`, so the queue is primed and the first attack is pending.
fn enemy_turn_app(enemy_attacks: &[i32], player_hp: i32) -> (App, Vec<Entity>, Entity) {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, StatesPlugin))
        .init_resource::<MenuSelection>()
        .init_resource::<EnemyTurnQueue>()
        .init_resource::<BattleResult>()
        .init_state::<TurnPhase>()
        .add_message::<LogMessage>()
        .add_message::<AttackRequested>()
        .add_message::<DamageDealt>()
        // Pinned variance band on every character means the roll is irrelevant,
        // but a fixed-seed RNG keeps the sample deterministic regardless.
        .insert_resource(DamageRng::from_seed(0))
        // Drive virtual time by hand: each `update` advances exactly `STEP`.
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
            Health::full(player_hp),
        ))
        .id();

    let enemies: Vec<Entity> = enemy_attacks
        .iter()
        .enumerate()
        .map(|(index, &attack)| {
            app.world_mut()
                .spawn((
                    Enemy { index },
                    DisplayName(format!("Goblin {index}")),
                    CombatStats { attack, defense: 0 },
                    // Pinned variance so each attack deals exactly its attack
                    // value (defense 0) — `compute_damage(attack, 0, 1.0)`.
                    DamageVariance { min: 1.0, max: 1.0 },
                    Health::full(100),
                    Visibility::Visible,
                ))
                .id()
        })
        .collect();

    // Enter the enemy turn so `OnEnter(EnemyTurn)` builds the queue.
    app.world_mut()
        .resource_mut::<NextState<TurnPhase>>()
        .set(TurnPhase::EnemyTurn);
    // First update applies the OnEnter (queue built); time advances by STEP.
    app.update();

    (app, enemies, player)
}

fn player_hp(app: &mut App, player: Entity) -> i32 {
    app.world().entity(player).get::<Health>().unwrap().current
}

fn current_phase(app: &App) -> TurnPhase {
    *app.world().resource::<State<TurnPhase>>().get()
}

fn queue_len(app: &App) -> usize {
    app.world().resource::<EnemyTurnQueue>().pending.len()
}

/// Drain every pending `DamageDealt` so repeated reads across frames never
/// double-count a single attack.
fn drain_damage(app: &mut App) -> Vec<DamageDealt> {
    app.world_mut()
        .resource_mut::<Messages<DamageDealt>>()
        .drain()
        .collect()
}

/// Advance virtual time by `count` × `STEP`, one `update` per step.
fn advance(app: &mut App, count: u32) {
    for _ in 0..count {
        app.update();
    }
}

/// Number of `STEP`s that just exceed one `ATTACK_INTERVAL` (so the gap timer
/// has surely elapsed). With STEP = 100 ms and the interval = 1 s, that's 10.
fn steps_per_interval() -> u32 {
    // Round up so the accumulated time is ≥ the interval.
    (ATTACK_INTERVAL.as_millis() as u32).div_ceil(STEP.as_millis() as u32)
}

/// Every alive enemy attacks exactly once per round, in index order, with the
/// first blow landing immediately and each later one a full interval apart.
#[test]
fn each_enemy_attacks_once_per_round() {
    let (mut app, enemies, player) = enemy_turn_app(&[10, 10], 100);

    // The first attack is immediate: it fired on the `update` inside the builder
    // (timer pre-finished), so the player has already taken one 10-damage hit
    // and exactly one enemy remains queued.
    let first = drain_damage(&mut app);
    assert_eq!(
        first.len(),
        1,
        "exactly one attack on the immediate first beat"
    );
    assert_eq!(first[0].attacker, enemies[0], "enemy index 0 attacks first");
    assert_eq!(player_hp(&mut app, player), 90);
    assert_eq!(queue_len(&app), 1, "second enemy still pending");

    // Advance a full interval: the second (last) enemy attacks.
    advance(&mut app, steps_per_interval());
    let second = drain_damage(&mut app);
    assert_eq!(second.len(), 1, "the second enemy's single attack");
    assert_eq!(second[0].attacker, enemies[1]);
    assert_eq!(player_hp(&mut app, player), 80, "two 10-damage hits total");

    // Queue now empty → the turn hands back to the player. One tick sets the
    // `NextState`; a second `update` lets the transition apply.
    advance(&mut app, 2);
    assert_eq!(current_phase(&app), TurnPhase::PlayerTurn);
}

/// A dead enemy is never queued, so it takes no turn: only the living enemy
/// attacks when the turn (re)builds its queue.
#[test]
fn dead_enemies_are_skipped() {
    let (mut app, enemies, _player) = enemy_turn_app(&[10, 10], 100);
    // Discard the immediate attack from the initial build (both enemies alive).
    let _ = drain_damage(&mut app);

    // Kill the still-pending enemy 1, then re-enter the turn so `OnEnter` rebuilds
    // the queue from the *current* alive set — enemy 1 must be excluded.
    app.world_mut()
        .entity_mut(enemies[1])
        .get_mut::<Health>()
        .unwrap()
        .current = 0;
    app.world_mut()
        .resource_mut::<NextState<TurnPhase>>()
        .set(TurnPhase::EnemyTurn);
    app.update();

    // Exactly one immediate attack — from the living enemy 0 — and then the
    // queue is empty (the dead enemy was never enqueued).
    let dealt = drain_damage(&mut app);
    assert_eq!(dealt.len(), 1, "only the living enemy attacks");
    assert_eq!(dealt[0].attacker, enemies[0], "the dead enemy is skipped");
    assert_eq!(queue_len(&app), 0, "no further attackers queued");
}

/// No second attack lands until a full `ATTACK_INTERVAL` of virtual time has
/// passed after the first.
#[test]
fn no_second_attack_before_one_interval() {
    let (mut app, _enemies, _player) = enemy_turn_app(&[10, 10], 100);
    // Consume the immediate first attack.
    let _ = drain_damage(&mut app);
    assert_eq!(queue_len(&app), 1);

    // Step almost a full interval (one STEP short): still no second attack.
    advance(&mut app, steps_per_interval() - 1);
    assert!(
        drain_damage(&mut app).is_empty(),
        "no second attack before the interval elapses"
    );
    assert_eq!(queue_len(&app), 1, "second enemy still waiting");

    // One more step crosses the interval: now it fires.
    advance(&mut app, 1);
    assert_eq!(
        drain_damage(&mut app).len(),
        1,
        "second attack after the interval"
    );
}

/// `Defending` halves the attacker's attack value for exactly one enemy turn,
/// then clears `OnEnter(PlayerTurn)` so the next round takes full damage.
#[test]
fn defend_halves_attack_for_one_turn_then_clears() {
    // Single enemy with attack 20, defense 0 → full damage 20, halved 10.
    let (mut app, _enemies, player) = enemy_turn_app(&[20], 100);
    // Mark the player defending and re-enter the turn so the halving applies to
    // the (already-immediate) attack. Drain the un-defended build attack first.
    let _ = drain_damage(&mut app);
    app.world_mut().entity_mut(player).insert(Defending);
    app.world_mut()
        .resource_mut::<NextState<TurnPhase>>()
        .set(TurnPhase::EnemyTurn);
    app.update();

    let defended = drain_damage(&mut app);
    assert_eq!(defended.len(), 1);
    assert_eq!(
        defended[0].amount, 10,
        "attack 20 halved to 10 before the formula while Defending"
    );

    // Queue empty → back to PlayerTurn, which clears `Defending`. One tick sets
    // the `NextState`; a second `update` applies the transition and runs
    // `OnEnter(PlayerTurn)`.
    advance(&mut app, 2);
    assert_eq!(current_phase(&app), TurnPhase::PlayerTurn);
    assert!(
        app.world().entity(player).get::<Defending>().is_none(),
        "Defending cleared OnEnter(PlayerTurn)"
    );

    // A second enemy turn now deals full damage again.
    app.world_mut()
        .resource_mut::<NextState<TurnPhase>>()
        .set(TurnPhase::EnemyTurn);
    app.update();
    let full = drain_damage(&mut app);
    assert_eq!(full.len(), 1);
    assert_eq!(full[0].amount, 20, "full damage once Defending has cleared");
}

/// A blow that drops the player to 0 HP ends the battle (`BattleOver`) and stops
/// every remaining queued enemy from attacking.
#[test]
fn player_death_mid_queue_stops_remaining_attacks() {
    // Three enemies; the first hits for 100 and kills the full-HP player.
    let (mut app, _enemies, player) = enemy_turn_app(&[100, 10, 10], 100);

    // The immediate first attack already landed in the builder, killing the
    // player. `check_battle_end` ran in that frame's Cleanup: it drove HP to 0
    // and cleared the queue right away, and queued the `BattleOver` transition.
    let first = drain_damage(&mut app);
    assert_eq!(first.len(), 1, "only the lethal first attack resolved");
    assert_eq!(player_hp(&mut app, player), 0, "player driven to zero HP");
    assert!(
        app.world().resource::<EnemyTurnQueue>().pending.is_empty(),
        "remaining enemies are cleared from the queue on player death"
    );

    // One settling `update` applies the queued `BattleOver` transition. The
    // enemy-turn tick is gated out of `BattleOver`, so no further attack fires.
    advance(&mut app, 1);
    assert_eq!(
        current_phase(&app),
        TurnPhase::BattleOver,
        "player death ends the battle"
    );
    assert!(
        !app.world().resource::<BattleResult>().victory,
        "the recorded outcome is a defeat"
    );

    // Advancing time fires no further attacks: the tick system is gated out of
    // BattleOver and the queue is empty regardless.
    advance(&mut app, steps_per_interval() * 3);
    assert!(
        drain_damage(&mut app).is_empty(),
        "no enemy attacks after the battle is over"
    );
    assert_eq!(current_phase(&app), TurnPhase::BattleOver);
}

/// Defensive guard: with no `Player` entity present there is nothing to attack,
/// so the tick clears the queue and bails back to `PlayerTurn` rather than
/// stalling forever in `EnemyTurn`. Exercises the otherwise-unreachable branch.
#[test]
fn missing_player_ends_the_turn_without_stalling() {
    let (mut app, _enemies, player) = enemy_turn_app(&[10, 10], 100);
    // Discard the immediate first attack from the build (player still present).
    let _ = drain_damage(&mut app);

    // Remove the player, then re-enter the turn so the tick runs with no target.
    app.world_mut().entity_mut(player).despawn();
    app.world_mut()
        .resource_mut::<NextState<TurnPhase>>()
        .set(TurnPhase::EnemyTurn);
    app.update();

    // The tick found no player: it cleared the queue and requested PlayerTurn,
    // with no attack written and no panic. A settling frame applies the
    // transition.
    assert!(
        drain_damage(&mut app).is_empty(),
        "no attack is written when there is no player to target"
    );
    assert!(
        app.world().resource::<EnemyTurnQueue>().pending.is_empty(),
        "the queue is cleared when the turn cannot proceed"
    );
    advance(&mut app, 1);
    assert_eq!(
        current_phase(&app),
        TurnPhase::PlayerTurn,
        "the turn bails back to the player rather than stalling"
    );
}
