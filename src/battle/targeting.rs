//! Enemy targeting: the `Targeting`-phase cursor that picks which enemy the
//! player attacks, plus the yellow tint + selection indicator that show it.
//!
//! Bevy port of the targeting half of the Godot `BattleScene`: Fight enters
//! [`Targeting`](TurnPhase::Targeting), where Left/Right cycle the *alive*
//! enemies (with wrap), Escape cancels back to the menu, and Enter (or a click)
//! confirms the attack. The selection logic is factored into pure functions —
//! [`alive_enemies`], [`cycle_target`], [`try_select_target`] — so the cycle,
//! wrap, and click-rejection cases can be asserted headlessly without a renderer
//! or an input device, exactly as the `GdUnit4` originals were.

use bevy::prelude::*;

use crate::components::{Enemy, Health, Player, Targeted};

use super::spawn::BattleLayout;
use super::state::TurnPhase;
use crate::combat::events::AttackRequested;

/// Yellow tint applied to the targeted enemy's sprite, and the colour of the
/// selection indicator. Matches the action-menu highlight.
const TARGET_TINT: Color = Color::srgb(1.0, 1.0, 0.0);
/// White — an untargeted sprite's natural tint (no recolour).
const NO_TINT: Color = Color::WHITE;

/// The enemy the targeting cursor currently sits on, or `None` outside the
/// [`Targeting`](TurnPhase::Targeting) phase. Mirrors Godot `_selectedEnemy`
/// (`null` when not targeting). Kept as a resource — rather than only the
/// [`Targeted`] marker — so the confirm/cancel input reads the selection in O(1)
/// without scanning every enemy.
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct SelectedTarget(pub Option<Entity>);

/// The yellow `Mesh2d(Triangle2d)` that hovers above the targeted enemy.
///
/// A single long-lived entity, repositioned over the current target and hidden
/// when nothing is targeted, rather than spawned/despawned per selection —
/// cheaper and keeps its mesh/material handles resident.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectionIndicator;

/// An `(entity, index)` pair for one living enemy, used by the pure cycle
/// helpers. `index` is the [`Enemy::index`] layout slot, which defines the
/// left-to-right cycle order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AliveEnemy {
    pub entity: Entity,
    pub index: usize,
}

/// Direction the targeting cursor moves: [`Right`](Self::Right) to the
/// next-higher enemy slot, [`Left`](Self::Left) to the previous, both wrapping.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CycleDirection {
    Left,
    Right,
}

/// Collect the living enemies as `(entity, index)` pairs, sorted by layout slot.
///
/// The sort makes the cycle order deterministic and independent of ECS iteration
/// order, so Left/Right always walk the on-screen row left-to-right. Defeated
/// enemies are excluded — they can neither be cycled to nor attacked.
#[must_use]
pub fn alive_enemies(query: &Query<(Entity, &Enemy, &Health)>) -> Vec<AliveEnemy> {
    let mut alive: Vec<AliveEnemy> = query
        .iter()
        .filter(|(_, _, health)| health.is_alive())
        .map(|(entity, enemy, _)| AliveEnemy {
            entity,
            index: enemy.index,
        })
        .collect();
    alive.sort_by_key(|enemy| enemy.index);
    alive
}

/// Step the selection to the next alive enemy in `direction`, with wrap-around.
///
/// Pure over `(current, alive, direction)`:
/// - empty `alive` ⇒ `None` (nothing to target).
/// - `current` not in `alive` (e.g. it just died, or we just entered targeting)
///   ⇒ the first alive enemy, regardless of direction — matching Godot's
///   "re-home onto a valid enemy" behaviour.
/// - otherwise step ±1 over the alive list modulo its length, so the ends wrap.
///
/// Cycling over the *compacted* alive list (not raw layout indices) is what
/// makes dead enemies transparently skipped.
#[must_use]
pub fn cycle_target(
    current: Option<Entity>,
    alive: &[AliveEnemy],
    direction: CycleDirection,
) -> Option<Entity> {
    if alive.is_empty() {
        return None;
    }
    let position = current.and_then(|entity| alive.iter().position(|a| a.entity == entity));
    let next = match position {
        None => 0,
        Some(index) => {
            let len = alive.len();
            match direction {
                CycleDirection::Right => (index + 1) % len,
                // `+ len - 1 ≡ -1 (mod len)` keeps the step unsigned so slot 0
                // wraps to the last enemy without a signed cast.
                CycleDirection::Left => (index + len - 1) % len,
            }
        }
    };
    Some(alive[next].entity)
}

/// Validate a would-be target (from a click or a confirm): return `Some(entity)`
/// only when it is one of the currently alive enemies, else `None`.
///
/// The single gate every selection path runs through, so a click on a defeated
/// enemy, an empty patch of screen (no entity), or a stale entity can never
/// become the attack target. Pure for headless testing.
#[must_use]
pub fn try_select_target(candidate: Entity, alive: &[AliveEnemy]) -> Option<Entity> {
    alive
        .iter()
        .any(|a| a.entity == candidate)
        .then_some(candidate)
}

/// `OnEnter(Targeting)`: home the cursor onto the first alive enemy.
///
/// Sets [`SelectedTarget`] to the lowest-index living enemy (or `None` if the
/// battle is somehow already won) and marks it [`Targeted`]. The tint and
/// indicator follow from [`Targeted`] via [`update_target_visuals`] in the UI
/// set. Mirrors Godot `StartTargeting` selecting the first enemy.
pub fn on_enter_targeting(
    mut commands: Commands,
    mut selected: ResMut<SelectedTarget>,
    enemies: Query<(Entity, &Enemy, &Health)>,
) {
    let alive = alive_enemies(&enemies);
    let target = alive.first().map(|a| a.entity);
    selected.0 = target;
    if let Some(entity) = target {
        commands.entity(entity).insert(Targeted);
    }
}

/// `OnExit(Targeting)`: clear the cursor.
///
/// Removes the [`Targeted`] marker from every enemy and resets
/// [`SelectedTarget`], so neither the tint nor the indicator lingers into the
/// enemy turn or a cancelled return to the menu. Runs on *both* confirm and
/// cancel — leaving targeting by any path tidies up.
pub fn on_exit_targeting(
    mut commands: Commands,
    mut selected: ResMut<SelectedTarget>,
    targeted: Query<Entity, With<Targeted>>,
) {
    for entity in &targeted {
        commands.entity(entity).remove::<Targeted>();
    }
    selected.0 = None;
}

/// `BattleSet::Input`, gated to [`Targeting`](TurnPhase::Targeting): Left/Right
/// cycle alive enemies, Escape cancels to the menu, Enter confirms the attack.
///
/// Confirm delegates to [`confirm_target`] so the click observer shares the exact
/// same path. Mirrors the `Targeting` branch of Godot
/// `BattleScene._UnhandledInput`.
pub fn targeting_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut commands: Commands,
    mut selected: ResMut<SelectedTarget>,
    mut next_state: ResMut<NextState<TurnPhase>>,
    mut attacks: MessageWriter<AttackRequested>,
    player: Query<Entity, With<Player>>,
    enemies: Query<(Entity, &Enemy, &Health)>,
) {
    if keys.just_pressed(KeyCode::Escape) {
        next_state.set(TurnPhase::PlayerTurn);
        return;
    }

    let direction = if keys.just_pressed(KeyCode::ArrowRight) {
        Some(CycleDirection::Right)
    } else if keys.just_pressed(KeyCode::ArrowLeft) {
        Some(CycleDirection::Left)
    } else {
        None
    };
    if let Some(direction) = direction {
        let alive = alive_enemies(&enemies);
        let next = cycle_target(selected.0, &alive, direction);
        set_target(&mut commands, &mut selected, next);
        return;
    }

    if keys.just_pressed(KeyCode::Enter) {
        confirm_target(selected.0, &player, &mut attacks, &mut next_state);
    }
}

/// Confirm an attack on `target` (when one is selected): write the
/// [`AttackRequested`] from the player and leave [`Targeting`](TurnPhase::Targeting).
///
/// We set [`EnemyTurn`](TurnPhase::EnemyTurn) only as a "leave targeting"
/// placeholder: the attack resolves this same frame in `Resolve`, then
/// [`check_battle_end`] runs in `Cleanup` and overrides the destination to
/// [`BattleOver`](TurnPhase::BattleOver) on a victory (or confirms `EnemyTurn`
/// otherwise). Leaving `Targeting` here is what stops its input and lets
/// `OnExit(Targeting)` clear the cursor. A confirm with no live selection is a
/// no-op.
///
/// [`check_battle_end`]: crate::combat::resolve::check_battle_end
fn confirm_target(
    target: Option<Entity>,
    player: &Query<Entity, With<Player>>,
    attacks: &mut MessageWriter<AttackRequested>,
    next_state: &mut NextState<TurnPhase>,
) {
    let (Some(target), Some(attacker)) = (target, player.iter().next()) else {
        return;
    };
    attacks.write(AttackRequested { attacker, target });
    // Resolution + the battle-end check run this frame in Resolve/Cleanup and
    // set the concrete next phase (EnemyTurn or BattleOver). Moving to a
    // placeholder here would race that; instead leave targeting by routing
    // through Resolve, which `check_battle_end` always overrides.
    next_state.set(TurnPhase::EnemyTurn);
}

/// Point [`SelectedTarget`] at `next`, moving the [`Targeted`] marker so exactly
/// the new target carries it (and nothing does when `next` is `None`).
fn set_target(commands: &mut Commands, selected: &mut SelectedTarget, next: Option<Entity>) {
    if selected.0 == next {
        return;
    }
    if let Some(previous) = selected.0 {
        commands.entity(previous).remove::<Targeted>();
    }
    if let Some(entity) = next {
        commands.entity(entity).insert(Targeted);
    }
    selected.0 = next;
}

/// Per-entity click observer: in [`Targeting`](TurnPhase::Targeting), a click on
/// an alive enemy selects *and* confirms in one step (Godot click-to-attack
/// parity); clicks in any other phase are ignored.
///
/// Spawned on each enemy via `.observe(on_enemy_clicked)`. The validity gate is
/// [`try_select_target`], the same one the keyboard path uses, so a click on a
/// defeated enemy is rejected identically.
pub fn on_enemy_clicked(
    click: On<Pointer<Click>>,
    state: Res<State<TurnPhase>>,
    mut commands: Commands,
    mut selected: ResMut<SelectedTarget>,
    mut attacks: MessageWriter<AttackRequested>,
    mut next_state: ResMut<NextState<TurnPhase>>,
    player: Query<Entity, With<Player>>,
    enemies: Query<(Entity, &Enemy, &Health)>,
) {
    if *state.get() != TurnPhase::Targeting {
        return;
    }
    let alive = alive_enemies(&enemies);
    let Some(target) = try_select_target(click.event().entity, &alive) else {
        return;
    };
    set_target(&mut commands, &mut selected, Some(target));
    confirm_target(Some(target), &player, &mut attacks, &mut next_state);
}

/// `BattleSet::Ui`: recolour enemy sprites by [`Targeted`] and park the
/// selection indicator over the current target.
///
/// The targeted enemy's sprite is tinted yellow and every other enemy is reset
/// to white, so the highlight always matches the live marker set with no manual
/// "unhighlight the old one" bookkeeping. The indicator is moved
/// [`indicator_offset`](BattleLayout::indicator_offset) above the targeted
/// enemy and shown, or hidden when nothing is targeted.
pub fn update_target_visuals(
    selected: Res<SelectedTarget>,
    layout: Res<BattleLayout>,
    mut sprites: Query<(&mut Sprite, Has<Targeted>), With<Enemy>>,
    targets: Query<&Transform, (With<Enemy>, Without<SelectionIndicator>)>,
    mut indicator: Query<
        (&mut Transform, &mut Visibility),
        (With<SelectionIndicator>, Without<Enemy>),
    >,
) {
    for (mut sprite, targeted) in &mut sprites {
        sprite.color = if targeted { TARGET_TINT } else { NO_TINT };
    }

    let Ok((mut transform, mut visibility)) = indicator.single_mut() else {
        return;
    };
    match selected.0.and_then(|entity| targets.get(entity).ok()) {
        Some(target_transform) => {
            let base = target_transform.translation;
            transform.translation = base + Vec3::new(0.0, layout.indicator_offset, 1.0);
            *visibility = Visibility::Visible;
        }
        None => *visibility = Visibility::Hidden,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn alive(entries: &[(Entity, usize)]) -> Vec<AliveEnemy> {
        entries
            .iter()
            .map(|&(entity, index)| AliveEnemy { entity, index })
            .collect()
    }

    /// Right wraps past the last enemy back to the first; Left wraps the other
    /// way — the targeting analogue of the menu's `cycle_index` wrap cases.
    #[test]
    fn cycle_wraps_both_directions() {
        let a = Entity::from_raw_u32(1).unwrap();
        let b = Entity::from_raw_u32(2).unwrap();
        let c = Entity::from_raw_u32(3).unwrap();
        let list = alive(&[(a, 0), (b, 1), (c, 2)]);

        assert_eq!(cycle_target(Some(a), &list, CycleDirection::Right), Some(b));
        assert_eq!(cycle_target(Some(c), &list, CycleDirection::Right), Some(a));
        assert_eq!(cycle_target(Some(a), &list, CycleDirection::Left), Some(c));
        assert_eq!(cycle_target(Some(b), &list, CycleDirection::Left), Some(a));
    }

    /// A selection that is no longer alive (or none at all) re-homes onto the
    /// first living enemy regardless of direction.
    #[test]
    fn cycle_from_missing_selection_goes_to_first() {
        let a = Entity::from_raw_u32(1).unwrap();
        let b = Entity::from_raw_u32(2).unwrap();
        let dead = Entity::from_raw_u32(99).unwrap();
        let list = alive(&[(a, 0), (b, 1)]);

        assert_eq!(cycle_target(None, &list, CycleDirection::Right), Some(a));
        assert_eq!(cycle_target(None, &list, CycleDirection::Left), Some(a));
        assert_eq!(
            cycle_target(Some(dead), &list, CycleDirection::Right),
            Some(a)
        );
    }

    /// With no living enemies there is nothing to cycle to.
    #[test]
    fn cycle_empty_is_none() {
        let a = Entity::from_raw_u32(1).unwrap();
        assert_eq!(cycle_target(Some(a), &[], CycleDirection::Right), None);
        assert_eq!(cycle_target(None, &[], CycleDirection::Left), None);
    }

    /// Only an entity in the alive set can be selected; a dead/unknown entity is
    /// rejected (`try_select_target rejects dead/invalid`).
    #[test]
    fn try_select_accepts_only_alive() {
        let a = Entity::from_raw_u32(1).unwrap();
        let b = Entity::from_raw_u32(2).unwrap();
        let dead = Entity::from_raw_u32(3).unwrap();
        let list = alive(&[(a, 0), (b, 1)]);

        assert_eq!(try_select_target(a, &list), Some(a));
        assert_eq!(try_select_target(b, &list), Some(b));
        assert_eq!(try_select_target(dead, &list), None);
        assert_eq!(try_select_target(a, &[]), None);
    }
}
