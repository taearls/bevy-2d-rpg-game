//! Battle HUD and on-screen log — the Bevy port of the Godot `BattleUI`.
//!
//! Two cooperating pieces, each in its own submodule:
//! - [`hud`] — the player name + HP fill, the dynamic alive-enemy name labels
//!   (with the targeting highlight), and the world-space enemy mini HP bars.
//! - [`battle_log`] — the log lines spawned one `Text` child per `LogMessage`,
//!   plus the menu↔log panel swap that widens the centre panel while the log
//!   shows.
//!
//! The Godot original drove these widgets off a `BattleEvents` signal bus with
//! manual subscribe/disconnect bookkeeping; here every widget is a plain system
//! reading ECS state — player/enemy HUD from `Changed<Health>`, the log from a
//! `MessageReader<LogMessage>`, the panel width from [`UiConfig`] and the current
//! [`TurnPhase`]. That deletes the entire bus-lifetime problem: a despawned
//! entity simply drops out of the next query.

pub mod battle_log;
pub mod hud;

use bevy::prelude::*;

use battle_log::{
    clear_log_on_player_turn, render_log_panel, spawn_battle_log, swap_panel_for_phase,
};
use hud::{
    refresh_enemy_labels, refresh_player_hud, spawn_hud, sync_enemy_health_bars,
    sync_enemy_label_text, update_enemy_label_highlight,
};

use super::state::{BattleSet, TurnPhase};
use crate::state::GameState;

/// Tunable panel half-widths, the Phase 8 inspector's parity for the Godot
/// `BattleUI` `[Export(Range)]` knobs (`ActionMenuHalfWidth` / `BattleLogHalfWidth`).
///
/// The centre panel's *total* width is twice the active half-width: the action
/// menu shows at `2 * action_menu_half_width` (200 px by default), and the wider
/// battle log at `2 * battle_log_half_width` (350 px). Stored as half-widths to
/// mirror the Godot `OffsetLeft = -half` / `OffsetRight = half` centring, so a
/// live edit in the inspector maps one-to-one onto the original's behaviour.
#[derive(Resource, Reflect, Debug, Clone, Copy, PartialEq)]
#[reflect(Resource)]
pub struct UiConfig {
    /// Half-width of the centre panel while the action menu is showing.
    pub action_menu_half_width: f32,
    /// Half-width of the centre panel while the battle log is showing.
    pub battle_log_half_width: f32,
}

impl Default for UiConfig {
    fn default() -> Self {
        // 100 → 200 px menu, 175 → 350 px log: the Godot `BattleUI` defaults.
        Self {
            action_menu_half_width: 100.0,
            battle_log_half_width: 175.0,
        }
    }
}

impl UiConfig {
    /// Full pixel width of the centre panel for the given mode.
    ///
    /// `log_showing` selects the wider battle-log width; otherwise the narrower
    /// action-menu width. Doubling the half-width reproduces the Godot
    /// `OffsetRight - OffsetLeft = 2 * half` span.
    #[must_use]
    pub fn panel_width(&self, log_showing: bool) -> f32 {
        2.0 * if log_showing {
            self.battle_log_half_width
        } else {
            self.action_menu_half_width
        }
    }
}

/// Whether the battle log (rather than the action menu) currently fills the
/// centre panel. The log shows during [`EnemyTurn`](TurnPhase::EnemyTurn) and
/// [`BattleOver`](TurnPhase::BattleOver) — the phases where the player cannot act
/// — and the menu shows otherwise. Mirrors the Godot `_actionMenuActive` flag
/// that `ApplyCurrentPanelWidth` keyed off.
#[must_use]
pub fn log_showing(phase: TurnPhase) -> bool {
    matches!(phase, TurnPhase::EnemyTurn | TurnPhase::BattleOver)
}

/// Wires the battle HUD and log: spawns the static UI tree when a battle starts
/// ([`OnEnter(InBattle)`](GameState::InBattle)) and runs the per-frame refreshers
/// in [`BattleSet::Ui`], so the HUD always reflects the world state the
/// resolve/cleanup phases just produced.
pub struct BattleUiPlugin;

impl Plugin for BattleUiPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<UiConfig>()
            .init_resource::<UiConfig>()
            // Spawn the HUD/log tree when a battle starts, not at startup, so it
            // never sits behind the main menu before "New Game" is chosen.
            .add_systems(OnEnter(GameState::InBattle), (spawn_hud, spawn_battle_log))
            .add_systems(OnEnter(TurnPhase::PlayerTurn), clear_log_on_player_turn)
            .add_systems(
                Update,
                (
                    refresh_player_hud,
                    refresh_enemy_labels,
                    sync_enemy_label_text,
                    update_enemy_label_highlight,
                    sync_enemy_health_bars,
                    render_log_panel,
                    swap_panel_for_phase,
                )
                    .in_set(BattleSet::Ui),
            );
    }
}
