//! Persistent player progress carried across screens.
//!
//! A battle spawns a fresh player entity each time it starts and despawns it on
//! exit (see [`spawn_battle`](crate::battle::spawn::spawn_battle) and the
//! `DespawnOnExit(InBattle)` tagging), so the player's hit points cannot live on
//! the combatant entity between fights. [`PlayerProgress`] is the small resource
//! that survives those transitions: the battle reads it to seed the player's
//! starting health, and a victory writes the surviving health back so it carries
//! into the next encounter. A fresh game or a restart clears it back to "full".

use bevy::prelude::*;

use crate::battle::spawn::Roster;
use crate::characters::definition::CharacterDef;
use crate::components::Health;

/// The player's persistent state between battles.
///
/// `health` is `None` until [`seed_player_progress`] fills it from the hero
/// template's max health (which is also what New Game / Restart reset it to, via
/// [`Self::reset`]). Once a battle has been fought, a victory stores the player's
/// surviving [`Health`] here so the next fight starts from the carried-over
/// total.
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PlayerProgress {
    /// The player's carried-over hit points, or `None` before the hero template
    /// has loaded (treated as "full health" by the battle spawn).
    pub health: Option<Health>,
}

impl PlayerProgress {
    /// Clear progress so the next [`seed_player_progress`] reseeds the player to
    /// full health. Called when a new game starts or the player restarts after a
    /// game over.
    pub fn reset(&mut self) {
        self.health = None;
    }
}

/// Seed [`PlayerProgress`] to full health from the hero template once it has
/// loaded, but only while it is unset (`None`).
///
/// Runs every frame and early-outs cheaply once `health` is `Some`, so it costs
/// nothing in the steady state. Leaving the seeding to a system — rather than
/// inserting a value at startup — sidesteps the async asset load: the hero
/// template is not resident the instant the app boots, but this fills the
/// progress as soon as it is, which is well before the first encounter. After a
/// battle the field is `Some`, so a damaged total is never clobbered back to
/// full; only [`PlayerProgress::reset`] reopens it for reseeding.
pub fn seed_player_progress(
    mut progress: ResMut<PlayerProgress>,
    roster: Option<Res<Roster>>,
    defs: Res<Assets<CharacterDef>>,
) {
    if progress.health.is_some() {
        return;
    }
    let Some(roster) = roster else {
        return;
    };
    if let Some(hero) = defs.get(&roster.hero) {
        progress.health = Some(Health::full(hero.stats.max_health));
    }
}
