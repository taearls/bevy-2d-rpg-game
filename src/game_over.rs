//! The game-over screen shown after the player is defeated in battle.
//!
//! Mirrors the look and navigation of the start-up [`main_menu`](crate::main_menu)
//! — a vertical list of rows, each owning a yellow `>` cursor child that is
//! visibility-toggled onto the highlighted row — and reuses the battle menu's
//! pure [`cycle_index`](crate::battle::menu::cycle_index) for wrap-around so all
//! three menus feel like one game.
//!
//! Two options: "Restart Game" resets [`PlayerProgress`] to full and drops the
//! player back onto the [`Map`](GameState::Map); "Return to Title Screen" goes
//! back to the [`MainMenu`](GameState::MainMenu). The whole UI is tagged
//! [`DespawnOnExit(GameOver)`](DespawnOnExit) so either choice tears it down.

use bevy::prelude::*;

use crate::battle::menu::{CycleDirection, cycle_index};
use crate::progress::PlayerProgress;
use crate::state::GameState;

/// Yellow used for the cursor and the highlighted row label.
const HIGHLIGHT_COLOR: Color = Color::srgb(1.0, 1.0, 0.0);
/// White used for a row label that is not highlighted.
const DEFAULT_COLOR: Color = Color::WHITE;
/// Red used for the "Game Over" title.
const TITLE_COLOR: Color = Color::srgb(0.85, 0.18, 0.18);
/// The cursor glyph drawn to the left of the highlighted row.
const CURSOR_TEXT: &str = ">";
/// Font size of the "Game Over" title.
const TITLE_FONT_SIZE: f32 = 64.0;
/// Font size of each selectable option row.
const OPTION_FONT_SIZE: f32 = 32.0;

/// The game-over options, in display order. Index parity with the row layout and
/// with [`GameOverOption::ALL`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameOverOption {
    RestartGame,
    ReturnToTitle,
}

impl GameOverOption {
    /// Every option in menu order — the single source of truth for both the row
    /// layout and the count used by [`cycle_index`].
    pub const ALL: [GameOverOption; 2] =
        [GameOverOption::RestartGame, GameOverOption::ReturnToTitle];

    /// The label shown on this option's row.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            GameOverOption::RestartGame => "Restart Game",
            GameOverOption::ReturnToTitle => "Return to Title Screen",
        }
    }
}

/// Which row the cursor sits on. Seeded to `Some(0)` when the screen is spawned
/// so the cursor shows immediately.
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct GameOverSelection {
    pub highlighted: Option<usize>,
}

/// Root container of the game-over UI, despawned wholesale on leaving the screen.
#[derive(Component, Debug)]
pub struct GameOverRoot;

/// The yellow `>` cursor child of a row; visible only on the highlighted row.
#[derive(Component, Debug, Clone, Copy)]
pub struct GameOverCursor(pub usize);

/// The option-name `Text` of a row, recoloured on highlight.
#[derive(Component, Debug, Clone, Copy)]
pub struct GameOverLabel(pub usize);

/// Wires the game-over screen: spawns it on entering [`GameState::GameOver`] and
/// runs keyboard navigation + the highlight redraw while it is up. The UI is
/// `DespawnOnExit(GameOver)`, so it tears down automatically on either choice.
pub(crate) fn plugin(app: &mut App) {
    app.init_resource::<GameOverSelection>()
        .add_systems(OnEnter(GameState::GameOver), spawn_game_over)
        .add_systems(
            Update,
            (game_over_input, update_game_over_highlight).run_if(in_state(GameState::GameOver)),
        );
}

/// `OnEnter(GameOver)`: highlight the first option and build the menu UI tree — a
/// centred column with the "Game Over" title above the two option rows, each a
/// hidden yellow `>` cursor beside its label.
pub fn spawn_game_over(mut commands: Commands, mut selection: ResMut<GameOverSelection>) {
    selection.highlighted = Some(0);

    commands
        .spawn((
            GameOverRoot,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: Val::Px(12.0),
                ..default()
            },
            DespawnOnExit(GameState::GameOver),
        ))
        .with_children(|root| {
            root.spawn((
                Text::new("Game Over"),
                TextFont {
                    font_size: FontSize::Px(TITLE_FONT_SIZE),
                    ..default()
                },
                TextColor(TITLE_COLOR),
                Node {
                    margin: UiRect::bottom(Val::Px(32.0)),
                    ..default()
                },
            ));

            for (index, option) in GameOverOption::ALL.iter().enumerate() {
                root.spawn(Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(8.0),
                    ..default()
                })
                .with_children(|row| {
                    row.spawn((
                        GameOverCursor(index),
                        Text::new(CURSOR_TEXT),
                        TextFont {
                            font_size: FontSize::Px(OPTION_FONT_SIZE),
                            ..default()
                        },
                        TextColor(HIGHLIGHT_COLOR),
                        // Hidden until `update_game_over_highlight` reveals the
                        // cursor on the highlighted row.
                        Visibility::Hidden,
                    ));
                    row.spawn((
                        GameOverLabel(index),
                        Text::new(option.label()),
                        TextFont {
                            font_size: FontSize::Px(OPTION_FONT_SIZE),
                            ..default()
                        },
                        TextColor(DEFAULT_COLOR),
                    ));
                });
            }
        });
}

/// Keyboard navigation, gated to the game-over screen. Up/Down cycle the
/// highlight with wrap; Enter confirms the highlighted option.
pub fn game_over_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut selection: ResMut<GameOverSelection>,
    mut next_state: ResMut<NextState<GameState>>,
    mut progress: ResMut<PlayerProgress>,
) {
    let count = GameOverOption::ALL.len();

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
                confirm_option(GameOverOption::ALL[index], &mut next_state, &mut progress);
            }
        }
        _ => {}
    }
}

/// Run the chosen option. Split out so headless tests can invoke it directly.
///
/// "Restart Game" resets the carried-over health and returns to the map at full
/// HP; "Return to Title Screen" goes back to the main menu (where a later New
/// Game also resets progress).
fn confirm_option(
    option: GameOverOption,
    next_state: &mut NextState<GameState>,
    progress: &mut PlayerProgress,
) {
    match option {
        GameOverOption::RestartGame => {
            progress.reset();
            next_state.set(GameState::Map);
        }
        GameOverOption::ReturnToTitle => next_state.set(GameState::MainMenu),
    }
}

/// Redraw the screen to match [`GameOverSelection`]: show the cursor on exactly
/// the highlighted row (hidden elsewhere) and recolour labels yellow/white.
pub fn update_game_over_highlight(
    selection: Res<GameOverSelection>,
    mut cursors: Query<(&GameOverCursor, &mut Visibility)>,
    mut labels: Query<(&GameOverLabel, &mut TextColor)>,
) {
    for (GameOverCursor(index), mut visibility) in &mut cursors {
        *visibility = if selection.highlighted == Some(*index) {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
    for (GameOverLabel(index), mut color) in &mut labels {
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

    /// The two options render the expected labels in order.
    #[test]
    fn options_have_expected_labels() {
        let labels: Vec<&str> = GameOverOption::ALL.iter().map(|o| o.label()).collect();
        assert_eq!(labels, ["Restart Game", "Return to Title Screen"]);
    }
}
