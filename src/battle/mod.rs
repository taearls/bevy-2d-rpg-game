//! Battle orchestration: RNG, seeding, roster naming, spawning, and (later
//! phases) turn flow and combat resolution.

pub mod naming;
pub mod rng;
pub mod seed;
pub mod spawn;

use bevy::asset::LoadState;
use bevy::prelude::*;

use spawn::{BattleLayout, Roster, load_roster, spawn_battle};

/// Drives battle setup: seeds the spawn RNG, loads the character roster, and
/// spawns the player + enemy lineup once the templates finish loading.
pub struct BattlePlugin;

impl Plugin for BattlePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BattleLayout>()
            .add_systems(Startup, load_roster)
            .add_systems(
                Update,
                (
                    report_roster_load_failures,
                    spawn_battle.run_if(roster_ready.and(run_once)),
                ),
            );
    }
}

/// Gate that turns true once every roster template has finished loading, so the
/// one-shot spawn does not run against missing or failed assets.
fn roster_ready(roster: Option<Res<Roster>>, asset_server: Res<AssetServer>) -> bool {
    let Some(roster) = roster else {
        return false;
    };
    roster
        .handles()
        .all(|handle| asset_server.is_loaded(handle))
}

/// Surface a roster asset that failed to load loudly and exactly once, rather
/// than letting [`roster_ready`] silently keep the spawn dormant forever (e.g.
/// a malformed `*.character.ron`). Runs every frame but logs each failed handle
/// a single time, tracked by `reported`.
fn report_roster_load_failures(
    roster: Option<Res<Roster>>,
    asset_server: Res<AssetServer>,
    mut reported: Local<bool>,
) {
    if *reported {
        return;
    }
    let Some(roster) = roster else {
        return;
    };
    for handle in roster.handles() {
        if let Some(LoadState::Failed(error)) = asset_server.get_load_state(handle.id()) {
            error!(
                "character template {:?} failed to load: {error}",
                handle.path()
            );
            *reported = true;
        }
    }
}
