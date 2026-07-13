//! The on-screen battle log and the menu↔log panel swap.
//!
//! Log lines arrive as [`LogMessage`]s (the same stream the stdout logger
//! drains); each is spawned as a `Text` child of [`BattleLogContainer`]. The
//! centre panel widens from the action-menu width to the wider battle-log width
//! while the log is showing — during the enemy turn and after the battle ends —
//! and the menu / log swap visibility accordingly. `OnEnter(PlayerTurn)` clears
//! the log and restores the menu.

use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
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
/// elapsed time since this stamp is under `LOG_VISIBLE_HOLD`.
#[derive(Resource, Debug, Default)]
pub struct LogHold {
    /// `Time::<Real>::elapsed_secs()` at the last line write; `None` until the
    /// first line of the current battle is logged.
    last_write: Option<f32>,
}

/// The full battle log — every [`LogMessage`] of the current battle, in order.
///
/// Distinct from the [`BattleLogContainer`] recent-lines view, which is cleared
/// each time the player commits an action: this accumulates the whole fight and
/// is wiped only at battle start ([`OnEnter(InBattle)`](GameState::InBattle)). It
/// backs the scrollable history shown by the menu's `Log` command.
#[derive(Resource, Debug, Default)]
pub struct BattleHistory {
    lines: Vec<String>,
}

impl BattleHistory {
    /// All recorded lines, oldest first.
    #[must_use]
    pub fn lines(&self) -> &[String] {
        &self.lines
    }
}

/// Maximum height of the scrollable history viewport (the menu `Log` view). The
/// box grows with its content up to this; taller content clips and scrolls.
const HISTORY_VIEWPORT_HEIGHT: f32 = 240.0;
/// Logical-pixel step for a keyboard line scroll (Up/Down) and per mouse-wheel
/// "line" notch. Roughly the body line height; the exact value only sets scroll
/// granularity — the *bounds* come from measured layout, not this.
const HISTORY_LINE_STEP: f32 = 22.0;

/// Pending scroll intent for the open history view, coordinating the
/// `Ui`-set rebuild with the `Input`-set scroller (which is the one with the
/// measured [`ComputedNode`] needed to clamp to the true content height).
#[derive(Resource, Debug, Default)]
pub struct HistoryScroll {
    /// Set when the history is (re)built; the scroller snaps to the bottom once
    /// the new content has been laid out, so the newest line is fully visible.
    snap_to_bottom: bool,
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

/// Background of the log panel — shares the panel style with the action menu it
/// swaps in for (`bg_color = (0.12, 0.12, 0.16, 1)`).
const LOG_PANEL_BG_COLOR: Color = Color::srgb(0.12, 0.12, 0.16);
/// White 2px border matching the action-menu panel.
const LOG_PANEL_BORDER_COLOR: Color = Color::WHITE;
/// How far above the bottom of the screen the log panel sits — matches the
/// action-menu panel's [`PANEL_BOTTOM_OFFSET`](crate::battle::menu) (0 px) so the
/// two occupy the exact same slot (overlapping the info pane, drawn in front)
/// when swapped.
const LOG_PANEL_BOTTOM_OFFSET: f32 = 0.0;

/// The container that holds the battle-log lines. Spawned hidden alongside the
/// action menu; shown while the log is active.
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

/// The fixed-height, `Overflow::scroll_y` viewport that clips the scrollable
/// history. Carries the [`ScrollPosition`] the scroll input mutates. Shown only
/// while the menu `Log` view is open; the recent-lines [`BattleLogContainer`]
/// takes the slot otherwise.
///
/// `Default + Clone` so the `bsn!` macro treats the marker as a `Template`.
#[derive(Component, Debug, Default, Clone)]
pub struct HistoryViewport;

/// The inner column holding one `Text` child per history line, scrolled within
/// [`HistoryViewport`]. Rebuilt from [`BattleHistory`] when the `Log` view opens.
///
/// `Default + Clone` so the `bsn!` macro treats the marker as a `Template`.
#[derive(Component, Debug, Default, Clone)]
pub struct HistoryContainer;

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
            // The scrollable full-history viewport, shown only while the player
            // opened the log from the menu (`LogView::open`). Fixed height +
            // `Overflow::scroll_y` clips and scrolls its inner `HistoryContainer`
            // column, whose lines are rebuilt from `BattleHistory` on open. Sits
            // in the same slot as the recent-lines box; `swap_history_view`
            // toggles which of the two is visible.
            (
                HistoryViewport
                Node {
                    // `display: None` keeps the viewport out of layout while
                    // closed, so its fixed height never shifts the recent-lines
                    // box. `manage_history_view` flips it to `Flex` when the `Log`
                    // view opens. (Plain `Visibility::Hidden` would still occupy
                    // layout space and push the recent box up.)
                    display: {Display::None},
                    flex_direction: FlexDirection::Column,
                    // `max_height` (not a fixed `height`): the box hugs its
                    // content while short, and only caps + scrolls once the log
                    // grows past this.
                    max_height: {Val::Px(HISTORY_VIEWPORT_HEIGHT)},
                    padding: {UiRect::axes(Val::Px(16.0), Val::Px(12.0))},
                    border: {UiRect::all(Val::Px(2.0))},
                    border_radius: {BorderRadius::all(Val::Px(4.0))},
                    overflow: {Overflow::scroll_y()},
                }
                BackgroundColor({LOG_PANEL_BG_COLOR})
                template_value(BorderColor::all(LOG_PANEL_BORDER_COLOR))
                Children [
                    (
                        HistoryContainer
                        Node {
                            flex_direction: FlexDirection::Column,
                            row_gap: Val::Px(2.0),
                        }
                    )
                ]
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
/// container, so each logged line appears as one row.
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

/// `BattleSet::Ui`: append every [`LogMessage`] to the persistent [`BattleHistory`]
/// so the menu `Log` view can show the whole fight. Has its own message-reader
/// cursor, independent of [`render_log_panel`], so both see every line. Mirrors
/// the recent-lines format so the two views read identically.
pub fn record_history(mut messages: MessageReader<LogMessage>, mut history: ResMut<BattleHistory>) {
    for LogMessage(text) in messages.read() {
        history.lines.push(format_log_line(text));
    }
}

/// `OnEnter(InBattle)`: start each battle with an empty history so the `Log` view
/// never shows a previous fight's lines.
pub fn reset_history(mut history: ResMut<BattleHistory>) {
    history.lines.clear();
}

/// `BattleSet::Ui`: swap the recent-lines box for the scrollable full history
/// while the player has the `Log` view open, and keep the history rebuilt from
/// [`BattleHistory`].
///
/// On the frame the view opens (or whenever the line count changes while open) it
/// repopulates [`HistoryContainer`] from the resource and snaps the scroll to the
/// newest line. While closed it hides the viewport so the auto-show recent-lines
/// box (driven by [`swap_panel_for_phase`]) owns the slot again.
#[allow(clippy::too_many_arguments)]
pub fn manage_history_view(
    mut commands: Commands,
    log_view: Res<LogView>,
    history: Res<BattleHistory>,
    mut scroll_state: ResMut<HistoryScroll>,
    mut last_shown: Local<usize>,
    mut viewport: Query<&mut Node, (With<HistoryViewport>, Without<BattleLogContainer>)>,
    mut recent: Query<&mut Node, (With<BattleLogContainer>, Without<HistoryViewport>)>,
    container: Query<Entity, With<HistoryContainer>>,
) {
    let Ok(mut viewport_node) = viewport.single_mut() else {
        return;
    };
    let Ok(mut recent_node) = recent.single_mut() else {
        return;
    };

    // Toggle with `display`, not `visibility`: a hidden-but-laid-out viewport
    // would still occupy its fixed height and shift the recent-lines box.
    if !log_view.open {
        viewport_node.display = Display::None;
        recent_node.display = Display::Flex;
        *last_shown = 0;
        return;
    }

    // Open: the history takes the slot; drop the recent-lines box from layout.
    viewport_node.display = Display::Flex;
    recent_node.display = Display::None;

    // Rebuild the lines only when the count changed (open transition, or a new
    // line arrived while open), then request a snap to the bottom. The actual
    // snap happens in `scroll_history`, which sees the laid-out `ComputedNode`
    // and so can land on the true content height (this frame's rebuild isn't
    // measured until the layout system runs).
    if *last_shown != history.lines.len() {
        *last_shown = history.lines.len();
        if let Ok(entity) = container.single() {
            commands.entity(entity).despawn_related::<Children>();
            commands.entity(entity).with_children(|col| {
                for line in &history.lines {
                    col.spawn((Text::new(line.clone()), TextColor(LOG_TEXT_COLOR)));
                }
            });
        }
        scroll_state.snap_to_bottom = true;
    }
}

/// `BattleSet::Input`, gated to [`PlayerTurn`](TurnPhase::PlayerTurn): scroll the
/// open `Log` history with the keyboard (Up/Down a line, PageUp/PageDown a page)
/// and the mouse wheel, and honour a pending snap-to-bottom request from
/// [`manage_history_view`]. No-op unless the view is open.
///
/// The scroll bound is read from the viewport's measured [`ComputedNode`]
/// (content height minus the inner area inside padding/border), not estimated
/// from a line count — so scrolling all the way down lands with the newest line
/// *fully* visible, regardless of font size or padding. `ScrollPosition` is in
/// logical pixels, while `ComputedNode` is physical, so the bound is converted
/// via `inverse_scale_factor`.
pub fn scroll_history(
    log_view: Res<LogView>,
    keys: Res<ButtonInput<KeyCode>>,
    mut wheel: MessageReader<MouseWheel>,
    mut scroll_state: ResMut<HistoryScroll>,
    mut viewport: Query<(&mut ScrollPosition, &ComputedNode), With<HistoryViewport>>,
) {
    if !log_view.open {
        wheel.clear();
        scroll_state.snap_to_bottom = false;
        return;
    }
    let Ok((mut scroll, computed)) = viewport.single_mut() else {
        return;
    };

    // Measured max scroll, in logical px: how far the content overflows the inner
    // (content-box) height. `content_size` already excludes padding/border; the
    // visible content height is `size - vertical padding - vertical border`.
    // `BorderRect` stores the top inset in `min_inset.y` and the bottom in
    // `max_inset.y`; sum both edges for padding and border.
    let inset = computed.padding.min_inset.y
        + computed.padding.max_inset.y
        + computed.border.min_inset.y
        + computed.border.max_inset.y;
    let overflow_physical = (computed.content_size.y - (computed.size.y - inset)).max(0.0);
    let max = overflow_physical * computed.inverse_scale_factor;

    // A pending snap (just (re)built) wins over input this frame: jump to bottom
    // now that the new content has been measured.
    if scroll_state.snap_to_bottom {
        scroll_state.snap_to_bottom = false;
        scroll.0.y = max;
        wheel.clear();
        return;
    }

    let page = (HISTORY_VIEWPORT_HEIGHT - HISTORY_LINE_STEP).max(HISTORY_LINE_STEP);
    let mut delta = 0.0;
    if keys.just_pressed(KeyCode::ArrowDown) {
        delta += HISTORY_LINE_STEP;
    }
    if keys.just_pressed(KeyCode::ArrowUp) {
        delta -= HISTORY_LINE_STEP;
    }
    if keys.just_pressed(KeyCode::PageDown) {
        delta += page;
    }
    if keys.just_pressed(KeyCode::PageUp) {
        delta -= page;
    }
    for ev in wheel.read() {
        // `MouseScrollUnit::Line` reports notches; `Pixel` reports raw pixels.
        // Up-scroll (positive `y`) should move the view toward older lines.
        delta -= match ev.unit {
            MouseScrollUnit::Line => ev.y * HISTORY_LINE_STEP,
            MouseScrollUnit::Pixel => ev.y,
        };
    }

    if delta != 0.0 {
        scroll.0.y = (scroll.0.y + delta).clamp(0.0, max);
    }
}

/// `BattleSet::Ui`: set the centre-panel width and swap the menu / log visibility.
///
/// The log fills the centre panel either automatically — during the enemy turn /
/// battle-over, where the player cannot act ([`log_showing`]) — or on demand,
/// when the player opens it from the menu's `Log` action ([`LogView::open`]). In
/// both cases the panel widens to the battle-log width and the action menu hides;
/// otherwise it narrows to the action-menu width and the menu shows. Reading
/// [`UiConfig`] every frame means a live inspector width edit still takes effect
/// immediately.
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
/// so it never triggers this. Despawns the container's children.
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
