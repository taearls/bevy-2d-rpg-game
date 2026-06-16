//! Battle spawning: rolls a random enemy roster and places the player and
//! enemies into the world.
//!
//! Mirrors the Godot `BattleScene.SpawnEnemies` flow: seed an RNG (pinned from
//! `battle.seed` when present, else entropy), roll 1..=`MAX_ENEMIES` enemies
//! from the roster, suffix duplicate display names, and lay the enemies out in a
//! horizontal row from [`BattleLayout`]. The roll itself ([`roll_roster`]) is a
//! pure function over the RNG and roster so it can be asserted headlessly
//! without spawning entities or loading assets.

use bevy::prelude::*;
use rand::Rng;
use rand_chacha::ChaCha8Rng;

use crate::characters::components::{
    CombatStats, DamageVariance, DisplayName, Enemy, Health, Player,
};
use crate::characters::definition::CharacterDef;

use super::naming::suffix_duplicate_names;
use super::rng::{DamageRng, SpawnRng};
use super::seed::read_seed_file;
use super::targeting::{SelectionIndicator, on_enemy_clicked};
use super::ui::hud::spawn_enemy_health_bar;

/// Maximum number of enemies a battle can spawn. The count is rolled inclusively
/// in `1..=MAX_ENEMIES`, matching Godot `RandiRange(1, MaxEnemies)`.
pub const MAX_ENEMIES: usize = 4;

/// Horizontal row layout for the enemy lineup, in Bevy world units (Y-up,
/// origin at the window centre). The player sits at [`Self::player`]. These are
/// the knobs the Phase 8 inspector tunes (Godot `[Export(Range)]` parity).
#[derive(Resource, Reflect, Debug, Clone, Copy, PartialEq)]
#[reflect(Resource)]
pub struct BattleLayout {
    /// X of the first enemy (index 0).
    pub enemy_start_x: f32,
    /// X gap between consecutive enemies.
    pub enemy_spacing: f32,
    /// Shared Y of the enemy row.
    pub enemy_y: f32,
    /// Vertical offset of the selection indicator above a targeted enemy
    /// (consumed in Phase 5; defined here alongside the other layout knobs).
    pub indicator_offset: f32,
    /// Position of the player character.
    pub player: Vec2,
}

impl Default for BattleLayout {
    fn default() -> Self {
        // Enemies fill a row on the left, player faces them from the right —
        // the Godot scene's arrangement translated into Bevy's centred space.
        Self {
            enemy_start_x: -400.0,
            enemy_spacing: 150.0,
            enemy_y: 60.0,
            indicator_offset: 80.0,
            player: Vec2::new(420.0, 60.0),
        }
    }
}

impl BattleLayout {
    /// World position of the enemy at row slot `index`.
    #[must_use]
    pub fn enemy_position(&self, index: usize) -> Vec2 {
        Vec2::new(
            self.enemy_start_x + index as f32 * self.enemy_spacing,
            self.enemy_y,
        )
    }
}

/// A single enemy chosen for a battle: the template it was rolled from plus the
/// (possibly suffixed) display name it should spawn with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RosterEntry {
    pub def: CharacterDef,
    pub display_name: String,
}

/// Roll an enemy roster from `roster` using `rng`.
///
/// Picks a count in `1..=MAX_ENEMIES`, then that many templates uniformly (with
/// replacement), then disambiguates duplicate display names ("Goblin A/B/…").
/// Returns an empty `Vec` when `roster` is empty — no count is rolled, matching
/// Godot's early return on an empty `EnemyStatsList` so the RNG stream is
/// untouched.
///
/// Pure over `(rng, roster)`: the same seeded `ChaCha8Rng` and roster always
/// yield the same result, which is what the headless spawn tests assert.
#[must_use]
pub fn roll_roster(rng: &mut ChaCha8Rng, roster: &[CharacterDef]) -> Vec<RosterEntry> {
    if roster.is_empty() {
        return Vec::new();
    }

    let count = rng.random_range(1..=MAX_ENEMIES);
    let picks: Vec<&CharacterDef> = (0..count)
        .map(|_| &roster[rng.random_range(0..roster.len())])
        .collect();

    let names: Vec<&str> = picks.iter().map(|def| def.display_name.as_str()).collect();
    let suffixed = suffix_duplicate_names(&names);

    picks
        .into_iter()
        .zip(suffixed)
        .map(|(def, display_name)| RosterEntry {
            def: def.clone(),
            display_name,
        })
        .collect()
}

/// Build a [`SpawnRng`] for this battle: pinned from `battle.seed` when that
/// file holds a valid integer, otherwise entropy-seeded for a fresh roll.
/// Mirrors Godot `LoadSeededRng`.
#[must_use]
pub fn spawn_rng_from_environment() -> SpawnRng {
    read_seed_file().map_or_else(SpawnRng::from_entropy, SpawnRng::from_seed)
}

/// Startup system: begin loading the hero and enemy templates (stashing their
/// handles in [`Roster`] so they stay resident) and insert the seeded
/// [`SpawnRng`]. The seed is read here, inside a system, rather than at
/// plugin-build time so a later phase can re-roll by re-running this without
/// rebuilding the app.
pub fn load_roster(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.insert_resource(Roster {
        hero: asset_server.load("characters/hero.character.ron"),
        enemies: vec![asset_server.load("characters/goblin.character.ron")],
    });
    commands.insert_resource(spawn_rng_from_environment());
    // Damage variance is entropy-seeded for live play; headless tests insert a
    // fixed-seed `DamageRng` so the variance roll — and thus the damage — is
    // deterministic.
    commands.insert_resource(DamageRng::from_entropy());
}

/// Spawn the player and a freshly rolled enemy row.
///
/// Reads the `SpawnRng` and `BattleLayout` resources and the loaded `hero` /
/// `goblin` templates from [`Roster`], then defers entity creation to
/// [`spawn_player`] / [`spawn_enemies`]. Runs once, when a battle starts. The 2D
/// camera is spawned globally by `GamePlugin` (shared with the menu), so it is
/// not created here.
pub fn spawn_battle(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut spawn_rng: ResMut<SpawnRng>,
    layout: Res<BattleLayout>,
    roster: Res<Roster>,
    defs: Res<Assets<CharacterDef>>,
) {
    let Some(hero) = defs.get(&roster.hero) else {
        error!("hero template missing; skipping spawn");
        return;
    };
    spawn_player(&mut commands, &asset_server, hero, layout.player);

    let enemy_defs: Vec<CharacterDef> = roster
        .enemies
        .iter()
        .filter_map(|handle| defs.get(handle).cloned())
        .collect();
    let entries = roll_roster(&mut spawn_rng.0, &enemy_defs);
    info!(
        "Spawning {} {}!",
        entries.len(),
        if entries.len() == 1 {
            "enemy"
        } else {
            "enemies"
        }
    );
    spawn_enemies(&mut commands, &asset_server, &layout, &entries);
}

/// Spawn the player entity from its template at `position`.
pub fn spawn_player(
    commands: &mut Commands,
    asset_server: &AssetServer,
    def: &CharacterDef,
    position: Vec2,
) {
    commands.spawn((
        Player,
        // Mirror horizontally so the hero faces the enemies on the left, matching
        // the Godot `HeroSprite` `flip_h = true`.
        Sprite {
            flip_x: true,
            ..Sprite::from_image(asset_server.load(def.sprite.clone()))
        },
        Transform::from_translation(position.extend(0.0)),
        DisplayName(def.display_name.clone()),
        Health::full(def.stats.max_health),
        CombatStats {
            attack: def.stats.attack,
            defense: def.stats.defense,
        },
        DamageVariance::default(),
    ));
}

/// Spawn one enemy entity per roster entry, laid out from `layout`.
pub fn spawn_enemies(
    commands: &mut Commands,
    asset_server: &AssetServer,
    layout: &BattleLayout,
    entries: &[RosterEntry],
) {
    for (index, entry) in entries.iter().enumerate() {
        let enemy = commands
            .spawn((
                Enemy { index },
                Sprite::from_image(asset_server.load(entry.def.sprite.clone())),
                Transform::from_translation(layout.enemy_position(index).extend(0.0)),
                DisplayName(entry.display_name.clone()),
                Health::full(entry.def.stats.max_health),
                CombatStats {
                    attack: entry.def.stats.attack,
                    defense: entry.def.stats.defense,
                },
                DamageVariance::default(),
                // Clickable for mouse targeting; the observer turns a click into
                // a select-and-confirm during the `Targeting` phase.
                Pickable::default(),
            ))
            .observe(on_enemy_clicked)
            .id();
        // The world-space name label + mini HP bar ride above the sprite, the bar
        // scaled by the enemy's health fraction by `sync_enemy_health_bars`.
        let name = entry.display_name.clone();
        commands
            .entity(enemy)
            .with_children(|parent| spawn_enemy_health_bar(parent, enemy, &name));
    }
}

/// Startup system: spawn the single, reusable selection indicator — a yellow
/// downward-pointing `Mesh2d(Triangle2d)` — hidden until targeting parks it over
/// an enemy.
///
/// One long-lived entity rather than a per-selection spawn:
/// [`update_target_visuals`](super::targeting::update_target_visuals) moves it
/// and toggles its visibility. The triangle points down so its lower vertex
/// indicates the enemy beneath it.
pub fn spawn_selection_indicator(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    // A downward-pointing triangle: apex below, base above.
    let triangle = Triangle2d::new(
        Vec2::new(0.0, -16.0),
        Vec2::new(-14.0, 16.0),
        Vec2::new(14.0, 16.0),
    );
    commands.spawn((
        SelectionIndicator,
        Mesh2d(meshes.add(triangle)),
        MeshMaterial2d(materials.add(Color::srgb(1.0, 1.0, 0.0))),
        Transform::default(),
        // Revealed by `update_target_visuals` once an enemy is targeted.
        Visibility::Hidden,
    ));
}

/// Handles to the loaded character templates, kept alive for the battle.
///
/// `hero` is the player template; `enemies` is the pool the spawn RNG rolls
/// from. Loaded at startup so the assets are resident before [`spawn_battle`]
/// runs.
#[derive(Resource, Debug)]
pub struct Roster {
    pub hero: Handle<CharacterDef>,
    pub enemies: Vec<Handle<CharacterDef>>,
}

impl Roster {
    /// Every template handle in the roster (hero first, then enemies), for
    /// checking load state uniformly.
    pub fn handles(&self) -> impl Iterator<Item = &Handle<CharacterDef>> {
        std::iter::once(&self.hero).chain(self.enemies.iter())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::characters::definition::CombatStatsDef;
    use rand_chacha::rand_core::SeedableRng;

    fn def(name: &str, max_health: i32, attack: i32, defense: i32) -> CharacterDef {
        CharacterDef {
            display_name: name.to_string(),
            sprite: "sprites/enemy.png".to_string(),
            stats: CombatStatsDef {
                max_health,
                attack,
                defense,
            },
        }
    }

    fn goblin_roster() -> Vec<CharacterDef> {
        vec![def("Goblin", 80, 10, 4)]
    }

    /// Rolled count always lands in the inclusive `1..=MAX_ENEMIES` band across
    /// many seeds (parity with Godot `RandiRange(1, MaxEnemies)`).
    #[test]
    fn rolled_count_is_within_one_to_max() {
        let roster = goblin_roster();
        for seed in 0..200u64 {
            let mut rng = ChaCha8Rng::seed_from_u64(seed);
            let entries = roll_roster(&mut rng, &roster);
            assert!(
                (1..=MAX_ENEMIES).contains(&entries.len()),
                "seed {seed} rolled {} enemies, expected 1..={MAX_ENEMIES}",
                entries.len()
            );
        }
    }

    /// Enemies carry the stats of the template they were rolled from.
    #[test]
    fn entries_carry_template_stats() {
        let roster = vec![def("Goblin", 80, 10, 4)];
        let mut rng = ChaCha8Rng::seed_from_u64(7);
        let entries = roll_roster(&mut rng, &roster);
        assert!(!entries.is_empty());
        for entry in &entries {
            assert_eq!(entry.def.stats.max_health, 80);
            assert_eq!(entry.def.stats.attack, 10);
            assert_eq!(entry.def.stats.defense, 4);
        }
    }

    /// Layout places enemy `i` at `start_x + i * spacing` on the shared row Y.
    #[test]
    fn enemy_positions_follow_start_plus_index_times_spacing() {
        let layout = BattleLayout {
            enemy_start_x: -300.0,
            enemy_spacing: 120.0,
            enemy_y: 40.0,
            ..BattleLayout::default()
        };
        for index in 0..MAX_ENEMIES {
            let pos = layout.enemy_position(index);
            assert!((pos.x - (-300.0 + index as f32 * 120.0)).abs() < f32::EPSILON);
            assert!((pos.y - 40.0).abs() < f32::EPSILON);
        }
    }

    /// Duplicate display names are lettered in order of appearance.
    #[test]
    fn duplicate_names_are_suffixed() {
        // A single-template roster guarantees duplicates whenever count > 1.
        let roster = goblin_roster();
        let entries = (0..u64::MAX)
            .map(|seed| roll_roster(&mut ChaCha8Rng::seed_from_u64(seed), &roster))
            .find(|entries| entries.len() > 1)
            .expect("some seed rolls more than one enemy");

        let names: Vec<&str> = entries.iter().map(|e| e.display_name.as_str()).collect();
        let expected: Vec<String> = (0..entries.len())
            .map(|i| format!("Goblin {}", (b'A' + i as u8) as char))
            .collect();
        assert_eq!(
            names,
            expected.iter().map(String::as_str).collect::<Vec<_>>()
        );
    }

    /// The same seed reproduces an identical roster (count, names, stats).
    #[test]
    fn same_seed_yields_identical_roster() {
        let roster = vec![def("Goblin", 80, 10, 4), def("Slime", 50, 8, 2)];
        let mut a = ChaCha8Rng::seed_from_u64(42);
        let mut b = ChaCha8Rng::seed_from_u64(42);
        assert_eq!(roll_roster(&mut a, &roster), roll_roster(&mut b, &roster));
    }

    /// An empty roster spawns nothing and leaves the RNG stream untouched
    /// (Godot's early return on an empty `EnemyStatsList`).
    #[test]
    fn empty_roster_rolls_nothing() {
        let mut rng = ChaCha8Rng::seed_from_u64(99);
        let mut untouched = rng.clone();
        let entries = roll_roster(&mut rng, &[]);
        assert!(entries.is_empty());
        // RNG untouched: next draw matches a stream that never saw the call.
        assert_eq!(rng.random::<u64>(), untouched.random::<u64>());
    }
}
