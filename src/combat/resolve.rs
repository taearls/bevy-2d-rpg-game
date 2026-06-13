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

use crate::battle::messages::LogMessage;
use crate::battle::rng::DamageRng;
use crate::battle::state::TurnPhase;
use crate::characters::components::{CombatStats, DamageVariance, DisplayName, Enemy, Health};

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
/// Mirrors Godot `BattleCharacter.TakeDamage` plus the `DamageDealt` /
/// `CharacterDefeated` signal emissions.
pub fn apply_attacks(
    mut attacks: MessageReader<AttackRequested>,
    mut damage_dealt: MessageWriter<DamageDealt>,
    mut log: MessageWriter<LogMessage>,
    mut rng: ResMut<DamageRng>,
    mut commands: Commands,
    attackers: Query<(&CombatStats, &DamageVariance, &DisplayName)>,
    mut targets: Query<(&mut Health, &DisplayName)>,
) {
    for &AttackRequested { attacker, target } in attacks.read() {
        let Ok((stats, variance, attacker_name)) = attackers.get(attacker) else {
            continue;
        };
        // Read the attacker's name before borrowing `targets` mutably so the two
        // queries never overlap (an entity can't be both in this resolver).
        let attacker_name = attacker_name.0.clone();

        let Ok((mut health, target_name)) = targets.get_mut(target) else {
            continue;
        };
        if !health.is_alive() {
            continue;
        }

        let roll = rng.0.random_range(variance.min..=variance.max);
        let amount = compute_damage(stats.attack, stats.defense, roll);
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

/// `BattleSet::Cleanup`: end the battle when every enemy is dead, otherwise hand
/// the turn to the enemies.
///
/// Runs only the frame combat was resolved (gated on the caller to
/// [`Targeting`](TurnPhase::Targeting) exit in Phase 5). If no enemy has positive
/// health, writes "Victory!" and moves to [`BattleOver`](TurnPhase::BattleOver);
/// otherwise advances to [`EnemyTurn`](TurnPhase::EnemyTurn). Mirrors the Godot
/// `CheckBattleEnd` victory branch (the defeat branch arrives with the enemy turn
/// in Phase 6).
pub fn check_battle_end(
    enemies: Query<&Health, With<Enemy>>,
    mut next_state: ResMut<NextState<TurnPhase>>,
    mut log: MessageWriter<LogMessage>,
) {
    let all_dead = enemies.iter().all(|health| !health.is_alive());
    if all_dead {
        log.write(LogMessage::new("Victory!"));
        next_state.set(TurnPhase::BattleOver);
    } else {
        next_state.set(TurnPhase::EnemyTurn);
    }
}
