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
pub mod ui;

use bevy::asset::LoadState;
use bevy::prelude::*;
use bevy::sprite::{SpritePickingMode, SpritePickingSettings};

use crate::combat::events::{AttackRequested, DamageDealt};
use crate::combat::resolve::{apply_attacks, check_battle_end, on_died_hide_sprite};
use crate::components::{CombatStats, DamageVariance, Health, Player};
use enemy_turn::{EnemyTurnQueue, on_enter_enemy_turn, tick_enemy_turn};
use menu::{
    LogView, MenuSelection, close_log_view_on_player_turn, log_overlay_input, menu_input,
    on_enter_player_turn, spawn_action_menu, update_menu_highlight,
};
use messages::{LogMessage, render_log_messages};
use spawn::{BattleLayout, Roster, load_roster, spawn_battle, spawn_selection_indicator};
use state::{BattleResult, BattleSet, TurnPhase};
use targeting::{
    SelectedTarget, on_enter_targeting, on_exit_targeting, targeting_input, update_target_visuals,
};
// Marker / label components — only registered for reflection under the debug
// inspector, so their import is gated to the same feature to keep a default
// build free of unused-import warnings.
#[cfg(feature = "debug-inspector")]
use crate::components::{Defending, DisplayName, Enemy, EnemyHealthBar, Targeted};
use crate::progress::PlayerProgress;
use crate::state::GameState;

/// Drives battle setup and turn flow: seeds the spawn RNG, loads the character
/// roster, spawns the player + enemy lineup once the templates finish loading,
/// and runs the [`TurnPhase`] state machine with its chained [`BattleSet`]s, the
/// player action menu, enemy targeting, and combat resolution.
pub(crate) fn plugin(app: &mut App) {
    app.add_plugins(ui::plugin)
        // Register the former Godot `[Export(Range)]` tuning knobs for
        // reflection so the Phase 8 inspector can edit them live. Registered
        // here in the plugin that wires these types into the battle (the
        // `UiConfig` knob is registered alongside in `ui::plugin`).
        // These tuning-knob types stay feature-independent; the inspector-only
        // marker components are gated to `debug-inspector` below.
        .register_type::<BattleLayout>()
        .register_type::<Health>()
        .register_type::<CombatStats>()
        .register_type::<DamageVariance>()
        .init_resource::<BattleLayout>()
        .init_resource::<MenuSelection>()
        .init_resource::<LogView>()
        .init_resource::<SelectedTarget>()
        .init_resource::<EnemyTurnQueue>()
        .init_resource::<BattleResult>()
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
        // fate, Ui redraws from the resulting world state. The whole chain is
        // gated on `InBattle` so none of it runs — and the keyboard isn't
        // double-read against the main menu — while the start-up menu is up.
        .configure_sets(
            Update,
            (
                BattleSet::Input,
                BattleSet::Resolve,
                BattleSet::Cleanup,
                BattleSet::Ui,
            )
                .chain()
                .run_if(in_state(GameState::InBattle)),
        )
        // Preload the roster at startup so the templates are resident the
        // instant the player picks "New Game"; the combatant + action-menu
        // entities, by contrast, are spawned only when a battle actually
        // begins so they never sit behind the menu.
        .add_systems(Startup, load_roster)
        .add_systems(
            OnEnter(GameState::InBattle),
            (
                reset_turn_phase,
                spawn_action_menu,
                spawn_selection_indicator,
            ),
        )
        .add_systems(OnEnter(TurnPhase::BattleOver), log_continue_hint)
        .add_systems(
            OnEnter(TurnPhase::PlayerTurn),
            (on_enter_player_turn, close_log_view_on_player_turn),
        )
        .add_systems(OnEnter(TurnPhase::Targeting), on_enter_targeting)
        .add_systems(OnExit(TurnPhase::Targeting), on_exit_targeting)
        .add_systems(OnEnter(TurnPhase::EnemyTurn), on_enter_enemy_turn)
        .add_systems(
            Update,
            (
                report_roster_load_failures,
                spawn_battle.run_if(
                    in_state(GameState::InBattle)
                        .and_then(roster_ready)
                        .and_then(battle_unspawned),
                ),
                battle_over_input.run_if(
                    in_state(GameState::InBattle).and_then(in_state(TurnPhase::BattleOver)),
                ),
                // The log-overlay close key runs before `menu_input` (which
                // early-returns while the overlay is open) so Escape / Enter
                // closes the log without also confirming a menu row.
                log_overlay_input
                    .in_set(BattleSet::Input)
                    .before(menu_input)
                    .run_if(in_state(TurnPhase::PlayerTurn)),
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

    // The marker / label components carry no tuning knobs — they're only
    // worth reflecting so the debug inspector can expand every component on a
    // combatant instead of showing them opaque. Gated on the feature so a
    // release build doesn't register reflection it never reads.
    #[cfg(feature = "debug-inspector")]
    app.register_type::<Player>()
        .register_type::<Enemy>()
        .register_type::<DisplayName>()
        .register_type::<Defending>()
        .register_type::<Targeted>()
        .register_type::<EnemyHealthBar>();
}

/// Gate that turns true while no combatants are spawned, so [`spawn_battle`] runs
/// exactly once per entry into [`GameState::InBattle`].
///
/// Replaces a global `run_once`: because the combatants are
/// `DespawnOnExit(InBattle)`, the player query is empty again on the next
/// encounter, re-opening this gate so each battle spawns a fresh lineup. Once the
/// player exists the gate closes for the rest of that fight.
fn battle_unspawned(players: Query<(), With<Player>>) -> bool {
    players.is_empty()
}

/// `OnEnter(InBattle)`: reset the per-battle turn flow to the player's turn.
///
/// A finished battle leaves [`TurnPhase`] on [`BattleOver`](TurnPhase::BattleOver);
/// re-entering a battle (from a fresh map encounter) must rewind it so the action
/// menu accepts input again. Setting the same value on the very first battle —
/// where it is already [`PlayerTurn`](TurnPhase::PlayerTurn) — is a harmless
/// no-op.
pub fn reset_turn_phase(mut next: ResMut<NextState<TurnPhase>>) {
    next.set(TurnPhase::PlayerTurn);
}

/// `OnEnter(BattleOver)`: prompt the player to acknowledge the result, which
/// [`battle_over_input`] then acts on. Appended to the existing "Victory!" /
/// "Game Over!" line that `check_battle_end` already logged.
fn log_continue_hint(mut log: MessageWriter<LogMessage>) {
    log.write(LogMessage::new("Press Enter to continue."));
}

/// `Update`, in `InBattle` + [`BattleOver`](TurnPhase::BattleOver): on Enter,
/// leave the battle for the right next screen.
///
/// A victory persists the player's surviving [`Health`] into [`PlayerProgress`]
/// (so it carries into the next encounter) and returns to the
/// [`Map`](GameState::Map); a defeat moves to the
/// [`GameOver`](GameState::GameOver) screen. The combatants and battle UI are torn
/// down by their `DespawnOnExit(InBattle)` tagging as the state changes.
pub fn battle_over_input(
    keys: Res<ButtonInput<KeyCode>>,
    result: Res<BattleResult>,
    mut progress: ResMut<PlayerProgress>,
    mut next_state: ResMut<NextState<GameState>>,
    player: Query<&Health, With<Player>>,
) {
    if !keys.just_pressed(KeyCode::Enter) {
        return;
    }
    if result.victory {
        if let Ok(health) = player.single() {
            progress.health = Some(*health);
        }
        next_state.set(GameState::Map);
    } else {
        next_state.set(GameState::GameOver);
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

#[cfg(test)]
mod tests {
    use bevy::ecs::system::RunSystemOnce;

    use super::*;

    /// The `battle_unspawned` spawn gate must re-arm once the combatants are gone.
    ///
    /// On the first entry into `InBattle` no player exists, so the gate is open
    /// (`spawn_battle` runs); once a player is present it closes for the rest of
    /// the fight. Because combatants are `DespawnOnExit(InBattle)`, leaving the
    /// battle empties the player query again, which must re-open the gate so the
    /// *next* map encounter spawns a fresh lineup. This locks in that round-trip
    /// without standing up the async asset loader.
    #[test]
    fn battle_unspawned_gate_rearms_after_player_despawns() {
        let mut world = World::new();

        // No combatants yet (first encounter): the gate is open.
        assert!(
            world.run_system_once(battle_unspawned).unwrap(),
            "an empty world spawns a battle"
        );

        // A player is on screen (mid-fight): the gate is closed.
        let player = world.spawn(Player).id();
        assert!(
            !world.run_system_once(battle_unspawned).unwrap(),
            "a present player keeps the battle from re-spawning"
        );

        // Battle ended → `DespawnOnExit(InBattle)` removed the player: re-armed.
        world.despawn(player);
        assert!(
            world.run_system_once(battle_unspawned).unwrap(),
            "the next encounter re-opens the gate once the player is gone"
        );
    }
}
