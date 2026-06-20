//! Headless combat-resolution coverage, mirroring the Godot `BattleEventsTest`.
//!
//! Builds a minimal `App` with just the combat plumbing — the `AttackRequested`
//! / `DamageDealt` messages, the `Died` observer, a fixed-seed `DamageRng`, and
//! the `apply_attacks` system — then queues attacks and asserts the resulting
//! ECS facts: the `DamageDealt` stream, mutated `Health`, the `Died`-driven
//! `Visibility::Hidden`, and the "nothing happens" cases. No renderer or asset
//! loading: attacker/target entities are spawned with their stats injected
//! directly, exactly as the `GdUnit4` originals fabricated `BattleCharacter`s.

use bevy::prelude::*;
use bevy::state::app::StatesPlugin;

use bevy_2d_rpg_game::battle::messages::LogMessage;
use bevy_2d_rpg_game::battle::rng::DamageRng;
use bevy_2d_rpg_game::battle::state::{BattleSet, TurnPhase};
use bevy_2d_rpg_game::combat::events::{AttackRequested, DamageDealt};
use bevy_2d_rpg_game::combat::resolve::{apply_attacks, on_died_hide_sprite};
use bevy_2d_rpg_game::components::{CombatStats, DamageVariance, DisplayName, Enemy, Health};

/// A headless world with the combat resolver, a fixed-seed `DamageRng` (so the
/// variance roll — and thus the damage — is deterministic), and the `Died`
/// observer wired up. No `TurnPhase` gating: tests drive `apply_attacks` every
/// frame and queue attacks explicitly.
fn combat_app() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, StatesPlugin))
        .init_state::<TurnPhase>()
        .add_message::<LogMessage>()
        .add_message::<AttackRequested>()
        .add_message::<DamageDealt>()
        // Seed 0 fixes the variance stream so the rolled damage is reproducible.
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
        .add_systems(Update, apply_attacks.in_set(BattleSet::Resolve));
    app
}

/// Spawn an attacker with the given stats and a wide-open variance band so the
/// damage is predictable regardless of the roll.
fn spawn_attacker(app: &mut App, name: &str, attack: i32, defense: i32) -> Entity {
    app.world_mut()
        .spawn((
            DisplayName(name.to_string()),
            CombatStats { attack, defense },
            // A degenerate [1.0, 1.0] band pins variance to 1.0 so
            // `compute_damage` is exactly `max(1, attack - defense)`.
            DamageVariance { min: 1.0, max: 1.0 },
            Health::full(100),
        ))
        .id()
}

/// Spawn an enemy target with `max` health and a visible sprite-less marker we
/// can assert visibility on.
fn spawn_target(app: &mut App, name: &str, max: i32) -> Entity {
    app.world_mut()
        .spawn((
            Enemy { index: 0 },
            DisplayName(name.to_string()),
            CombatStats {
                attack: 0,
                defense: 0,
            },
            DamageVariance::default(),
            Health::full(max),
            Visibility::Visible,
        ))
        .id()
}

fn queue_attack(app: &mut App, attacker: Entity, target: Entity) {
    app.world_mut()
        .resource_mut::<Messages<AttackRequested>>()
        .write(AttackRequested { attacker, target });
}

/// Remove and return every pending `DamageDealt`. Uses `drain` (not a cursor
/// read) so a message is never observed twice across successive frames —
/// `Messages` otherwise retains each event for two update cycles.
fn drain_damage(app: &mut App) -> Vec<DamageDealt> {
    app.world_mut()
        .resource_mut::<Messages<DamageDealt>>()
        .drain()
        .collect()
}

fn health_of(app: &mut App, entity: Entity) -> Health {
    *app.world().entity(entity).get::<Health>().unwrap()
}

fn visibility_of(app: &mut App, entity: Entity) -> Visibility {
    *app.world().entity(entity).get::<Visibility>().unwrap()
}

/// An attack emits a `DamageDealt` and reduces the target's health by the
/// computed amount (`max(1, attack - defense)` at variance 1.0).
#[test]
fn attack_emits_damage_and_mutates_health() {
    let mut app = combat_app();
    let attacker = spawn_attacker(&mut app, "Hero", 12, 0);
    let target = spawn_target(&mut app, "Goblin", 80);
    queue_attack(&mut app, attacker, target);

    app.update();

    let dealt = drain_damage(&mut app);
    assert_eq!(dealt.len(), 1, "exactly one DamageDealt per attack");
    assert_eq!(dealt[0].attacker, attacker);
    assert_eq!(dealt[0].target, target);
    assert_eq!(dealt[0].amount, 12, "12 attack - 0 defense at variance 1.0");
    assert_eq!(health_of(&mut app, target).current, 80 - 12);
}

/// Defense subtracts from the hit but a connecting attack always lands at least
/// one point (parity with the `max(1, ...)` floor).
#[test]
fn defense_reduces_but_floors_at_one() {
    let mut app = combat_app();
    // Attack barely exceeds defense → base 1; even attack ≤ defense floors to 1.
    let attacker = spawn_attacker(&mut app, "Hero", 5, 100);
    let target = spawn_target(&mut app, "Goblin", 80);
    queue_attack(&mut app, attacker, target);

    app.update();

    assert_eq!(drain_damage(&mut app)[0].amount, 1);
    assert_eq!(health_of(&mut app, target).current, 79);
}

/// A lethal hit drives health to zero (never negative) and triggers `Died`,
/// whose observer hides the sprite.
#[test]
fn lethal_attack_triggers_died_and_hides_sprite() {
    let mut app = combat_app();
    let attacker = spawn_attacker(&mut app, "Hero", 50, 0);
    let target = spawn_target(&mut app, "Goblin", 30);
    queue_attack(&mut app, attacker, target);

    app.update();

    let health = health_of(&mut app, target);
    assert_eq!(health.current, 0, "health clamps at zero, not negative");
    assert!(!health.is_alive());
    assert_eq!(
        visibility_of(&mut app, target),
        Visibility::Hidden,
        "the Died observer hides the defeated sprite"
    );
}

/// An attacker with non-positive effective attack still floors to 1 — but an
/// attack from an entity with `attack <= 0` deals 0 per `compute_damage`, so the
/// target is untouched and a `DamageDealt` of 0 is the only signal. This mirrors
/// the `BattleEventsTest` "zero attack emits nothing meaningful" case: zero
/// attack ⇒ zero damage.
#[test]
fn zero_attack_deals_no_damage() {
    let mut app = combat_app();
    let attacker = spawn_attacker(&mut app, "Weakling", 0, 0);
    let target = spawn_target(&mut app, "Goblin", 80);
    queue_attack(&mut app, attacker, target);

    app.update();

    let dealt = drain_damage(&mut app);
    assert_eq!(dealt.len(), 1);
    assert_eq!(dealt[0].amount, 0, "attack <= 0 deals zero damage");
    assert_eq!(
        health_of(&mut app, target).current,
        80,
        "a zero-damage hit leaves health untouched"
    );
    assert_eq!(
        visibility_of(&mut app, target),
        Visibility::Visible,
        "no death, so the sprite stays visible"
    );
}

/// An attack targeting an already-dead enemy is skipped entirely: no second
/// `DamageDealt`, no re-trigger of `Died`.
#[test]
fn attack_on_dead_target_is_skipped() {
    let mut app = combat_app();
    let attacker = spawn_attacker(&mut app, "Hero", 50, 0);
    let target = spawn_target(&mut app, "Goblin", 30);

    // First attack kills it.
    queue_attack(&mut app, attacker, target);
    app.update();
    let _ = drain_damage(&mut app);

    // Second attack on the corpse does nothing.
    queue_attack(&mut app, attacker, target);
    app.update();
    assert!(
        drain_damage(&mut app).is_empty(),
        "no DamageDealt for an attack on a dead target"
    );
}
