//! Start-up main menu: the New Game / Options / Credits screen shown before any
//! battle, with a yellow `>` cursor and keyboard navigation.
//!
//! Deliberately mirrors the look and feel of the battle [`ActionMenu`](crate::battle::menu)
//! — a vertical list of rows, each owning its own cursor child that we
//! visibility-toggle on the highlighted row — so the two menus feel like one
//! game. The wrap-around navigation reuses the battle menu's pure
//! [`cycle_index`](crate::battle::menu::cycle_index) helper rather than
//! duplicating it; only the row set (three options) and the confirm dispatch
//! differ.
//!
//! New Game resets [`PlayerProgress`] and drops the player onto the overworld
//! [`Map`](GameState::Map), where a random encounter starts the first battle.
//! Options and Credits are intentionally non-functional for now: confirming
//! either logs a "not yet implemented" line and leaves the player on the menu.

use bevy::prelude::*;

use crate::battle::menu::{CycleDirection, cycle_index};
use crate::progress::PlayerProgress;
use crate::state::GameState;

/// Yellow used for the cursor and the highlighted row label (matches the battle
/// action menu).
const HIGHLIGHT_COLOR: Color = Color::srgb(1.0, 1.0, 0.0);
/// White used for a row label that is not highlighted.
const DEFAULT_COLOR: Color = Color::WHITE;
/// The cursor glyph drawn to the left of the highlighted row.
const CURSOR_TEXT: &str = ">";
/// Font size of the game title above the options.
const TITLE_FONT_SIZE: f32 = 64.0;
/// Font size of each selectable option row.
const OPTION_FONT_SIZE: f32 = 32.0;

/// The three main-menu options, in display order. Index parity with the row
/// layout and with [`MenuOption::ALL`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuOption {
    NewGame,
    Options,
    Credits,
}

impl MenuOption {
    /// Every option in menu order — the single source of truth for both the row
    /// layout and the count used by [`cycle_index`].
    pub const ALL: [MenuOption; 3] = [
        MenuOption::NewGame,
        MenuOption::Options,
        MenuOption::Credits,
    ];

    /// The label shown on this option's row.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            MenuOption::NewGame => "New Game",
            MenuOption::Options => "Options",
            MenuOption::Credits => "Credits",
        }
    }
}

/// Which row the cursor sits on. Mirrors the battle menu's `MenuSelection`;
/// seeded to `Some(0)` when the menu is spawned so the cursor shows immediately.
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct MainMenuSelection {
    pub highlighted: Option<usize>,
}

/// Root container of the whole menu UI, despawned wholesale on leaving the menu.
/// `Default + Clone` lets the `bsn!` macro treat the marker as a `Template`.
#[derive(Component, Debug, Default, Clone)]
pub struct MainMenuRoot;

/// The yellow `>` cursor child of a row; visible only on the highlighted row.
#[derive(Component, Debug, Clone, Copy, FromTemplate)]
pub struct MainMenuCursor(pub usize);

/// The option-name `Text` of a row, recoloured on highlight.
#[derive(Component, Debug, Clone, Copy, FromTemplate)]
pub struct MainMenuLabel(pub usize);

/// Wires the start-up menu: spawns it on entering [`GameState::MainMenu`],
/// despawns it on leaving, and runs keyboard navigation + the highlight redraw
/// while it is up.
pub(crate) fn plugin(app: &mut App) {
    app.init_resource::<MainMenuSelection>()
        .add_systems(OnEnter(GameState::MainMenu), spawn_main_menu)
        .add_systems(OnExit(GameState::MainMenu), despawn_main_menu)
        .add_systems(
            Update,
            (main_menu_input, update_main_menu_highlight).run_if(in_state(GameState::MainMenu)),
        );
}

/// `OnEnter(MainMenu)`: highlight the first option and build the menu UI tree —
/// a centred column with the game title above the three option rows, each a
/// hidden yellow `>` cursor beside its label.
pub fn spawn_main_menu(mut commands: Commands, mut selection: ResMut<MainMenuSelection>) {
    selection.highlighted = Some(0);

    // A centred column: the game title above the three option rows. The rows are
    // index-parametrized, so they are built as a `Vec<impl Scene>` (a `SceneList`)
    // and spliced into the root's `Children` after the title with `{rows}`.
    let rows: Vec<_> = MenuOption::ALL
        .iter()
        .enumerate()
        .map(|(index, option)| menu_row(index, option.label()))
        .collect();
    commands.spawn_scene(bsn! {
        MainMenuRoot
        Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            row_gap: Val::Px(12.0),
        }
        Children [
            (
                Text("Bevy 2D RPG")
                TextFont { font_size: {FontSize::Px(TITLE_FONT_SIZE)} }
                TextColor({DEFAULT_COLOR})
                Node { margin: {UiRect::bottom(Val::Px(32.0))} }
            ),
            {rows}
        ]
    });
}

/// One main-menu row scene: a flex row holding the (initially hidden) yellow `>`
/// cursor and the option label, both tagged with `index` for
/// [`update_main_menu_highlight`]. Returns an `impl Scene` so the rows can be
/// collected into a `SceneList` and spliced into the root column.
fn menu_row(index: usize, label: &str) -> impl Scene {
    let label = label.to_string();
    bsn! {
        Node {
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(8.0),
        }
        Children [
            (
                MainMenuCursor({index})
                Text({CURSOR_TEXT})
                TextFont { font_size: {FontSize::Px(OPTION_FONT_SIZE)} }
                TextColor({HIGHLIGHT_COLOR})
                // Hidden until `update_main_menu_highlight` reveals the cursor on
                // the highlighted row.
                Visibility::Hidden
            ),
            (
                MainMenuLabel({index})
                Text({label})
                TextFont { font_size: {FontSize::Px(OPTION_FONT_SIZE)} }
                TextColor({DEFAULT_COLOR})
            )
        ]
    }
}

/// `OnExit(MainMenu)`: tear the whole menu down. One despawn of the tagged root
/// takes its children (title + rows) with it.
pub fn despawn_main_menu(mut commands: Commands, roots: Query<Entity, With<MainMenuRoot>>) {
    for root in &roots {
        commands.entity(root).despawn();
    }
}

/// Keyboard navigation, gated to the main menu. Up/Down cycle the highlight with
/// wrap; Enter confirms the highlighted option.
pub fn main_menu_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut selection: ResMut<MainMenuSelection>,
    mut next_state: ResMut<NextState<GameState>>,
    mut progress: ResMut<PlayerProgress>,
) {
    let count = MenuOption::ALL.len();

    let pressed = [KeyCode::ArrowDown, KeyCode::ArrowUp, KeyCode::Enter]
        .into_iter()
        .find(|&key| keys.just_pressed(key));

    match pressed {
        Some(KeyCode::ArrowDown) => {
            selection.highlighted = Some(cycle_index(
                selection.highlighted,
                CycleDirection::Down,
                count,
            ));
        }
        Some(KeyCode::ArrowUp) => {
            selection.highlighted = Some(cycle_index(
                selection.highlighted,
                CycleDirection::Up,
                count,
            ));
        }
        Some(KeyCode::Enter) => {
            if let Some(index) = selection.highlighted {
                confirm_option(MenuOption::ALL[index], &mut next_state, &mut progress);
            }
        }
        _ => {}
    }
}

/// Run the chosen option. Split out so headless tests can invoke it directly.
///
/// New Game resets the carried-over health and drops the player onto the map by
/// switching to [`GameState::Map`]; Options and Credits are not implemented yet
/// and merely log, leaving the menu in place.
fn confirm_option(
    option: MenuOption,
    next_state: &mut NextState<GameState>,
    progress: &mut PlayerProgress,
) {
    match option {
        MenuOption::NewGame => {
            progress.reset();
            next_state.set(GameState::Map);
        }
        MenuOption::Options => info!("Options menu is not yet implemented."),
        MenuOption::Credits => info!("Credits screen is not yet implemented."),
    }
}

/// Redraw the menu to match [`MainMenuSelection`]: show the cursor on exactly the
/// highlighted row (hidden elsewhere) and recolour labels yellow/white.
pub fn update_main_menu_highlight(
    selection: Res<MainMenuSelection>,
    mut cursors: Query<(&MainMenuCursor, &mut Visibility)>,
    mut labels: Query<(&MainMenuLabel, &mut TextColor)>,
) {
    for (MainMenuCursor(index), mut visibility) in &mut cursors {
        *visibility = if selection.highlighted == Some(*index) {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
    for (MainMenuLabel(index), mut color) in &mut labels {
        color.0 = if selection.highlighted == Some(*index) {
            HIGHLIGHT_COLOR
        } else {
            DEFAULT_COLOR
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The three options render the expected labels in order.
    #[test]
    fn options_have_expected_labels() {
        let labels: Vec<&str> = MenuOption::ALL.iter().map(|o| o.label()).collect();
        assert_eq!(labels, ["New Game", "Options", "Credits"]);
    }
}
