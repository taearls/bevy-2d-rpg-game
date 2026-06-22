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

use crate::battle::menu::{ActionMenuPanel, LogView};
use crate::battle::messages::LogMessage;
use crate::battle::state::TurnPhase;
use crate::state::GameState;

use super::{UiConfig, log_showing};

/// Minimum wall-clock time a freshly written log line is kept on screen before
/// [`swap_panel_for_phase`] is allowed to hide the panel. Without this, a line
/// written right around a `PlayerTurn` transition (e.g. the player's own attack,
/// or a one-attacker enemy turn that resolves in a couple of frames) would flash
/// for only a few frames before the panel hides. Real time, so it's independent
/// of game pacing.
const LOG_VISIBLE_HOLD: f32 = 1.5;

/// Tracks how long ago the most recent log line was written, in [`Time<Real>`]
/// seconds-since-startup. [`swap_panel_for_phase`] keeps the log shown while the
/// elapsed time since this stamp is under [`LOG_VISIBLE_HOLD`].
#[derive(Resource, Debug, Default)]
pub struct LogHold {
    /// `Time::<Real>::elapsed_secs()` at the last line write; `None` until the
    /// first line of the current battle is logged.
    last_write: Option<f32>,
}

/// Colour of a battle-log line.
const LOG_TEXT_COLOR: Color = Color::srgb(0.9, 0.9, 0.9);
/// Dimmer colour for the close hint, so it reads as a footnote, not a log line.
const LOG_HINT_COLOR: Color = Color::srgb(0.55, 0.55, 0.6);
/// Smaller font for the close hint than the log lines, reinforcing the footnote
/// look (log lines use the default ~20 px body size).
const LOG_HINT_FONT_SIZE: f32 = 12.0;
/// The close-hint text shown only when the player opened the log via the menu.
const LOG_HINT_TEXT: &str = "Esc/Enter to close";

/// Background of the log panel — shares the Godot `action_menu_panel` style with
/// the action menu it swaps in for (`bg_color = (0.12, 0.12, 0.16, 1)`).
const LOG_PANEL_BG_COLOR: Color = Color::srgb(0.12, 0.12, 0.16);
/// White 2px border matching the action-menu panel.
const LOG_PANEL_BORDER_COLOR: Color = Color::WHITE;
/// How far above the bottom of the screen the log panel sits — matches the
/// action-menu panel's [`PANEL_BOTTOM_OFFSET`](crate::battle::menu) (0 px) so the
/// two occupy the exact same slot (overlapping the info pane, drawn in front)
/// when swapped.
const LOG_PANEL_BOTTOM_OFFSET: f32 = 0.0;

/// The container that holds the battle-log lines (the Godot
/// `_battleMessageContainer`). Spawned hidden alongside the action menu; shown
/// while the log is active.
///
/// `Default + Clone` so the `bsn!` macro can treat it as a `Template` (markers
/// auto-derive `FromTemplate` from those two).
#[derive(Component, Debug, Default, Clone)]
pub struct BattleLogContainer;

/// The full-width wrapper that centres the log box. Carries the `Visibility`
/// that [`swap_panel_for_phase`] toggles; because Bevy visibility inherits, the
/// styled [`BattleLogContainer`] child shows/hides along with it.
///
/// `Default + Clone` so the `bsn!` macro can treat it as a `Template`.
#[derive(Component, Debug, Default, Clone)]
pub struct BattleLogPanel;

/// The "Esc/Enter to close" footnote on the log box. Shown only when the player
/// opened the log themselves from the menu's `Log` action — not during the
/// enemy-turn auto-show, where there is nothing to close. Its visibility is
/// driven by [`toggle_log_hint`] from [`LogView::open`].
///
/// `Default + Clone` so the `bsn!` macro can treat it as a `Template`.
#[derive(Component, Debug, Default, Clone)]
pub struct LogHint;

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
    // A full-width, bottom-anchored wrapper that horizontally centres the log
    // box, mirroring the action-menu panel it swaps in for so the two occupy the
    // same on-screen slot. Authored as a `bsn!` scene: the wrapper carries the
    // marker + visibility, and the styled `BattleLogContainer` is its sole child.
    //
    // `template_value(...)` wraps components built via a constructor (no plain
    // struct/tuple form the macro can parse) — here the `BorderColor::all` helper.
    commands.spawn_scene(bsn! {
        BattleLogPanel
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(LOG_PANEL_BOTTOM_OFFSET),
            left: Val::Px(0.0),
            width: Val::Percent(100.0),
            // Centre the box horizontally; stack the log box over its close hint.
            flex_direction: FlexDirection::Column,
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            row_gap: Val::Px(4.0),
        }
        // Draw the log box in front of the info pane it overlaps.
        ZIndex(1)
        // Hidden until the enemy turn / battle end shows the log.
        Visibility::Hidden
        template_value(DespawnOnExit(GameState::InBattle))
        Children [
            (
                BattleLogContainer
                Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(2.0),
                    padding: {UiRect::axes(Val::Px(16.0), Val::Px(12.0))},
                    border: {UiRect::all(Val::Px(2.0))},
                    border_radius: {BorderRadius::all(Val::Px(4.0))},
                }
                BackgroundColor({LOG_PANEL_BG_COLOR})
                template_value(BorderColor::all(LOG_PANEL_BORDER_COLOR))
            ),
            // The close hint sits just below the box, hidden until `toggle_log_hint`
            // reveals it when the player opened the log from the menu. As a sibling
            // of the container (not a child) it is untouched by
            // `clear_log_on_player_action`, which only despawns the container's lines.
            (
                LogHint
                Text({LOG_HINT_TEXT})
                TextFont { font_size: {FontSize::Px(LOG_HINT_FONT_SIZE)}, style: {FontStyle::Italic} }
                TextColor({LOG_HINT_COLOR})
                Visibility::Hidden
            )
        ]
    });
}

/// `BattleSet::Ui`: drain pending [`LogMessage`]s into `Text` children of the log
/// container, so each logged line appears as one row. Mirrors Godot `LogMessage`
/// instantiating a label per message into `_battleMessageContainer`.
pub fn render_log_panel(
    mut commands: Commands,
    mut messages: MessageReader<LogMessage>,
    time: Res<Time<Real>>,
    mut hold: ResMut<LogHold>,
    container: Query<Entity, With<BattleLogContainer>>,
) {
    let Ok(container) = container.single() else {
        // Drain anyway so messages don't pile up if the panel is missing.
        messages.clear();
        return;
    };
    if messages.is_empty() {
        return;
    }
    // Refresh the hold stamp so the panel stays visible at least LOG_VISIBLE_HOLD
    // after this batch, even if the phase flips back to PlayerTurn next frame.
    hold.last_write = Some(time.elapsed_secs());
    commands.entity(container).with_children(|panel| {
        for LogMessage(text) in messages.read() {
            panel.spawn((Text::new(format_log_line(text)), TextColor(LOG_TEXT_COLOR)));
        }
    });
}

/// `BattleSet::Ui`: set the centre-panel width and swap the menu / log visibility.
///
/// The log fills the centre panel either automatically — during the enemy turn /
/// battle-over, where the player cannot act ([`log_showing`]) — or on demand,
/// when the player opens it from the menu's `Log` action ([`LogView::open`]). In
/// both cases the panel widens to the battle-log width and the action menu hides;
/// otherwise it narrows to the action-menu width and the menu shows. Reading
/// [`UiConfig`] every frame means a live inspector width edit still takes effect
/// immediately. Mirrors Godot `ApplyCurrentPanelWidth` keyed off `_actionMenuActive`.
pub fn swap_panel_for_phase(
    state: Res<State<TurnPhase>>,
    config: Res<UiConfig>,
    log_view: Res<LogView>,
    time: Res<Time<Real>>,
    hold: Res<LogHold>,
    mut panel: Query<&mut Node, With<ActionMenuPanel>>,
    mut menu_visibility: Query<
        &mut Visibility,
        (With<ActionMenuPanel>, Without<BattleLogContainer>),
    >,
    mut log_visibility: Query<&mut Visibility, (With<BattleLogPanel>, Without<ActionMenuPanel>)>,
) {
    // Keep the log shown for at least LOG_VISIBLE_HOLD after the last line, so
    // freshly written messages don't flash and vanish on a quick phase flip.
    let within_hold = hold
        .last_write
        .is_some_and(|t| time.elapsed_secs() - t < LOG_VISIBLE_HOLD);

    // Show the log when the phase forces it (enemy turn / over), the player has
    // opened it from the menu, or a recent line is still inside its hold window.
    let showing = log_showing(*state.get()) || log_view.open || within_hold;

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

/// `BattleSet::Ui`: show the "Esc/Enter to close" hint only when the player opened
/// the log from the menu ([`LogView::open`]).
///
/// The hint is hidden during the enemy-turn / battle-over auto-show, where the
/// log is informational and there is nothing for the player to close. It sets the
/// hint to [`Visibility::Inherited`] (not `Visible`) when shown, so it still
/// disappears with the panel when [`swap_panel_for_phase`] hides the log.
pub fn toggle_log_hint(log_view: Res<LogView>, mut hint: Query<&mut Visibility, With<LogHint>>) {
    if let Ok(mut visibility) = hint.single_mut() {
        *visibility = if log_view.open {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
}

/// `OnExit(PlayerTurn)`: clear the battle-log lines as the player commits an
/// action, so the previous turn's lines survive the whole player turn and can be
/// reviewed via the menu's `Log` option before being wiped.
///
/// Clearing here — rather than `OnEnter(PlayerTurn)` — is what gives the `Log`
/// overlay something to show: the enemy turn's lines (and the player's own last
/// attack) persist until the player picks Fight/Items/Defend/Flee, all of which
/// leave `PlayerTurn`. Opening the `Log` overlay does **not** leave `PlayerTurn`,
/// so it never triggers this. Despawns the container's children (the Godot
/// `ClearMessages` → `ClearAndFreeChildren`).
pub fn clear_log_on_player_action(
    mut commands: Commands,
    mut hold: ResMut<LogHold>,
    container: Query<&Children, With<BattleLogContainer>>,
) {
    // Drop the visibility hold along with the lines, so the panel collapses back
    // to the action menu immediately instead of lingering empty. Any new line
    // written this turn (e.g. the player's own attack) re-stamps the hold.
    hold.last_write = None;
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
