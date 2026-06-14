//! Battle orchestration: RNG, seeding, roster naming, spawning, turn flow,
//! the action menu, enemy targeting, and combat resolution.

pub mod enemy_turn;
pub mod menu;
pub mod messages;
pub mod naming;
pub mod rng;
pub mod seed;
pub mod spawn;
pub mod state;
pub mod targeting;

use bevy::asset::LoadState;
use bevy::prelude::*;
use bevy::sprite::{SpritePickingMode, SpritePickingSettings};

use enemy_turn::{EnemyTurnQueue, on_enter_enemy_turn, tick_enemy_turn};
use menu::{
    MenuSelection, menu_input, on_enter_player_turn, spawn_action_menu, update_menu_highlight,
};
use messages::{LogMessage, render_log_messages};
use spawn::{BattleLayout, Roster, load_roster, spawn_battle, spawn_selection_indicator};
use state::{BattleSet, TurnPhase};
use targeting::{
    SelectedTarget, on_enter_targeting, on_exit_targeting, targeting_input, update_target_visuals,
};

use crate::combat::events::{AttackRequested, DamageDealt};
use crate::combat::resolve::{apply_attacks, check_battle_end, on_died_hide_sprite};

/// Drives battle setup and turn flow: seeds the spawn RNG, loads the character
/// roster, spawns the player + enemy lineup once the templates finish loading,
/// and runs the [`TurnPhase`] state machine with its chained [`BattleSet`]s, the
/// player action menu, enemy targeting, and combat resolution.
pub struct BattlePlugin;

impl Plugin for BattlePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BattleLayout>()
            .init_resource::<MenuSelection>()
            .init_resource::<SelectedTarget>()
            .init_resource::<EnemyTurnQueue>()
            // Full-rectangle sprite hits, matching the Godot click areas, instead
            // of the default alpha-threshold test.
            .insert_resource(SpritePickingSettings {
                picking_mode: SpritePickingMode::BoundingBox,
                ..default()
            })
            .init_state::<TurnPhase>()
            .add_message::<LogMessage>()
            .add_message::<AttackRequested>()
            .add_message::<DamageDealt>()
            .add_observer(on_died_hide_sprite)
            // The four battle phases run in a fixed order every frame: input
            // queues attacks, Resolve applies them, Cleanup decides the battle's
            // fate, Ui redraws from the resulting world state.
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
            .add_systems(
                Startup,
                (load_roster, spawn_action_menu, spawn_selection_indicator),
            )
            .add_systems(OnEnter(TurnPhase::PlayerTurn), on_enter_player_turn)
            .add_systems(OnEnter(TurnPhase::Targeting), on_enter_targeting)
            .add_systems(OnExit(TurnPhase::Targeting), on_exit_targeting)
            .add_systems(OnEnter(TurnPhase::EnemyTurn), on_enter_enemy_turn)
            .add_systems(
                Update,
                (
                    report_roster_load_failures,
                    spawn_battle.run_if(roster_ready.and(run_once)),
                    menu_input
                        .in_set(BattleSet::Input)
                        .run_if(in_state(TurnPhase::PlayerTurn)),
                    targeting_input
                        .in_set(BattleSet::Input)
                        .run_if(in_state(TurnPhase::Targeting)),
                    tick_enemy_turn
                        .in_set(BattleSet::Input)
                        .run_if(in_state(TurnPhase::EnemyTurn)),
                    apply_attacks.in_set(BattleSet::Resolve),
                    // Decide the battle's fate only on the frame an attack
                    // actually landed. Gating on `DamageDealt` (written by
                    // `apply_attacks`) rather than on the `Targeting` state keeps
                    // a *cancelled* targeting (Escape → PlayerTurn, no attack)
                    // from being overridden into EnemyTurn/BattleOver.
                    check_battle_end
                        .in_set(BattleSet::Cleanup)
                        .run_if(on_message::<DamageDealt>),
                    update_menu_highlight.in_set(BattleSet::Ui),
                    update_target_visuals.in_set(BattleSet::Ui),
                    render_log_messages.in_set(BattleSet::Ui),
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
