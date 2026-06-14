//! Combat resolution: turn queued [`AttackRequested`]s into health changes,
//! death reactions, and the battle-end check.
//!
//! Bevy port of the resolution half of the Godot `BattleScene` /
//! `BattleCharacter.TakeDamage` flow. The per-hit math is delegated to the pure
//! [`compute_damage`](super::damage::compute_damage); everything here is the ECS
//! plumbing around it — sampling variance, mutating [`Health`], triggering
//! [`Died`], and emitting [`DamageDealt`] + [`LogMessage`].

use bevy::prelude::*;
use rand::Rng;

use crate::battle::enemy_turn::EnemyTurnQueue;
use crate::battle::messages::LogMessage;
use crate::battle::rng::DamageRng;
use crate::battle::state::{BattleResult, TurnPhase};
use crate::characters::components::{
    CombatStats, DamageVariance, Defending, DisplayName, Enemy, Health, Player,
};

use super::damage::compute_damage;
use super::events::{AttackRequested, DamageDealt, Died};

/// `BattleSet::Resolve`: drain every queued [`AttackRequested`], apply it, and
/// emit the resulting [`DamageDealt`] / [`LogMessage`] / [`Died`].
///
/// For each attack we read the attacker's [`CombatStats`] and [`DamageVariance`],
/// sample a variance roll from [`DamageRng`] (so a pinned seed reproduces the
/// fight), run [`compute_damage`], clamp the target's `current` health into
/// `0..=max`, and write the log line. A lethal hit triggers a [`Died`] event on
/// the target. Attacks naming an entity that has since despawned, or a target
/// already at zero health, are skipped silently — the resolver never assumes the
/// world still matches when the attack was queued.
///
/// A target carrying [`Defending`] halves the attacker's *attack value* before
/// the formula (not the final damage), matching the Godot
/// `_lastPlayerAction == Defend` branch that scaled the incoming attack stat.
/// The marker is cleared `OnEnter(PlayerTurn)`, so the mitigation lasts exactly
/// one enemy turn.
///
/// Mirrors Godot `BattleCharacter.TakeDamage` plus the `DamageDealt` /
/// `CharacterDefeated` signal emissions.
pub fn apply_attacks(
    mut attacks: MessageReader<AttackRequested>,
    mut damage_dealt: MessageWriter<DamageDealt>,
    mut log: MessageWriter<LogMessage>,
    mut rng: ResMut<DamageRng>,
    mut commands: Commands,
    attackers: Query<(&CombatStats, &DamageVariance, &DisplayName)>,
    mut targets: Query<(&mut Health, &DisplayName, Has<Defending>)>,
) {
    for &AttackRequested { attacker, target } in attacks.read() {
        let Ok((stats, variance, attacker_name)) = attackers.get(attacker) else {
            continue;
        };
        // Read the attacker's name before borrowing `targets` mutably so the two
        // queries never overlap (an entity can't be both in this resolver).
        let attacker_name = attacker_name.0.clone();

        let Ok((mut health, target_name, defending)) = targets.get_mut(target) else {
            continue;
        };
        if !health.is_alive() {
            continue;
        }

        // Defend halves the attacker's attack value *before* the formula, so the
        // defense subtraction and variance roll apply to the reduced figure.
        // Integer division floors, matching the Godot halving.
        let attack = if defending {
            stats.attack / 2
        } else {
            stats.attack
        };
        let roll = rng.0.random_range(variance.min..=variance.max);
        let amount = compute_damage(attack, stats.defense, roll);
        health.current = (health.current - amount).clamp(0, health.max);

        let target_name = target_name.0.clone();
        log.write(LogMessage::new(format!(
            "{attacker_name} attacks {target_name} for {amount} damage!"
        )));
        damage_dealt.write(DamageDealt {
            attacker,
            target,
            amount,
        });

        if !health.is_alive() {
            log.write(LogMessage::new(format!("{target_name} has been defeated!")));
            commands.trigger(Died { entity: target });
        }
    }
}

/// `Died` observer: hide the defeated entity's sprite.
///
/// Kept minimal — the entity stays in the world (so its `Enemy { index }` slot
/// and name remain queryable for "all enemies dead?" and the battle log) but
/// drops out of view. Mirrors the Godot `QueueFree`-deferred hide-on-defeat
/// without actually despawning, which would invalidate the layout indices the
/// targeting cycle relies on.
pub fn on_died_hide_sprite(died: On<Died>, mut visibility: Query<&mut Visibility>) {
    if let Ok(mut vis) = visibility.get_mut(died.event().entity) {
        *vis = Visibility::Hidden;
    }
}

/// `BattleSet::Cleanup`: decide the battle's fate the frame an attack landed.
///
/// Runs only on a frame that produced a [`DamageDealt`] (gated by the caller),
/// so it sees the world right after `apply_attacks`. The checks, in order:
///
/// 1. **Defeat** — the player is dead: write "Game Over!", record a losing
///    [`BattleResult`], clear the [`EnemyTurnQueue`] so no further enemy attacks
///    resolve, and move to [`BattleOver`](TurnPhase::BattleOver). Checked first
///    so a blow that fells the player ends the battle even if it also happened
///    to clear the last enemy.
/// 2. **Victory** — every enemy is dead: write "Victory!", record a winning
///    [`BattleResult`], and move to `BattleOver`.
/// 3. **Player attack resolved, enemies remain** — only when we are leaving
///    [`Targeting`](TurnPhase::Targeting): hand the turn to the enemies via
///    [`EnemyTurn`](TurnPhase::EnemyTurn).
///
/// During the enemy turn (case 3's guard is false) a non-terminal attack leaves
/// the state untouched: [`tick_enemy_turn`](crate::battle::enemy_turn::tick_enemy_turn)
/// owns the `EnemyTurn → PlayerTurn` hand-back once its queue empties. Mirrors
/// the Godot `CheckBattleEnd` victory/defeat branches.
pub fn check_battle_end(
    state: Res<State<TurnPhase>>,
    enemies: Query<&Health, With<Enemy>>,
    player: Query<&Health, With<Player>>,
    mut queue: ResMut<EnemyTurnQueue>,
    mut result: ResMut<BattleResult>,
    mut next_state: ResMut<NextState<TurnPhase>>,
    mut log: MessageWriter<LogMessage>,
) {
    let player_dead = player.iter().any(|health| !health.is_alive());
    if player_dead {
        log.write(LogMessage::new("Game Over!"));
        // Stop any enemies still queued behind the lethal blow.
        queue.pending.clear();
        *result = BattleResult { victory: false };
        next_state.set(TurnPhase::BattleOver);
        return;
    }

    let all_enemies_dead = enemies.iter().all(|health| !health.is_alive());
    if all_enemies_dead {
        log.write(LogMessage::new("Victory!"));
        *result = BattleResult { victory: true };
        next_state.set(TurnPhase::BattleOver);
        return;
    }

    // A resolved *player* attack (we are still in Targeting) hands off to the
    // enemy turn. A resolved enemy attack leaves the state alone — the enemy
    // queue tick decides when the turn ends.
    if *state.get() == TurnPhase::Targeting {
        next_state.set(TurnPhase::EnemyTurn);
    }
}
