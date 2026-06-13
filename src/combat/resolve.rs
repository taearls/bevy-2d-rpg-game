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

        let roll = rng.0.random_range(variance.min..=variance.max);
        // A defending target halves the attacker's effective attack *value*
        // before the formula (Godot's `_lastPlayerAction == Defend` check),
        // lasting the whole enemy turn until `Defending` clears on the next
        // `OnEnter(PlayerTurn)`.
        let attack_value = if defending {
            stats.attack / 2
        } else {
            stats.attack
        };
        let amount = compute_damage(attack_value, stats.defense, roll);
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

/// `BattleSet::Cleanup`: end the battle on a decisive outcome, otherwise let the
/// turn flow continue.
///
/// Runs the frame an attack landed (gated on [`DamageDealt`] by the caller). A
/// player defeat takes priority — "Game Over!", [`BattleResult`] `victory: false`,
/// and [`BattleOver`](TurnPhase::BattleOver) — and, because the enemy-turn tick
/// is gated `in_state(EnemyTurn)`, flipping to `BattleOver` here is exactly what
/// stops the rest of the enemy queue. Otherwise, every enemy dead is "Victory!",
/// `victory: true`, and `BattleOver`. With neither side decided the battle simply
/// continues: the placeholder transition the acting phase already queued
/// (`Targeting`→`EnemyTurn`, or the enemy turn staying put) stands untouched.
/// Mirrors Godot `CheckBattleEnd` (victory and defeat branches).
pub fn check_battle_end(
    enemies: Query<&Health, With<Enemy>>,
    player: Query<&Health, With<Player>>,
    mut next_state: ResMut<NextState<TurnPhase>>,
    mut log: MessageWriter<LogMessage>,
    mut commands: Commands,
) {
    let player_dead = player
        .iter()
        .next()
        .is_some_and(|health| !health.is_alive());
    if player_dead {
        log.write(LogMessage::new("Game Over!"));
        commands.insert_resource(BattleResult { victory: false });
        next_state.set(TurnPhase::BattleOver);
        return;
    }

    if enemies.iter().all(|health| !health.is_alive()) {
        log.write(LogMessage::new("Victory!"));
        commands.insert_resource(BattleResult { victory: true });
        next_state.set(TurnPhase::BattleOver);
    }
}
