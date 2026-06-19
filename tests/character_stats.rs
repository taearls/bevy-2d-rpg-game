//! Parity with the Godot `CharacterStatsTest`: component composition and serde
//! defaults for the data-driven character templates.

use bevy_2d_rpg_game::characters::components::{CombatStats, DamageVariance, DisplayName, Health};
use bevy_2d_rpg_game::characters::definition::{CharacterDef, CombatStatsDef};

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
fn combat_stats_def_defaults_are_the_tuned_values() {
    // `attack`/`defense` mirror the Godot `CombatStats.cs` exports; `max_health`
    // is tuned down from Godot's 100 to 50 (see `definition.rs` module docs).
    let stats = CombatStatsDef::default();
    assert_eq!(stats.max_health, 50);
    assert_eq!(stats.attack, 10);
    assert_eq!(stats.defense, 5);
}

#[test]
fn character_def_uses_stat_defaults_when_omitted() {
    // Name and sprite are supplied; stats fall back to 50/10/5.
    let def: CharacterDef =
        ron::from_str(r#"(display_name: "Goblin", sprite: "sprites/enemy.png")"#).unwrap();
    assert_eq!(def.display_name, "Goblin");
    assert_eq!(def.sprite, "sprites/enemy.png");
    assert_eq!(def.stats, CombatStatsDef::default());
}

#[test]
fn combat_stats_def_fills_missing_fields_individually() {
    // attack is overridden; max_health and defense keep their defaults.
    let stats: CombatStatsDef = ron::from_str("(attack: 25)").unwrap();
    assert_eq!(stats.max_health, 50);
    assert_eq!(stats.attack, 25);
    assert_eq!(stats.defense, 5);
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
    };
    let serialized = ron::to_string(&original).unwrap();
    let restored: CharacterDef = ron::from_str(&serialized).unwrap();
    assert_eq!(original, restored);
}
