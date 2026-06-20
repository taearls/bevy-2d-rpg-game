//! The enemy turn: a timed queue that lets each living enemy attack the player
//! once, with a beat between blows.
//!
//! Bevy port of the Godot `BattleScene.ProcessEnemyAttacks` chain. The Godot
//! version queued one-shot `SceneTreeTimer`s per enemy; here a single
//! [`EnemyTurnQueue`] resource holds the pending attackers and one [`Timer`],
//! ticked by [`tick_enemy_turn`]. That collapses the recursive timer chain into
//! one deterministic system — under `TimeUpdateStrategy::ManualDuration` a test
//! can advance virtual time by an exact amount and assert precisely which
//! attacks have fired.
//!
//! The first attack lands immediately on entering the turn (parity with Godot
//! `ProcessEnemyAttacks(0)`), then each subsequent attack waits
//! [`ATTACK_INTERVAL`]. When the queue empties the turn hands back to
//! [`PlayerTurn`](TurnPhase::PlayerTurn); a player death mid-queue is caught by
//! [`check_battle_end`](crate::combat::resolve::check_battle_end), which clears
//! the queue and ends the battle so no further attacks resolve.

use std::collections::VecDeque;
use std::time::Duration;

use bevy::prelude::*;

use crate::components::{Enemy, Health, Player};

use super::state::TurnPhase;
use crate::combat::events::AttackRequested;

/// Wall-clock gap between consecutive enemy attacks. The first attack of the
/// turn is immediate; every one after it waits this long. Matches the Godot
/// `EnemyAttackDelay` of one second.
pub const ATTACK_INTERVAL: Duration = Duration::from_secs(1);

/// The enemies still waiting to attack this turn, plus the inter-attack timer.
///
/// Built fresh `OnEnter(EnemyTurn)` from the alive enemies in [`Enemy::index`]
/// order, then drained one entity at a time by [`tick_enemy_turn`]. Held as a
/// single resource — rather than per-enemy timer entities — so the whole turn is
/// one tickable unit and trivially clearable when the player dies mid-queue.
#[derive(Resource, Debug, Default)]
pub struct EnemyTurnQueue {
    /// Enemies that have not yet attacked this turn, front = next to act.
    pub pending: VecDeque<Entity>,
    /// Counts down [`ATTACK_INTERVAL`] between attacks. Starts finished so the
    /// first attack fires on the turn's first tick with no delay.
    pub timer: Timer,
}

impl EnemyTurnQueue {
    /// Whether every queued attacker has acted.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }
}

/// `OnEnter(EnemyTurn)`: build the attack queue from the living enemies in
/// layout order and arm the timer to fire its first attack immediately.
///
/// Sorting by [`Enemy::index`] makes the attack order deterministic and matches
/// the on-screen left-to-right row, independent of ECS iteration order. The
/// timer is created already finished (`tick(ATTACK_INTERVAL)` past a one-shot
/// timer) so [`tick_enemy_turn`] pops the first attacker on the very next frame
/// without waiting — Godot's immediate `ProcessEnemyAttacks(0)`.
pub fn on_enter_enemy_turn(
    mut queue: ResMut<EnemyTurnQueue>,
    enemies: Query<(Entity, &Enemy, &Health)>,
) {
    let mut alive: Vec<(usize, Entity)> = enemies
        .iter()
        .filter(|(_, _, health)| health.is_alive())
        .map(|(entity, enemy, _)| (enemy.index, entity))
        .collect();
    alive.sort_by_key(|(index, _)| *index);

    queue.pending = alive.into_iter().map(|(_, entity)| entity).collect();

    // A one-shot timer started already elapsed: the first `tick` in
    // `tick_enemy_turn` finds it finished and releases the immediate attack.
    let mut timer = Timer::new(ATTACK_INTERVAL, TimerMode::Once);
    timer.tick(ATTACK_INTERVAL);
    queue.timer = timer;
}

/// `BattleSet::Input`, gated to [`EnemyTurn`](TurnPhase::EnemyTurn): release one
/// enemy attack each time the interval elapses, then return to the player.
///
/// Ticks the timer by the frame's delta; while it is finished and the queue is
/// non-empty, pops the front enemy and writes an [`AttackRequested`] against the
/// player, resetting the timer so the next attack waits a full
/// [`ATTACK_INTERVAL`]. The first call of the turn finds the timer pre-finished
/// (see [`on_enter_enemy_turn`]) so attack one is immediate. When the queue is
/// empty the turn is over and the state returns to
/// [`PlayerTurn`](TurnPhase::PlayerTurn) — unless the battle already ended, in
/// which case `check_battle_end` has moved us to `BattleOver` and this system no
/// longer runs.
///
/// The attack is only *requested* here; `apply_attacks` resolves it in
/// `Resolve` and [`check_battle_end`](crate::combat::resolve::check_battle_end)
/// decides the battle's fate in `Cleanup`. A lethal blow to the player makes
/// that system clear `pending` and move to `BattleOver`, so this tick stops
/// firing.
pub fn tick_enemy_turn(
    time: Res<Time>,
    mut queue: ResMut<EnemyTurnQueue>,
    mut attacks: MessageWriter<AttackRequested>,
    mut next_state: ResMut<NextState<TurnPhase>>,
    player: Query<Entity, With<Player>>,
) {
    if queue.is_empty() {
        next_state.set(TurnPhase::PlayerTurn);
        return;
    }

    queue.timer.tick(time.delta());
    if !queue.timer.is_finished() {
        return;
    }

    let Some(player) = player.iter().next() else {
        // No player to attack — nothing to resolve; bail to the player turn so
        // the battle does not stall in EnemyTurn.
        queue.pending.clear();
        next_state.set(TurnPhase::PlayerTurn);
        return;
    };

    // At most one attack per finished interval: pop the front, fire it, and
    // re-arm the timer for the next beat.
    if let Some(enemy) = queue.pending.pop_front() {
        attacks.write(AttackRequested {
            attacker: enemy,
            target: player,
        });
        queue.timer.reset();
    }

    // Last attacker just acted: the turn ends next frame once the queue reads
    // empty (the early return at the top), after this attack has resolved.
}
