//! Battle orchestration: RNG, seeding, roster naming, spawning, and (later
//! phases) turn flow and combat resolution.

pub mod naming;
pub mod rng;
pub mod seed;
pub mod spawn;

use bevy::prelude::*;

use crate::characters::definition::CharacterDef;

use spawn::{BattleLayout, Roster, spawn_battle, spawn_rng_from_environment};

/// Drives battle setup: seeds the spawn RNG, loads the character roster, and
/// spawns the player + enemy lineup once the templates finish loading.
pub struct BattlePlugin;

impl Plugin for BattlePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BattleLayout>()
            .insert_resource(spawn_rng_from_environment())
            .add_systems(Startup, load_roster)
            .add_systems(Update, spawn_battle.run_if(roster_ready.and(run_once)));
    }
}

/// Kick off loading of the hero and enemy templates and stash their handles in
/// the [`Roster`] resource so they stay resident.
fn load_roster(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.insert_resource(Roster {
        hero: asset_server.load("characters/hero.character.ron"),
        enemies: vec![asset_server.load("characters/goblin.character.ron")],
    });
}

/// Gate that turns true once every roster template has finished loading, so the
/// one-shot spawn does not run against missing assets.
fn roster_ready(roster: Option<Res<Roster>>, defs: Res<Assets<CharacterDef>>) -> bool {
    let Some(roster) = roster else {
        return false;
    };
    defs.contains(&roster.hero) && roster.enemies.iter().all(|handle| defs.contains(handle))
}
