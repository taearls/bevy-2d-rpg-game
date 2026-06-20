//! Headless HUD + battle-log coverage, the Bevy analogue of the Godot
//! `BattleUITest`.
//!
//! Every assertion is an ECS fact — `Text` contents, `Node.width`, a child
//! count, a `TextColor`, a `Transform.scale` — never a pixel. The real UI
//! systems are wired into a renderer-free `App` (`MinimalPlugins` + `StatesPlugin`):
//! `spawn_hud` / `spawn_battle_log` build the tree, the `BattleSet::Ui`
//! refreshers run each frame, and `clear_log_on_player_action` runs on leaving
//! `PlayerTurn`. As in the other UI suites, `InputPlugin` is omitted so a
//! manually-set state is not disturbed, and the combat resolver is included so
//! damage flows through `Changed<Health>` exactly as in play.

use bevy::prelude::*;
use bevy::scene::ScenePlugin;
use bevy::state::app::StatesPlugin;

use bevy_2d_rpg_game::battle::menu::{
    ActionMenuPanel, LogView, MenuCursor, MenuSelection, spawn_action_menu, update_menu_highlight,
};
use bevy_2d_rpg_game::battle::messages::LogMessage;
use bevy_2d_rpg_game::battle::state::{BattleSet, TurnPhase};
use bevy_2d_rpg_game::battle::ui::UiConfig;
use bevy_2d_rpg_game::battle::ui::battle_log::{
    BattleLogContainer, BattleLogPanel, LogHint, clear_log_on_player_action, render_log_panel,
    spawn_battle_log, swap_panel_for_phase, toggle_log_hint,
};
use bevy_2d_rpg_game::battle::ui::hud::{
    EnemyNameLabel, PlayerHpFill, PlayerNameLabel, refresh_enemy_labels, refresh_player_hud,
    spawn_hud, sync_enemy_health_bars, sync_enemy_label_text, update_enemy_label_highlight,
};
use bevy_2d_rpg_game::components::{DisplayName, Enemy, EnemyHealthBar, Health, Player, Targeted};

/// Yellow target highlight, matched against an `EnemyNameLabel`'s `TextColor`.
const HIGHLIGHT: Color = Color::srgb(1.0, 1.0, 0.0);
const WHITE: Color = Color::WHITE;

/// Build a headless app with the full HUD + log wiring and the menu panel, in
/// `PlayerTurn`. Spawns one player and `enemy_healths.len()` enemies (index
/// `0..n`) at the given starting HP, each with a mini HP bar child. Returns the
/// app, the enemy entities, and the player entity.
fn ui_app(enemy_healths: &[i32]) -> (App, Vec<Entity>, Entity) {
    let mut app = App::new();
    // `AssetPlugin` + `ScenePlugin` are required because the UI spawners now build
    // their hierarchies with `bsn!` + `spawn_scene` (which the real binary gets via
    // `DefaultPlugins`); they are not part of `MinimalPlugins`.
    app.add_plugins((
        MinimalPlugins,
        AssetPlugin::default(),
        ScenePlugin,
        StatesPlugin,
    ))
    .init_resource::<MenuSelection>()
    .init_resource::<LogView>()
    .init_resource::<UiConfig>()
    .init_state::<TurnPhase>()
    .add_message::<LogMessage>()
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
    .add_systems(Startup, (spawn_hud, spawn_battle_log, spawn_action_menu))
    .add_systems(OnExit(TurnPhase::PlayerTurn), clear_log_on_player_action)
    .add_systems(
        Update,
        (
            refresh_player_hud,
            refresh_enemy_labels,
            sync_enemy_label_text,
            update_enemy_label_highlight,
            sync_enemy_health_bars,
            update_menu_highlight,
            render_log_panel,
            swap_panel_for_phase,
            toggle_log_hint,
        )
            .in_set(BattleSet::Ui),
    );

    let player = app
        .world_mut()
        .spawn((Player, DisplayName("Hero".to_string()), Health::full(100)))
        .id();

    let enemies: Vec<Entity> = enemy_healths
        .iter()
        .enumerate()
        .map(|(index, &hp)| {
            let enemy = app
                .world_mut()
                .spawn((
                    Enemy { index },
                    DisplayName(format!("Goblin {index}")),
                    Health {
                        current: hp,
                        max: 100,
                    },
                    Visibility::Visible,
                ))
                .id();
            // The production path spawns the name label + mini HP bar inline in
            // the enemy `bsn!` scene; here we mirror the two
            // entities the assertions touch — the `EnemyNameLabel` (for the label
            // count and targeting-highlight cases) and the fill quad carrying
            // `EnemyHealthBar` (for the scale case) — directly through the world
            // spawner.
            app.world_mut().entity_mut(enemy).with_children(|parent| {
                parent.spawn((
                    EnemyNameLabel(enemy),
                    Text2d::new(format!("Goblin {index}")),
                    TextColor(Color::WHITE),
                ));
                parent.spawn((
                    EnemyHealthBar { owner: enemy },
                    Sprite::from_color(Color::WHITE, Vec2::new(48.0, 6.0)),
                    Transform::default(),
                ));
            });
            enemy
        })
        .collect();

    // Run a couple of frames so Startup spawns the tree and the first
    // `Changed<Health>` refresh populates the HUD.
    app.update();
    app.update();
    (app, enemies, player)
}

/// Set the current phase and let the transition + a UI frame apply.
fn set_phase(app: &mut App, phase: TurnPhase) {
    app.world_mut()
        .resource_mut::<NextState<TurnPhase>>()
        .set(phase);
    app.update();
    app.update();
}

fn text_of<C: Component>(app: &mut App) -> String {
    let mut query = app.world_mut().query_filtered::<&Text, With<C>>();
    query.single(app.world()).unwrap().0.clone()
}

/// The player HP fill width as a percent value, panicking if it is not a
/// `Val::Percent`. Lets the assertions compare the fraction with a float
/// tolerance rather than on exact `Val` equality (`100. * 60/100` is `60.000004`
/// in f32).
fn fill_percent(app: &mut App) -> f32 {
    let mut q = app
        .world_mut()
        .query_filtered::<&Node, With<PlayerHpFill>>();
    match q.single(app.world()).unwrap().width {
        Val::Percent(p) => p,
        other => panic!("expected the fill width to be a percent, got {other:?}"),
    }
}

fn damage(app: &mut App, entity: Entity, amount: i32) {
    let mut health = app.world_mut().entity_mut(entity);
    let mut hp = health.get_mut::<Health>().unwrap();
    hp.current = (hp.current - amount).max(0);
}

/// Damage moves the player HP fill width and the "(defeated)" suffix appears at
/// zero HP (`BattleUITest` fill-percent + defeated-label parity).
#[test]
fn player_hud_reflects_damage_and_defeat() {
    let (mut app, _enemies, player) = ui_app(&[100]);

    // Full health: name bare, fill 100%.
    assert_eq!(text_of::<PlayerNameLabel>(&mut app), "Hero");
    assert!((fill_percent(&mut app) - 100.0).abs() < 1e-3);

    // Take 40 damage → fill at 60%.
    damage(&mut app, player, 40);
    app.update();
    assert!((fill_percent(&mut app) - 60.0).abs() < 1e-3);
    assert_eq!(text_of::<PlayerNameLabel>(&mut app), "Hero");

    // Lethal damage → fill empty, defeated suffix.
    damage(&mut app, player, 60);
    app.update();
    assert!(fill_percent(&mut app).abs() < 1e-3);
    assert_eq!(text_of::<PlayerNameLabel>(&mut app), "Hero (defeated)");
}

/// The alive-enemy label count drops when an enemy is defeated
/// (`BattleUITest` label-count parity).
#[test]
fn enemy_label_count_drops_on_death() {
    let (mut app, enemies, _player) = ui_app(&[100, 100, 100]);

    let count = app
        .world_mut()
        .query::<&EnemyNameLabel>()
        .iter(app.world())
        .count();
    assert_eq!(count, 3, "all three enemies are labelled while alive");

    // Defeat the middle enemy.
    damage(&mut app, enemies[1], 100);
    app.update();
    let count = app
        .world_mut()
        .query::<&EnemyNameLabel>()
        .iter(app.world())
        .count();
    assert_eq!(count, 2, "the defeated enemy's label is removed");

    // The remaining labels are the two living enemies, not the dead one.
    let mut q = app.world_mut().query::<&EnemyNameLabel>();
    let named: Vec<Entity> = q.iter(app.world()).map(|l| l.0).collect();
    assert!(named.contains(&enemies[0]) && named.contains(&enemies[2]));
    assert!(!named.contains(&enemies[1]));
}

/// Editing an enemy's `DisplayName` (as the debug inspector does) updates its
/// world-space label text live, via `sync_enemy_label_text`.
#[test]
fn enemy_label_tracks_display_name_edits() {
    let (mut app, enemies, _player) = ui_app(&[100, 100]);

    let label_text_of = |app: &mut App, owner: Entity| -> String {
        let mut q = app.world_mut().query::<(&EnemyNameLabel, &Text2d)>();
        q.iter(app.world())
            .find(|(EnemyNameLabel(o), _)| *o == owner)
            .map(|(_, text)| text.0.clone())
            .unwrap()
    };

    assert_eq!(label_text_of(&mut app, enemies[0]), "Goblin 0");

    // Rename enemy 0, as an inspector edit to its `DisplayName` would.
    app.world_mut()
        .entity_mut(enemies[0])
        .get_mut::<DisplayName>()
        .unwrap()
        .0 = "Renamed".to_string();
    app.update();

    assert_eq!(
        label_text_of(&mut app, enemies[0]),
        "Renamed",
        "the label tracks the edited display name"
    );
    // The untouched enemy's label is unchanged.
    assert_eq!(label_text_of(&mut app, enemies[1]), "Goblin 1");
}

/// The mini HP bar fill scales with the owner's health fraction.
#[test]
fn enemy_health_bar_fill_scales() {
    let (mut app, enemies, _player) = ui_app(&[100]);

    let scale_of = |app: &mut App| {
        let mut q = app.world_mut().query::<(&EnemyHealthBar, &Transform)>();
        q.iter(app.world()).next().unwrap().1.scale.x
    };
    assert!((scale_of(&mut app) - 1.0).abs() < f32::EPSILON);

    damage(&mut app, enemies[0], 75);
    app.update();
    assert!((scale_of(&mut app) - 0.25).abs() < f32::EPSILON);
}

/// During targeting the enemy label under the cursor goes yellow and the menu
/// cursor still shows on its row — "greys menu but keeps cursor". Leaving
/// targeting clears the highlight ("hide-prompt restores colors").
#[test]
fn targeting_highlights_enemy_label_and_keeps_cursor() {
    let (mut app, enemies, _player) = ui_app(&[100, 100]);

    // Highlight row 0 in the menu and mark enemy 0 as targeted.
    app.world_mut().resource_mut::<MenuSelection>().highlighted = Some(0);
    app.world_mut().entity_mut(enemies[0]).insert(Targeted);
    set_phase(&mut app, TurnPhase::Targeting);

    // The targeted enemy's label is yellow; the other stays white.
    let mut q = app.world_mut().query::<(&EnemyNameLabel, &TextColor)>();
    for (EnemyNameLabel(entity), color) in q.iter(app.world()) {
        if *entity == enemies[0] {
            assert_eq!(color.0, HIGHLIGHT, "targeted enemy label is highlighted");
        } else {
            assert_eq!(color.0, WHITE, "untargeted enemy label stays white");
        }
    }

    // The menu cursor on the highlighted row 0 is still shown (cursor kept). The
    // highlighted cursor is `Inherited` rather than `Visible` so it tracks the
    // panel's visibility; "kept" means simply "not explicitly hidden".
    let mut cursors = app.world_mut().query::<(&MenuCursor, &Visibility)>();
    let row0_shown = cursors
        .iter(app.world())
        .any(|(MenuCursor(i), vis)| *i == 0 && *vis != Visibility::Hidden);
    assert!(row0_shown, "the menu cursor is kept on the selected row");

    // Leave targeting (remove the marker, as on_exit_targeting would) → no
    // label stays highlighted.
    app.world_mut().entity_mut(enemies[0]).remove::<Targeted>();
    set_phase(&mut app, TurnPhase::PlayerTurn);
    let mut q = app.world_mut().query::<(&EnemyNameLabel, &TextColor)>();
    assert!(
        q.iter(app.world()).all(|(_, color)| color.0 == WHITE),
        "leaving targeting clears every enemy highlight"
    );
}

/// The menu cursor tracks the action-menu panel: it is `Inherited` on the
/// highlighted row (so it follows the panel's visibility) rather than an explicit
/// `Visible` that would override the panel's `Hidden` and linger off-turn. When
/// the enemy turn hides the panel, the cursor is masked along with it; the player
/// turn shows it again.
#[test]
fn menu_cursor_follows_panel_visibility() {
    let (mut app, _enemies, _player) = ui_app(&[100]);

    // Row 0 is highlighted at the start of the player turn.
    app.world_mut().resource_mut::<MenuSelection>().highlighted = Some(0);
    app.update();

    let panel_visibility = |app: &mut App| {
        let mut q = app
            .world_mut()
            .query_filtered::<&Visibility, With<ActionMenuPanel>>();
        *q.single(app.world()).unwrap()
    };
    let row0_cursor_visibility = |app: &mut App| {
        let mut q = app.world_mut().query::<(&MenuCursor, &Visibility)>();
        q.iter(app.world())
            .find(|(MenuCursor(i), _)| *i == 0)
            .map(|(_, vis)| *vis)
            .unwrap()
    };

    // Player turn: panel visible, highlighted cursor inherits (so it shows).
    set_phase(&mut app, TurnPhase::PlayerTurn);
    assert_eq!(panel_visibility(&mut app), Visibility::Visible);
    assert_eq!(
        row0_cursor_visibility(&mut app),
        Visibility::Inherited,
        "the highlighted cursor inherits, so it shows while the panel is visible"
    );

    // Enemy turn: the panel hides, and because the cursor merely inherits it is
    // masked rather than lingering.
    set_phase(&mut app, TurnPhase::EnemyTurn);
    assert_eq!(panel_visibility(&mut app), Visibility::Hidden);
    assert_eq!(
        row0_cursor_visibility(&mut app),
        Visibility::Inherited,
        "the cursor still only inherits — now resolving to hidden with the panel"
    );
}

/// Log lines append as `Text` children; entering the enemy turn widens the panel
/// to 350 px and shows the log. The lines then **persist into the player turn**
/// (so the menu's `Log` option has something to review), with the menu restored to
/// 200 px; they clear only when the player commits an action (leaves `PlayerTurn`).
#[test]
fn log_appends_persists_into_player_turn_then_clears_on_action() {
    let (mut app, _enemies, _player) = ui_app(&[100]);

    let log_child_count = |app: &mut App| {
        let mut q = app
            .world_mut()
            .query_filtered::<Option<&Children>, With<BattleLogContainer>>();
        q.single(app.world()).unwrap().map_or(0, Children::len)
    };
    let panel_width = |app: &mut App| {
        let mut q = app
            .world_mut()
            .query_filtered::<&Node, With<ActionMenuPanel>>();
        q.single(app.world()).unwrap().width
    };

    // Player turn: narrow 200 px panel, empty log.
    assert_eq!(panel_width(&mut app), Val::Px(200.0));
    assert_eq!(log_child_count(&mut app), 0);

    // Two log lines arrive during the enemy turn.
    set_phase(&mut app, TurnPhase::EnemyTurn);
    app.world_mut()
        .resource_mut::<Messages<LogMessage>>()
        .write(LogMessage::new("Goblin 0 attacks Hero for 8 damage!"));
    app.world_mut()
        .resource_mut::<Messages<LogMessage>>()
        .write(LogMessage::new("Hero takes the hit!"));
    app.update();

    assert_eq!(
        log_child_count(&mut app),
        2,
        "both log lines append as children"
    );
    assert_eq!(
        panel_width(&mut app),
        Val::Px(350.0),
        "the auto-shown log widens the panel to 350 px during the enemy turn"
    );

    // Returning to the player turn keeps the lines (reviewable via the Log menu
    // option); the menu width restores to 200 px.
    set_phase(&mut app, TurnPhase::PlayerTurn);
    assert_eq!(
        log_child_count(&mut app),
        2,
        "the log persists into the player turn"
    );
    assert_eq!(
        panel_width(&mut app),
        Val::Px(200.0),
        "the menu width is restored"
    );

    // Committing an action (leaving PlayerTurn, e.g. Fight → Targeting) clears it.
    set_phase(&mut app, TurnPhase::Targeting);
    assert_eq!(
        log_child_count(&mut app),
        0,
        "the log clears when the player commits an action"
    );
}

/// Opening the log from the menu during the player turn (`LogView::open`) swaps
/// the centre panel: the action menu hides and the log panel shows, even though
/// the phase is still `PlayerTurn`. Closing it swaps back. This is the visual
/// half of the menu's `Log` option (the input half is covered in `action_menu`).
#[test]
fn opening_log_view_swaps_menu_for_log_during_player_turn() {
    let (mut app, _enemies, _player) = ui_app(&[100]);

    let menu_vis = |app: &mut App| {
        let mut q = app
            .world_mut()
            .query_filtered::<&Visibility, With<ActionMenuPanel>>();
        *q.single(app.world()).unwrap()
    };
    let log_vis = |app: &mut App| {
        let mut q = app
            .world_mut()
            .query_filtered::<&Visibility, With<BattleLogPanel>>();
        *q.single(app.world()).unwrap()
    };

    // Player turn, log closed: menu shows, log hidden.
    set_phase(&mut app, TurnPhase::PlayerTurn);
    assert_eq!(menu_vis(&mut app), Visibility::Visible);
    assert_eq!(log_vis(&mut app), Visibility::Hidden);

    // Open the log overlay (as the `Log` menu action does) — still PlayerTurn.
    app.world_mut().resource_mut::<LogView>().open = true;
    app.update();
    assert_eq!(
        menu_vis(&mut app),
        Visibility::Hidden,
        "the action menu hides while the log overlay is open"
    );
    assert_eq!(
        log_vis(&mut app),
        Visibility::Visible,
        "the log panel shows while the overlay is open"
    );

    // Close it: menu returns, log hides.
    app.world_mut().resource_mut::<LogView>().open = false;
    app.update();
    assert_eq!(menu_vis(&mut app), Visibility::Visible);
    assert_eq!(log_vis(&mut app), Visibility::Hidden);
}

/// The "Esc/Enter to close" hint shows only when the player opened the log from
/// the menu (`LogView::open`), and stays hidden during the enemy-turn auto-show
/// where there is nothing for the player to close.
#[test]
fn close_hint_shows_only_when_player_opened_the_log() {
    let (mut app, _enemies, _player) = ui_app(&[100]);

    let hint_vis = |app: &mut App| {
        let mut q = app
            .world_mut()
            .query_filtered::<&Visibility, With<LogHint>>();
        *q.single(app.world()).unwrap()
    };

    // Player turn, log closed: hint hidden.
    set_phase(&mut app, TurnPhase::PlayerTurn);
    assert_eq!(hint_vis(&mut app), Visibility::Hidden);

    // Enemy-turn auto-show: the log is up, but the hint stays hidden — the player
    // can't close it on the enemy's turn.
    set_phase(&mut app, TurnPhase::EnemyTurn);
    assert_eq!(
        hint_vis(&mut app),
        Visibility::Hidden,
        "no close hint during the enemy-turn auto-show"
    );

    // Back to the player turn and the player opens the log from the menu: the hint
    // appears (Inherited, so it shows with the now-visible panel).
    set_phase(&mut app, TurnPhase::PlayerTurn);
    app.world_mut().resource_mut::<LogView>().open = true;
    app.update();
    assert_eq!(
        hint_vis(&mut app),
        Visibility::Inherited,
        "the hint shows when the player opened the log"
    );

    // Closing the log hides the hint again.
    app.world_mut().resource_mut::<LogView>().open = false;
    app.update();
    assert_eq!(hint_vis(&mut app), Visibility::Hidden);
}

/// The close hint is styled as a footnote: italic and smaller than the log lines
/// (which use the default ~20 px body size).
#[test]
fn close_hint_is_italic_and_smaller_than_log_lines() {
    let (mut app, _enemies, _player) = ui_app(&[100]);

    let mut q = app.world_mut().query_filtered::<&TextFont, With<LogHint>>();
    let font = q.single(app.world()).unwrap();

    assert_eq!(font.style, FontStyle::Italic, "the hint is italic");
    match font.font_size {
        FontSize::Px(px) => assert!(
            px < 20.0,
            "the hint ({px} px) is smaller than the default body size"
        ),
        other => panic!("expected a pixel font size, got {other:?}"),
    }
}

/// A live `UiConfig` edit changes the active panel width the next frame — the
/// inspector-tunable parity case.
#[test]
fn ui_config_edit_changes_panel_width() {
    let (mut app, _enemies, _player) = ui_app(&[100]);

    let panel_width = |app: &mut App| {
        let mut q = app
            .world_mut()
            .query_filtered::<&Node, With<ActionMenuPanel>>();
        q.single(app.world()).unwrap().width
    };

    // Default action-menu half-width 100 → 200 px panel.
    assert_eq!(panel_width(&mut app), Val::Px(200.0));

    // Widen the action-menu half-width to 150 → 300 px panel.
    app.world_mut()
        .resource_mut::<UiConfig>()
        .action_menu_half_width = 150.0;
    app.update();
    assert_eq!(panel_width(&mut app), Val::Px(300.0));
}
