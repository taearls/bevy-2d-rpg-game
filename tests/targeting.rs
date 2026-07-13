//! Headless targeting coverage, mirroring the Godot `BattleSceneTest` targeting
//! cases.
//!
//! Builds an `App` with the turn-state machine, the action menu's
//! `OnEnter(PlayerTurn)` highlight, the full targeting input/lifecycle, and the
//! combat resolver + `check_battle_end`, then drives it by pressing keys on the
//! `ButtonInput<KeyCode>` resource — exactly the pattern the action-menu tests
//! use. Assertions are ECS facts: the `SelectedTarget` resource, the `Targeted`
//! marker set, `Health`, the `TurnPhase` state, and the menu highlight.
//!
//! As in the menu tests, `InputPlugin` is deliberately omitted (its `PreUpdate`
//! system would wipe a manually-pressed key) and `StatesPlugin` is added
//! explicitly (it is not part of `MinimalPlugins`). The renderer-only
//! `update_target_visuals` system is *not* wired: the selection logic it draws
//! from (`SelectedTarget` / `Targeted`) is asserted directly.

use bevy::prelude::*;
use bevy::state::app::StatesPlugin;

use aliasing::battle::enemy_turn::EnemyTurnQueue;
use aliasing::battle::menu::{MenuSelection, on_enter_player_turn};
use aliasing::battle::messages::LogMessage;
use aliasing::battle::rng::DamageRng;
use aliasing::battle::state::{BattleResult, BattleSet, TurnPhase};
use aliasing::battle::targeting::{
    SelectedTarget, on_enter_targeting, on_exit_targeting, targeting_input,
};
use aliasing::combat::events::{AttackRequested, DamageDealt};
use aliasing::combat::resolve::{apply_attacks, check_battle_end, on_died_hide_sprite};
use aliasing::components::{
    CombatStats, DamageVariance, DisplayName, Enemy, Health, Player, Targeted,
};

/// Build a headless battle app starting in `Targeting` with `enemy_healths`
/// enemies (index `0..n`, given starting HP) and one player. The menu
/// `OnEnter(PlayerTurn)` highlight is wired so the "cancel restores the menu"
/// case is observable.
fn targeting_app(enemy_healths: &[i32]) -> (App, Vec<Entity>, Entity) {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, StatesPlugin))
        .init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<MenuSelection>()
        .init_resource::<SelectedTarget>()
        .init_resource::<EnemyTurnQueue>()
        .init_resource::<BattleResult>()
        .init_state::<TurnPhase>()
        .add_message::<LogMessage>()
        .add_message::<AttackRequested>()
        .add_message::<DamageDealt>()
        .insert_resource(DamageRng::from_seed(0))
        .add_observer(on_died_hide_sprite)
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
        .add_systems(OnEnter(TurnPhase::PlayerTurn), on_enter_player_turn)
        .add_systems(OnEnter(TurnPhase::Targeting), on_enter_targeting)
        .add_systems(OnExit(TurnPhase::Targeting), on_exit_targeting)
        .add_systems(
            Update,
            (
                targeting_input
                    .in_set(BattleSet::Input)
                    .run_if(in_state(TurnPhase::Targeting)),
                apply_attacks.in_set(BattleSet::Resolve),
                check_battle_end
                    .in_set(BattleSet::Cleanup)
                    .run_if(on_message::<DamageDealt>),
            ),
        );

    let player = app
        .world_mut()
        .spawn((
            Player,
            DisplayName("Hero".to_string()),
            CombatStats {
                attack: 50,
                defense: 0,
            },
            // Pinned variance so a confirmed attack deals exactly 50.
            DamageVariance { min: 1.0, max: 1.0 },
            Health::full(100),
        ))
        .id();

    let enemies: Vec<Entity> = enemy_healths
        .iter()
        .enumerate()
        .map(|(index, &hp)| {
            app.world_mut()
                .spawn((
                    Enemy { index },
                    DisplayName(format!("Goblin {index}")),
                    CombatStats {
                        attack: 10,
                        defense: 0,
                    },
                    DamageVariance::default(),
                    Health::full(hp),
                    Visibility::Visible,
                ))
                .id()
        })
        .collect();

    // Enter the targeting phase: `OnEnter(Targeting)` homes onto the first alive
    // enemy.
    app.world_mut()
        .resource_mut::<NextState<TurnPhase>>()
        .set(TurnPhase::Targeting);
    app.update();

    (app, enemies, player)
}

/// Press `key`, run one frame so `targeting_input` sees the `just_pressed` edge,
/// then release + reset so the next press of the same key is a fresh edge. A
/// trailing input-free `update` lets any queued `NextState` transition (and its
/// `OnEnter`/`OnExit` systems) take effect before assertions — mirroring the
/// action-menu test harness.
fn press(app: &mut App, key: KeyCode) {
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(key);
    app.update();
    {
        let mut input = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        input.release(key);
        input.reset(key);
    }
    app.update();
}

fn selected(app: &mut App) -> Option<Entity> {
    app.world().resource::<SelectedTarget>().0
}

fn current_phase(app: &mut App) -> TurnPhase {
    *app.world().resource::<State<TurnPhase>>().get()
}

fn health_of(app: &mut App, entity: Entity) -> i32 {
    app.world().entity(entity).get::<Health>().unwrap().current
}

/// The set of entities currently carrying the `Targeted` marker.
fn targeted(app: &mut App) -> Vec<Entity> {
    app.world_mut()
        .query_filtered::<Entity, With<Targeted>>()
        .iter(app.world())
        .collect()
}

/// `OnEnter(Targeting)` homes onto the first (lowest-index) enemy and marks
/// exactly it `Targeted`.
#[test]
fn entering_targeting_selects_first_enemy() {
    let (mut app, enemies, _) = targeting_app(&[80, 80, 80]);
    assert_eq!(selected(&mut app), Some(enemies[0]));
    assert_eq!(targeted(&mut app), vec![enemies[0]]);
}

/// Right cycles forward through alive enemies and wraps past the last back to
/// the first; the `Targeted` marker follows the selection.
#[test]
fn right_cycles_and_wraps() {
    let (mut app, enemies, _) = targeting_app(&[80, 80, 80]);

    press(&mut app, KeyCode::ArrowRight);
    assert_eq!(selected(&mut app), Some(enemies[1]));
    assert_eq!(targeted(&mut app), vec![enemies[1]]);

    press(&mut app, KeyCode::ArrowRight);
    assert_eq!(selected(&mut app), Some(enemies[2]));

    // Wrap back to the first.
    press(&mut app, KeyCode::ArrowRight);
    assert_eq!(selected(&mut app), Some(enemies[0]));
}

/// Left wraps from the first enemy to the last.
#[test]
fn left_wraps_backward() {
    let (mut app, enemies, _) = targeting_app(&[80, 80, 80]);
    press(&mut app, KeyCode::ArrowLeft);
    assert_eq!(selected(&mut app), Some(enemies[2]));
}

/// Cycling skips dead enemies entirely — a defeated middle enemy is never
/// selected.
#[test]
fn cycle_skips_dead_enemies() {
    // Middle enemy starts dead (0 HP).
    let (mut app, enemies, _) = targeting_app(&[80, 0, 80]);
    // Starts on the first alive enemy.
    assert_eq!(selected(&mut app), Some(enemies[0]));

    // Right should jump straight to index 2, skipping the dead index 1.
    press(&mut app, KeyCode::ArrowRight);
    assert_eq!(selected(&mut app), Some(enemies[2]));

    // And wrap back to 0, still skipping 1.
    press(&mut app, KeyCode::ArrowRight);
    assert_eq!(selected(&mut app), Some(enemies[0]));
}

/// Escape cancels targeting: back to `PlayerTurn`, the `Targeted` marker is
/// cleared, `SelectedTarget` is reset, and the menu re-highlights row 0.
#[test]
fn escape_cancels_and_restores_menu() {
    let (mut app, _, _) = targeting_app(&[80, 80]);
    assert_eq!(current_phase(&mut app), TurnPhase::Targeting);

    press(&mut app, KeyCode::Escape);

    assert_eq!(current_phase(&mut app), TurnPhase::PlayerTurn);
    assert_eq!(selected(&mut app), None, "selection cleared on cancel");
    assert!(targeted(&mut app).is_empty(), "Targeted cleared on cancel");
    assert_eq!(
        app.world().resource::<MenuSelection>().highlighted,
        Some(0),
        "menu re-highlights row 0 on return to PlayerTurn"
    );
}

/// Enter confirms: the selected enemy takes damage, and the battle moves to
/// `EnemyTurn` (enemies remain), with the targeting cursor cleared.
#[test]
fn confirm_damages_target_and_advances_to_enemy_turn() {
    let (mut app, enemies, _) = targeting_app(&[80, 80]);
    // Cursor is on enemy 0.
    press(&mut app, KeyCode::Enter);

    assert_eq!(
        health_of(&mut app, enemies[0]),
        80 - 50,
        "confirmed attack deals 50 to the selected enemy"
    );
    assert_eq!(
        current_phase(&mut app),
        TurnPhase::EnemyTurn,
        "enemies remain, so the turn passes to them"
    );
    assert!(
        targeted(&mut app).is_empty(),
        "OnExit(Targeting) clears the cursor"
    );
}

/// Killing the last living enemy ends the battle in victory.
#[test]
fn confirm_killing_last_enemy_wins() {
    // A single enemy with less HP than the 50-damage hit.
    let (mut app, enemies, _) = targeting_app(&[30]);
    press(&mut app, KeyCode::Enter);

    assert_eq!(health_of(&mut app, enemies[0]), 0, "the last enemy dies");
    assert_eq!(
        current_phase(&mut app),
        TurnPhase::BattleOver,
        "all enemies dead ⇒ Victory! ⇒ BattleOver"
    );
    assert!(
        app.world().resource::<BattleResult>().victory,
        "the recorded outcome is a victory"
    );
}

/// A confirm leaves the *other* enemy alive when only one of several dies, so it
/// is the enemy turn (not victory) that follows.
#[test]
fn confirm_killing_one_of_two_does_not_win() {
    let (mut app, enemies, _) = targeting_app(&[30, 80]);
    // Enemy 0 (30 HP) dies to the 50 hit; enemy 1 survives.
    press(&mut app, KeyCode::Enter);

    assert_eq!(health_of(&mut app, enemies[0]), 0);
    assert_eq!(health_of(&mut app, enemies[1]), 80);
    assert_eq!(current_phase(&mut app), TurnPhase::EnemyTurn);
}
