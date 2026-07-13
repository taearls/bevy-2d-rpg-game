//! Parity with the Godot `CharacterStatsTest`: component composition and RON
//! deserialization for the data-driven character templates.

use aliasing::characters::definition::{CharacterDef, CombatStatsDef, DamageVarianceDef};
use aliasing::components::{CombatStats, DamageVariance, DisplayName, Health};

#[test]
fn health_full_starts_at_max_and_is_alive() {
    let hp = Health::full(100);
    assert_eq!(hp.current, 100);
    assert_eq!(hp.max, 100);
    assert!(hp.is_alive());
}

#[test]
fn health_is_not_alive_at_zero() {
    let hp = Health {
        current: 0,
        max: 100,
    };
    assert!(!hp.is_alive());
}

#[test]
fn components_compose_into_a_character_profile() {
    let name = DisplayName("Hero".to_string());
    let stats = CombatStats {
        attack: 12,
        defense: 8,
    };
    let variance = DamageVariance::default();

    assert_eq!(name.0, "Hero");
    assert_eq!(stats.attack, 12);
    assert_eq!(stats.defense, 8);
    assert_eq!(variance, DamageVariance { min: 0.8, max: 1.2 });
}

#[test]
fn damage_variance_defaults_match_godot_exports() {
    let variance = DamageVariance::default();
    assert_eq!(variance, DamageVariance { min: 0.8, max: 1.2 });
}

#[test]
fn character_def_uses_stat_values() {
    let test_display_name = "Goblin";
    let test_sprite = "sprites/enemy.png";
    let test_attack = 10;
    let test_max_health = 50;
    let test_defense = 5;
    let test_variance_min = 0.8;
    let test_variance_max = 1.2;
    let combat_stats_def_str =
        format!("attack: {test_attack}, max_health: {test_max_health}, defense: {test_defense}");
    let damage_variance_def_str = format!("min: {test_variance_min}, max: {test_variance_max}");
    let character_def: CharacterDef = ron::from_str(&format!(
        r#"(display_name: "{test_display_name}", sprite: "{test_sprite}", stats: ({combat_stats_def_str}), damage_variance: ({damage_variance_def_str}))"#
    ))
    .unwrap();

    assert_eq!(character_def.display_name, test_display_name);
    assert_eq!(character_def.sprite, test_sprite);
    assert_eq!(
        character_def.stats,
        CombatStatsDef {
            max_health: test_max_health,
            attack: test_attack,
            defense: test_defense,
        }
    );
    assert_eq!(
        character_def.damage_variance,
        DamageVarianceDef {
            min: test_variance_min,
            max: test_variance_max,
        }
    );
}

#[test]
fn character_def_round_trips_through_ron() {
    let original = CharacterDef {
        display_name: "Hero".to_string(),
        sprite: "sprites/hero.png".to_string(),
        stats: CombatStatsDef {
            max_health: 120,
            attack: 12,
            defense: 8,
        },
        damage_variance: DamageVarianceDef { min: 0.8, max: 1.2 },
    };
    let serialized = ron::to_string(&original).unwrap();
    let restored: CharacterDef = ron::from_str(&serialized).unwrap();
    assert_eq!(original, restored);
}

/// The RON assets are the source of truth: with the serde defaults removed, a
/// template that omits any stat field must now fail to deserialize rather than
/// silently fall back. This locks in the no-defaults contract.
#[test]
fn character_def_rejects_missing_stat_field() {
    // `attack` is omitted from `stats` — previously it defaulted to 10.
    let result: Result<CharacterDef, _> = ron::from_str(
        r#"(display_name: "Goblin", sprite: "sprites/enemy.png", stats: (max_health: 80, defense: 4), damage_variance: (min: 0.8, max: 1.2))"#,
    );
    assert!(
        result.is_err(),
        "a template missing a stat field must not deserialize now that defaults are gone"
    );
}

/// A template omitting the whole `damage_variance` block must also fail — the
/// new field is required, not defaulted.
#[test]
fn character_def_rejects_missing_damage_variance() {
    let result: Result<CharacterDef, _> = ron::from_str(
        r#"(display_name: "Goblin", sprite: "sprites/enemy.png", stats: (max_health: 80, attack: 10, defense: 4))"#,
    );
    assert!(
        result.is_err(),
        "a template missing damage_variance must not deserialize"
    );
}
