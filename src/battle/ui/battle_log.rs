//! The on-screen battle log and the menu↔log panel swap.
//!
//! Bevy port of the Godot `BattleUI` log half. Log lines arrive as
//! [`LogMessage`]s (the same stream the stdout logger drains); each is spawned as
//! a `Text` child of [`BattleLogContainer`]. The centre panel widens from the
//! action-menu width to the wider battle-log width while the log is showing —
//! during the enemy turn and after the battle ends — and the menu / log swap
//! visibility accordingly. `OnEnter(PlayerTurn)` clears the log and restores the
//! menu, mirroring the Godot `ShowActionMenu` teardown.

use bevy::prelude::*;

use crate::battle::menu::ActionMenuPanel;
use crate::battle::messages::LogMessage;
use crate::battle::state::TurnPhase;

use super::{UiConfig, log_showing};

/// Colour of a battle-log line.
const LOG_TEXT_COLOR: Color = Color::srgb(0.9, 0.9, 0.9);

/// The container that holds the battle-log lines (the Godot
/// `_battleMessageContainer`). Spawned hidden alongside the action menu; shown
/// while the log is active.
#[derive(Component, Debug)]
pub struct BattleLogContainer;

/// Format a log line for display. A standalone helper so the (currently
/// timestamp-free) format has a single home and the log test can assert it
/// without reaching into the spawn system.
#[must_use]
pub fn format_log_line(text: &str) -> String {
    text.to_string()
}

/// Spawn the battle-log container as a hidden sibling of the action menu, sharing
/// the same bottom-left anchor so it occupies the centre panel slot when shown.
///
/// Kept separate from the menu spawn so the log owns its own marker and
/// visibility. Starts [`Visibility::Hidden`]; [`swap_panel_for_phase`] reveals it
/// during the enemy turn / battle-over phases.
pub fn spawn_battle_log(mut commands: Commands) {
    commands.spawn((
        BattleLogContainer,
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(24.0),
            left: Val::Px(24.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(2.0),
            ..default()
        },
        // Hidden until the enemy turn / battle end shows the log.
        Visibility::Hidden,
    ));
}

/// `BattleSet::Ui`: drain pending [`LogMessage`]s into `Text` children of the log
/// container, so each logged line appears as one row. Mirrors Godot `LogMessage`
/// instantiating a label per message into `_battleMessageContainer`.
pub fn render_log_panel(
    mut commands: Commands,
    mut messages: MessageReader<LogMessage>,
    container: Query<Entity, With<BattleLogContainer>>,
) {
    let Ok(container) = container.single() else {
        // Drain anyway so messages don't pile up if the panel is missing.
        messages.clear();
        return;
    };
    commands.entity(container).with_children(|panel| {
        for LogMessage(text) in messages.read() {
            panel.spawn((Text::new(format_log_line(text)), TextColor(LOG_TEXT_COLOR)));
        }
    });
}

/// `BattleSet::Ui`: set the centre-panel width and the menu/log visibility from
/// the current phase.
///
/// While the log shows (enemy turn / battle over) the panel widens to the
/// battle-log width and the menu hides; otherwise it narrows to the action-menu
/// width and the menu shows. Reading [`UiConfig`] every frame means a live
/// inspector edit to either half-width takes effect immediately — the Phase 8
/// parity case. Mirrors Godot `ApplyCurrentPanelWidth` keyed off
/// `_actionMenuActive`.
pub fn swap_panel_for_phase(
    state: Res<State<TurnPhase>>,
    config: Res<UiConfig>,
    mut panel: Query<&mut Node, With<ActionMenuPanel>>,
    mut menu_visibility: Query<
        &mut Visibility,
        (With<ActionMenuPanel>, Without<BattleLogContainer>),
    >,
    mut log_visibility: Query<
        &mut Visibility,
        (With<BattleLogContainer>, Without<ActionMenuPanel>),
    >,
) {
    let showing = log_showing(*state.get());

    if let Ok(mut node) = panel.single_mut() {
        node.width = Val::Px(config.panel_width(showing));
    }
    if let Ok(mut visibility) = menu_visibility.single_mut() {
        *visibility = if showing {
            Visibility::Hidden
        } else {
            Visibility::Visible
        };
    }
    if let Ok(mut visibility) = log_visibility.single_mut() {
        *visibility = if showing {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
}

/// `OnEnter(PlayerTurn)`: clear the battle-log lines so each player turn starts
/// with an empty log, restoring the action-menu view. Despawns the container's
/// children (the Godot `ClearMessages` → `ClearAndFreeChildren`); the
/// panel-width / visibility restore is handled by [`swap_panel_for_phase`] from
/// the new phase.
pub fn clear_log_on_player_turn(
    mut commands: Commands,
    container: Query<&Children, With<BattleLogContainer>>,
) {
    let Ok(children) = container.single() else {
        return;
    };
    for &child in children {
        commands.entity(child).despawn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The log line format is the raw message for now (timestamp reserved).
    #[test]
    fn log_line_is_raw_text() {
        assert_eq!(format_log_line("Hero attacks!"), "Hero attacks!");
    }
}
