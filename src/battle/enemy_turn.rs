//! Enemy turn: the queue of enemies that act after the player, the timer that
//! paces their attacks, and the round-trip back to the player turn.
//!
//! Bevy port of the Godot `BattleScene.ProcessEnemyAttacks` chain. The Godot
//! version chained `SceneTreeTimer`s (immediate first attack, then 1.0 s waits);
//! here one [`EnemyTurnQueue`] resource — built `OnEnter(EnemyTurn)` from the
//! alive enemies in layout order — is drained by a single timer-paced
//! [`tick_enemy_turn`] system. That is deterministic under
//! `TimeUpdateStrategy::ManualDuration`, so the pacing can be asserted headlessly
//! with virtual time. The pure queue-ordering ([`enemy_turn_order`]) is factored
//! out so the "alive, in index order" rule is testable without an ECS world.

use std::collections::VecDeque;

use bevy::prelude::*;

use crate::characters::components::{Enemy, Health, Player};
use crate::combat::events::AttackRequested;

use super::state::TurnPhase;

/// Seconds between consecutive enemy attacks. The first attack of the turn fires
/// immediately (no wait), matching Godot `ProcessEnemyAttacks(0)`; this is the
/// gap before every attack after it.
pub const ENEMY_ATTACK_INTERVAL: f32 = 1.0;

/// The enemies still to act this turn, in the order they will attack, plus the
/// repeating timer that paces them.
///
/// Built fresh `OnEnter(EnemyTurn)` and drained front-to-back by
/// [`tick_enemy_turn`]. An always-present resource (defaulted at plugin build,
/// overwritten each enemy turn) so the gated tick system never races its
/// insertion.
#[derive(Resource, Debug)]
pub struct EnemyTurnQueue {
    /// Enemies yet to attack, popped from the front in layout order.
    pub pending: VecDeque<Entity>,
    /// Paces the gap between attacks (the first attack of the turn is fired
    /// immediately by `OnEnter`, so this only governs the 2nd onward).
    pub timer: Timer,
}

impl Default for EnemyTurnQueue {
    fn default() -> Self {
        Self {
            pending: VecDeque::new(),
            timer: Timer::from_seconds(ENEMY_ATTACK_INTERVAL, TimerMode::Repeating),
        }
    }
}

/// Order alive enemies for the turn queue: keep only the living, then sort by
/// layout [`index`](Enemy::index) so they act left-to-right.
///
/// Pure over `(index, entity, alive)` triples so the "dead enemies skipped, rest
/// in index order" rule can be asserted without a world. Mirrors Godot building
/// the attack list from `IsAlive` enemies in `EnemyIndex` order.
#[must_use]
pub fn enemy_turn_order(enemies: &[(usize, Entity, bool)]) -> VecDeque<Entity> {
    let mut alive: Vec<(usize, Entity)> = enemies
        .iter()
        .filter(|(_, _, alive)| *alive)
        .map(|(index, entity, _)| (*index, *entity))
        .collect();
    alive.sort_by_key(|(index, _)| *index);
    alive.into_iter().map(|(_, entity)| entity).collect()
}

/// `OnEnter(EnemyTurn)`: build the queue from the alive enemies and fire the
/// first attack immediately.
///
/// The queue is the living enemies in layout order ([`enemy_turn_order`]); the
/// first is popped and its [`AttackRequested`] written this transition (Godot's
/// immediate `ProcessEnemyAttacks(0)`), so it resolves on the same frame the turn
/// begins. The remaining enemies are paced 1.0 s apart by [`tick_enemy_turn`] off
/// a fresh timer.
pub fn on_enter_enemy_turn(
    mut commands: Commands,
    mut attacks: MessageWriter<AttackRequested>,
    player: Query<Entity, With<Player>>,
    enemies: Query<(Entity, &Enemy, &Health)>,
) {
    let order: Vec<(usize, Entity, bool)> = enemies
        .iter()
        .map(|(entity, enemy, health)| (enemy.index, entity, health.is_alive()))
        .collect();
    let mut queue = EnemyTurnQueue {
        pending: enemy_turn_order(&order),
        timer: Timer::from_seconds(ENEMY_ATTACK_INTERVAL, TimerMode::Repeating),
    };

    // Parity with Godot `ProcessEnemyAttacks(0)`: the first enemy attacks with no
    // delay. Firing it here (rather than via the timer) keeps the gap before the
    // *second* attack an exact 1.0 s, since the timer then starts from zero.
    if let Some(player) = player.iter().next()
        && let Some(enemy) = queue.pending.pop_front()
    {
        attacks.write(AttackRequested {
            attacker: enemy,
            target: player,
        });
    }

    commands.insert_resource(queue);
}

/// `BattleSet::Input`, gated to [`EnemyTurn`](TurnPhase::EnemyTurn): pace the
/// remaining enemy attacks, then hand the turn back to the player.
///
/// Each frame ticks the timer by the (virtual) frame delta; when it fires, the
/// next enemy is popped and its [`AttackRequested`] written for `Resolve`. Once
/// the queue is empty the turn is over and we return to
/// [`PlayerTurn`](TurnPhase::PlayerTurn). A player defeat mid-queue is handled by
/// `check_battle_end` flipping the state to `BattleOver`, which gates this system
/// off — so the remaining attacks simply never fire.
pub fn tick_enemy_turn(
    time: Res<Time>,
    mut queue: ResMut<EnemyTurnQueue>,
    mut attacks: MessageWriter<AttackRequested>,
    mut next_state: ResMut<NextState<TurnPhase>>,
    player: Query<Entity, With<Player>>,
) {
    if queue.pending.is_empty() {
        next_state.set(TurnPhase::PlayerTurn);
        return;
    }

    if !queue.timer.tick(time.delta()).just_finished() {
        return;
    }

    let Some(player) = player.iter().next() else {
        return;
    };
    if let Some(enemy) = queue.pending.pop_front() {
        attacks.write(AttackRequested {
            attacker: enemy,
            target: player,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entity(raw: u32) -> Entity {
        Entity::from_raw_u32(raw).unwrap()
    }

    /// Alive enemies are ordered by layout index regardless of input order.
    #[test]
    fn order_sorts_alive_by_index() {
        let a = entity(1);
        let b = entity(2);
        let c = entity(3);
        // Supplied out of index order.
        let order = enemy_turn_order(&[(2, c, true), (0, a, true), (1, b, true)]);
        assert_eq!(order, VecDeque::from([a, b, c]));
    }

    /// Dead enemies are dropped from the queue, the survivors keeping their order.
    #[test]
    fn order_skips_dead_enemies() {
        let a = entity(1);
        let dead = entity(2);
        let c = entity(3);
        let order = enemy_turn_order(&[(0, a, true), (1, dead, false), (2, c, true)]);
        assert_eq!(order, VecDeque::from([a, c]));
    }

    /// All-dead enemies yield an empty queue (the turn ends at once).
    #[test]
    fn order_all_dead_is_empty() {
        let a = entity(1);
        let b = entity(2);
        assert!(enemy_turn_order(&[(0, a, false), (1, b, false)]).is_empty());
    }
}
