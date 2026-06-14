//! Player action menu: the Fight / Items / Defend / Flee list, its yellow `>`
//! cursor, and the keyboard navigation that drives it during the player turn.
//!
//! Bevy port of the Godot `ActionMenu` + the menu half of `BattleScene`. The
//! Godot version reparented a single cursor `Label` between rows; here every row
//! owns its own cursor child and we visibility-toggle the one on the highlighted
//! row, which is despawn-safe and trivial to assert headlessly. The highlight
//! cycling logic ([`cycle_index`]) is a pure function so the `ActionMenuTest`
//! wrap-around cases can be mirrored without an ECS world.

use bevy::prelude::*;

use crate::characters::components::{Defending, DisplayName, Player};

use super::messages::LogMessage;
use super::state::TurnPhase;

/// Yellow used for the cursor and the highlighted row label.
const HIGHLIGHT_COLOR: Color = Color::srgb(1.0, 1.0, 0.0);
/// White used for a row label that is not highlighted.
const DEFAULT_COLOR: Color = Color::WHITE;
/// The cursor glyph drawn to the left of the highlighted row.
const CURSOR_TEXT: &str = ">";

/// Background of the action-menu / log panel — the Godot `action_menu_panel`
/// `StyleBoxFlat` (`bg_color = (0.12, 0.12, 0.16, 1)`).
const PANEL_BG_COLOR: Color = Color::srgb(0.12, 0.12, 0.16);
/// White 2px border of the action-menu panel (the Godot `border_color`).
const PANEL_BORDER_COLOR: Color = Color::WHITE;
/// How far above the bottom of the screen the centred action-menu panel sits.
/// Lower than the 160px info-pane height so the box's bottom edge overlaps the
/// pane; a positive [`ZIndex`] then draws it in front of the dark bar for a
/// layered look (rather than floating in the gap above it).
const PANEL_BOTTOM_OFFSET: f32 = 80.0;

/// The four menu actions, in display order. Index parity with the row layout and
/// with the Godot `SetActions(Fight, Items, Defend, Flee)` ordering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuAction {
    Fight,
    Items,
    Defend,
    Flee,
}

impl MenuAction {
    /// Every action in menu order — the single source of truth for both the row
    /// layout and the count used by [`cycle_index`].
    pub const ALL: [MenuAction; 4] = [
        MenuAction::Fight,
        MenuAction::Items,
        MenuAction::Defend,
        MenuAction::Flee,
    ];

    /// The label shown on this action's row.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            MenuAction::Fight => "Fight",
            MenuAction::Items => "Items",
            MenuAction::Defend => "Defend",
            MenuAction::Flee => "Flee",
        }
    }
}

/// Which row the cursor sits on, or `None` before the first highlight. Mirrors
/// Godot `ActionMenu._highlightedIndex` (with `-1` modelled as `None`).
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct MenuSelection {
    pub highlighted: Option<usize>,
}

/// Root container of the action menu (the Godot `ActionMenu` `VBoxContainer`).
#[derive(Component, Debug)]
pub struct ActionMenuPanel;

/// One selectable row, tagged with its action index.
#[derive(Component, Debug, Clone, Copy)]
pub struct MenuRow(pub usize);

/// The yellow `>` cursor child of a row; visible only on the highlighted row.
#[derive(Component, Debug, Clone, Copy)]
pub struct MenuCursor(pub usize);

/// The action-name `Text` child of a row, recoloured on highlight.
#[derive(Component, Debug, Clone, Copy)]
pub struct MenuLabel(pub usize);

/// Direction the highlight moves when cycling: [`Down`](Self::Down) advances to
/// the next row, [`Up`](Self::Up) the previous, both with wrap-around.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CycleDirection {
    Down,
    Up,
}

/// Step the highlighted index in `direction` with wrap-around.
///
/// Pure port of Godot `ActionMenu.CycleHighlight`: from no selection it lands on
/// row 0 regardless of direction; otherwise it wraps modulo `count`. Computed in
/// `usize` to stay sign-clean — an upward step adds `count - 1` (≡ `-1` mod
/// `count`) so row 0 wraps to the last row without a signed cast. `count` must
/// be non-zero (the menu always has four rows).
#[must_use]
pub fn cycle_index(current: Option<usize>, direction: CycleDirection, count: usize) -> usize {
    match current {
        None => 0,
        Some(index) => {
            let step = match direction {
                CycleDirection::Down => 1,
                CycleDirection::Up => count - 1,
            };
            (index + step) % count
        }
    }
}

/// `OnEnter(PlayerTurn)`: clear any leftover `Defending` marker and highlight
/// row 0, matching Godot `StartPlayerTurn` (`Highlight(0)`) and the Phase 4
/// "`Defending` removed `OnEnter(PlayerTurn)`" requirement.
pub fn on_enter_player_turn(
    mut commands: Commands,
    mut selection: ResMut<MenuSelection>,
    defenders: Query<Entity, With<Defending>>,
) {
    for entity in &defenders {
        commands.entity(entity).remove::<Defending>();
    }
    selection.highlighted = Some(0);
}

/// Keyboard navigation, gated to the player turn. Up/Down cycle the highlight
/// with wrap; Enter confirms the highlighted action. Mirrors the `PlayerTurn`
/// branch of Godot `BattleScene._UnhandledInput`.
pub fn menu_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut selection: ResMut<MenuSelection>,
    mut next_state: ResMut<NextState<TurnPhase>>,
    mut log: MessageWriter<LogMessage>,
    mut commands: Commands,
    player: Query<(Entity, &DisplayName), With<Player>>,
) {
    let count = MenuAction::ALL.len();
    if keys.just_pressed(KeyCode::ArrowDown) {
        selection.highlighted = Some(cycle_index(
            selection.highlighted,
            CycleDirection::Down,
            count,
        ));
    } else if keys.just_pressed(KeyCode::ArrowUp) {
        selection.highlighted = Some(cycle_index(
            selection.highlighted,
            CycleDirection::Up,
            count,
        ));
    } else if keys.just_pressed(KeyCode::Enter)
        && let Some(index) = selection.highlighted
    {
        confirm_action(
            MenuAction::ALL[index],
            &mut next_state,
            &mut log,
            &mut commands,
            &player,
        );
    }
}

/// Run the chosen action, dispatching to the right turn transition and log line.
/// Split out so headless tests can invoke it directly, mirroring
/// `ActionMenuTest.ConfirmSelection_InvokesCorrectHandler`.
fn confirm_action(
    action: MenuAction,
    next_state: &mut NextState<TurnPhase>,
    log: &mut MessageWriter<LogMessage>,
    commands: &mut Commands,
    player: &Query<(Entity, &DisplayName), With<Player>>,
) {
    // The player's name fronts every log line; fall back to "Player" if the
    // player entity is somehow absent so a missing query never panics.
    let (player_entity, name) = match player.iter().next() {
        Some((entity, DisplayName(name))) => (Some(entity), name.clone()),
        None => (None, "Player".to_string()),
    };

    match action {
        MenuAction::Fight => {
            next_state.set(TurnPhase::Targeting);
        }
        MenuAction::Items => {
            log.write(LogMessage::new(format!("{name} uses an item!")));
            next_state.set(TurnPhase::EnemyTurn);
        }
        MenuAction::Defend => {
            if let Some(entity) = player_entity {
                commands.entity(entity).insert(Defending);
            }
            log.write(LogMessage::new(format!("{name} is defending!")));
            next_state.set(TurnPhase::EnemyTurn);
        }
        MenuAction::Flee => {
            log.write(LogMessage::new(format!("{name} attempts to flee!")));
            next_state.set(TurnPhase::EnemyTurn);
        }
    }
}

/// Redraw the menu to match [`MenuSelection`]: show the cursor on exactly the
/// highlighted row (hidden elsewhere) and recolour labels yellow/white. Runs in
/// [`BattleSet::Ui`] every frame so the menu always reflects current state.
pub fn update_menu_highlight(
    selection: Res<MenuSelection>,
    mut cursors: Query<(&MenuCursor, &mut Visibility)>,
    mut labels: Query<(&MenuLabel, &mut TextColor)>,
) {
    for (MenuCursor(index), mut visibility) in &mut cursors {
        *visibility = if selection.highlighted == Some(*index) {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
    for (MenuLabel(index), mut color) in &mut labels {
        color.0 = if selection.highlighted == Some(*index) {
            HIGHLIGHT_COLOR
        } else {
            DEFAULT_COLOR
        };
    }
}

/// Spawn the action-menu UI: a bottom-anchored column of four rows, each a
/// hidden yellow `>` cursor beside its action label. Highlight state is applied
/// separately by [`update_menu_highlight`].
pub fn spawn_action_menu(mut commands: Commands) {
    // A full-width, bottom-anchored wrapper that horizontally centres the panel
    // box — the Bevy equivalent of the Godot `ActionMenuPanel` `anchor 0.5`
    // centring. The wrapper takes no space visually (no background); the
    // `ActionMenuPanel` child is the styled box that floats above the info pane.
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(PANEL_BOTTOM_OFFSET),
                left: Val::Px(0.0),
                width: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                ..default()
            },
            // Draw the menu box in front of the info pane it overlaps.
            ZIndex(1),
        ))
        .with_children(|wrapper| {
            wrapper
                .spawn((
                    ActionMenuPanel,
                    Node {
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(4.0),
                        // The Godot `action_menu_panel` content margins (16/12)
                        // and 2px border.
                        padding: UiRect::axes(Val::Px(16.0), Val::Px(12.0)),
                        border: UiRect::all(Val::Px(2.0)),
                        border_radius: BorderRadius::all(Val::Px(4.0)),
                        ..default()
                    },
                    BackgroundColor(PANEL_BG_COLOR),
                    BorderColor::all(PANEL_BORDER_COLOR),
                ))
                .with_children(|panel| {
                    for (index, action) in MenuAction::ALL.iter().enumerate() {
                        panel
                            .spawn((
                                MenuRow(index),
                                Node {
                                    flex_direction: FlexDirection::Row,
                                    column_gap: Val::Px(8.0),
                                    ..default()
                                },
                            ))
                            .with_children(|row| {
                                row.spawn((
                                    MenuCursor(index),
                                    Text::new(CURSOR_TEXT),
                                    TextColor(HIGHLIGHT_COLOR),
                                    // Hidden until `update_menu_highlight` reveals the
                                    // cursor on the highlighted row.
                                    Visibility::Hidden,
                                ));
                                row.spawn((
                                    MenuLabel(index),
                                    Text::new(action.label()),
                                    TextColor(DEFAULT_COLOR),
                                ));
                            });
                    }
                });
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Forward stepping advances by one and wraps past the last row to 0,
    /// mirroring `ActionMenuTest.CycleHighlight_WrapsForward`.
    #[test]
    fn cycle_forward_wraps() {
        assert_eq!(cycle_index(Some(0), CycleDirection::Down, 4), 1);
        assert_eq!(cycle_index(Some(3), CycleDirection::Down, 4), 0);
    }

    /// Backward stepping decrements and wraps from row 0 to the last row,
    /// mirroring `ActionMenuTest.CycleHighlight_WrapsBackward`.
    #[test]
    fn cycle_backward_wraps() {
        assert_eq!(cycle_index(Some(2), CycleDirection::Up, 4), 1);
        assert_eq!(cycle_index(Some(0), CycleDirection::Up, 4), 3);
    }

    /// From no selection, either direction lands on row 0
    /// (`ActionMenuTest.CycleHighlight_FromUnhighlighted_GoesToFirst`).
    #[test]
    fn cycle_from_unhighlighted_goes_to_zero() {
        assert_eq!(cycle_index(None, CycleDirection::Down, 4), 0);
        assert_eq!(cycle_index(None, CycleDirection::Up, 4), 0);
    }

    /// A single-row menu stays put under cycling
    /// (`ActionMenuTest.CycleHighlight_SingleItem_StaysOnSame`).
    #[test]
    fn cycle_single_item_stays() {
        assert_eq!(cycle_index(Some(0), CycleDirection::Down, 1), 0);
        assert_eq!(cycle_index(Some(0), CycleDirection::Up, 1), 0);
    }

    /// The four actions render the Godot labels in order.
    #[test]
    fn actions_have_expected_labels() {
        let labels: Vec<&str> = MenuAction::ALL.iter().map(|a| a.label()).collect();
        assert_eq!(labels, ["Fight", "Items", "Defend", "Flee"]);
    }
}
