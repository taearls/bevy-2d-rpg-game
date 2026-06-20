//! Headless spawn coverage, mirroring the Godot `BattleSceneTest` spawn cases.
//!
//! Stats are injected directly (the issue's "stats injected directly" note):
//! these tests drive `spawn_player` / `spawn_enemies` against a minimal `App`
//! with hand-built roster entries, so no async asset loading or renderer is
//! involved. The RNG-driven roster selection itself is unit-tested in
//! `src/battle/spawn.rs`; here we assert the *spawned ECS state*.

use bevy::asset::AssetPlugin;
use bevy::ecs::system::RunSystemOnce;
use bevy::prelude::*;
use bevy::scene::ScenePlugin;

use bevy_2d_rpg_game::battle::spawn::{BattleLayout, RosterEntry, spawn_enemies, spawn_player};
use bevy_2d_rpg_game::characters::definition::{CharacterDef, CombatStatsDef};
use bevy_2d_rpg_game::components::{
    CombatStats, DamageVariance, DisplayName, Enemy, Health, Player,
};

/// Minimal headless world with the asset + scene infrastructure. `AssetServer`
/// mints texture handles for the `bsn!` `Sprite { image: ... }`, and `ScenePlugin`
/// backs the `spawn_scene` the enemy spawner now uses; `Image` is registered
/// explicitly so handles resolve without pulling in the renderer.
fn headless_app() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default(), ScenePlugin))
        .init_asset::<Image>();
    app
}

fn goblin_def(max_health: i32, attack: i32, defense: i32) -> CharacterDef {
    CharacterDef {
        display_name: "Goblin".to_string(),
        sprite: "sprites/enemy.png".to_string(),
        stats: CombatStatsDef {
            max_health,
            attack,
            defense,
        },
    }
}

fn entry(display_name: &str, def: CharacterDef) -> RosterEntry {
    RosterEntry {
        def,
        display_name: display_name.to_string(),
    }
}

/// Exact-position assertion. Layout values here are integer-valued, so the
/// computed coordinates are representable without rounding error; an
/// epsilon tolerance keeps clippy's pedantic float-cmp lint happy.
#[track_caller]
fn assert_close(actual: f32, expected: f32) {
    assert!(
        (actual - expected).abs() < f32::EPSILON,
        "expected {expected}, got {actual}"
    );
}

#[test]
fn spawns_one_player_with_template_stats() {
    let mut app = headless_app();
    let layout = BattleLayout::default();
    let hero = CharacterDef {
        display_name: "Hero".to_string(),
        sprite: "sprites/hero.png".to_string(),
        stats: CombatStatsDef {
            max_health: 120,
            attack: 12,
            defense: 8,
        },
    };

    app.world_mut()
        .run_system_once(
            move |mut commands: Commands, asset_server: Res<AssetServer>| {
                let max = hero.stats.max_health;
                spawn_player(&mut commands, &asset_server, &hero, layout.player, max);
            },
        )
        .unwrap();

    let mut q = app
        .world_mut()
        .query_filtered::<(&DisplayName, &Health, &CombatStats, &Transform), With<Player>>();
    let players: Vec<_> = q.iter(app.world()).collect();
    assert_eq!(players.len(), 1);
    let (name, hp, stats, transform) = players[0];
    assert_eq!(name.0, "Hero");
    assert_eq!(hp.current, 120);
    assert_eq!(hp.max, 120);
    assert_eq!(stats.attack, 12);
    assert_eq!(stats.defense, 8);
    let pos = transform.translation.truncate();
    assert_close(pos.x, layout.player.x);
    assert_close(pos.y, layout.player.y);
}

#[test]
fn spawns_enemies_with_correct_stats_indices_and_spacing() {
    let mut app = headless_app();
    let layout = BattleLayout {
        enemy_start_x: -300.0,
        enemy_spacing: 120.0,
        enemy_y: 40.0,
        ..BattleLayout::default()
    };
    let entries = vec![
        entry("Goblin A", goblin_def(80, 10, 4)),
        entry("Goblin B", goblin_def(80, 10, 4)),
        entry("Slime", goblin_def(50, 8, 2)),
    ];
    let expected = entries.clone();

    app.world_mut()
        .run_system_once(
            move |mut commands: Commands, asset_server: Res<AssetServer>| {
                spawn_enemies(&mut commands, &asset_server, &layout, &entries);
            },
        )
        .unwrap();

    let mut q = app.world_mut().query_filtered::<(
        &Enemy,
        &DisplayName,
        &Health,
        &CombatStats,
        &DamageVariance,
        &Transform,
    ), With<Enemy>>();
    let mut spawned: Vec<_> = q.iter(app.world()).collect();
    spawned.sort_by_key(|(enemy, ..)| enemy.index);

    assert_eq!(spawned.len(), expected.len());
    for (i, (enemy, name, hp, stats, variance, transform)) in spawned.iter().enumerate() {
        assert_eq!(enemy.index, i, "indices run 0..n in order");
        assert_eq!(name.0, expected[i].display_name);
        assert_eq!(hp.current, expected[i].def.stats.max_health);
        assert_eq!(hp.max, expected[i].def.stats.max_health);
        assert_eq!(stats.attack, expected[i].def.stats.attack);
        assert_eq!(stats.defense, expected[i].def.stats.defense);
        assert_eq!(**variance, DamageVariance::default());

        // Horizontal spacing: x = start_x + i * spacing, shared row Y.
        let pos = transform.translation.truncate();
        assert_close(pos.x, -300.0 + i as f32 * 120.0);
        assert_close(pos.y, 40.0);
    }
}

#[test]
fn empty_roster_spawns_no_enemies() {
    let mut app = headless_app();
    let layout = BattleLayout::default();

    app.world_mut()
        .run_system_once(
            move |mut commands: Commands, asset_server: Res<AssetServer>| {
                spawn_enemies(&mut commands, &asset_server, &layout, &[]);
            },
        )
        .unwrap();

    let mut q = app.world_mut().query_filtered::<Entity, With<Enemy>>();
    assert_eq!(q.iter(app.world()).count(), 0);
}
