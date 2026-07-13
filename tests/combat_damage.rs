//! Parity with the Godot `BattleCharacterTest` damage subset, exercising the
//! pure `compute_damage` formula and how it reduces a `Health` component.
//!
//! Note the deliberate divergence documented on `compute_damage`: this port
//! rounds where the C# original truncated, so a few of these expectations
//! differ from Godot by at most one point (called out inline).

use aliasing::combat::compute_damage;
use aliasing::components::Health;

/// Apply a computed hit to a health pool the way later combat systems will:
/// subtract and floor at zero.
fn take_damage(hp: &mut Health, attack: i32, defense: i32, variance: f32) -> i32 {
    let dmg = compute_damage(attack, defense, variance);
    hp.current = (hp.current - dmg).max(0);
    dmg
}

#[test]
fn damage_reduces_health() {
    let mut hp = Health::full(100);
    let dmg = take_damage(&mut hp, 20, 5, 1.0);
    assert_eq!(dmg, 15); // max(1, 20 - 5) * 1.0 = 15
    assert_eq!(hp.current, 85);
    assert!(hp.is_alive());
}

#[test]
fn damage_is_at_least_one_when_defense_exceeds_attack() {
    // 5 - 10 = -5 → base floored to 1 → 1 * 1.0 = 1.
    let dmg = compute_damage(5, 10, 1.0);
    assert_eq!(dmg, 1);
}

#[test]
fn damage_is_at_least_one_after_variance_rounds_below_one() {
    // base 1 * 0.4 = 0.4 → rounds to 0 → floored back up to 1.
    let dmg = compute_damage(11, 10, 0.4);
    assert_eq!(dmg, 1);
}

#[test]
fn health_floors_at_zero_and_is_not_alive() {
    let mut hp = Health {
        current: 10,
        max: 100,
    };
    let dmg = take_damage(&mut hp, 1000, 0, 1.0);
    assert_eq!(dmg, 1000);
    assert_eq!(hp.current, 0); // never goes negative
    assert!(!hp.is_alive());
}

#[test]
fn zero_attack_deals_no_damage() {
    assert_eq!(compute_damage(0, 5, 1.0), 0);
}

#[test]
fn negative_attack_deals_no_damage() {
    assert_eq!(compute_damage(-3, 5, 1.0), 0);
}

#[test]
fn halved_attack_deals_proportionally_less() {
    // A Defending enemy attacks at half strength next turn. Against 0 defense:
    // attack 20 → 20; halved attack 10 → 10.
    let full = compute_damage(20, 0, 1.0);
    let halved = compute_damage(20 / 2, 0, 1.0);
    assert_eq!(full, 20);
    assert_eq!(halved, 10);
}

#[test]
fn variance_scales_within_the_spread() {
    let base = 30 - 10; // 20
    let low = compute_damage(30, 10, 0.8);
    let high = compute_damage(30, 10, 1.2);
    assert_eq!(low, (base as f32 * 0.8).round() as i32); // 16
    assert_eq!(high, (base as f32 * 1.2).round() as i32); // 24
}

#[test]
fn rounding_diverges_from_godot_truncation() {
    // Control: base 7 * 1.2 = 8.4 → both truncation and rounding give 8.
    assert_eq!(compute_damage(17, 10, 1.2), 8);
    // Divergent: base 5 * 1.1 = 5.5 → truncation (Godot) gives 5; we round to 6.
    assert_eq!(compute_damage(15, 10, 1.1), 6);
    // Divergent: base 6 * 1.15 = 6.9 → truncation gives 6; we round to 7.
    assert_eq!(compute_damage(16, 10, 1.15), 7);
}
