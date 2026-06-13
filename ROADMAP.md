# Roadmap

Godot 4.6 C# turn-based RPG → Bevy 0.18.1 port. One GitHub issue per phase,
sequential (each blocked by the prior). Full design context in
[PORT_PLAN.md](PORT_PLAN.md).

## Open Issues Summary

| Issue | Title | Priority | Effort | Status |
|-------|-------|----------|--------|--------|
| [#1](../../issues/1) | Phase 1: Project scaffold, toolchain, and assets | 🔴 Critical | ~0.5 day | ✅ Done |
| [#2](../../issues/2) | Phase 2: Core domain logic (damage formula, seed parsing, RNG, name suffixing) | 🔴 Critical | ~0.5 day | ✅ Done |
| [#3](../../issues/3) | Phase 3: Character RON assets + battle spawning | 🟡 High | ~1 day | ✅ Done |
| [#4](../../issues/4) | Phase 4: Turn states + action menu | 🟡 High | ~1 day | ✅ Done |
| [#5](../../issues/5) | Phase 5: Targeting, player attack, and victory | 🟡 High | ~1 day | ✅ Done |
| [#6](../../issues/6) | Phase 6: Enemy turn, Defend resolution, and game over | 🟡 High | ~1 day | ✅ Done |
| [#7](../../issues/7) | Phase 7: HUD + battle log UI parity | 🟢 Medium | ~1 day | Open |
| [#8](../../issues/8) | Phase 8: Debug inspector (bevy-inspector-egui) + polish | 🟢 Medium | ~0.5 day | Open |

## Current Sprint

**Next up:** [#7 — Phase 7: HUD + battle log UI parity](../../issues/7) (🟢 medium, unblocked now that #6 is done)

### Recently Completed

- ✅ [#6 — Phase 6: Enemy turn, Defend resolution, and game over](../../issues/6) —
  an `EnemyTurnQueue { pending, timer }` built `OnEnter(EnemyTurn)` from the alive
  enemies in index order; the first enemy attacks immediately (Godot
  `ProcessEnemyAttacks(0)`) and a repeating 1.0 s `tick_enemy_turn` paces the rest,
  each pop writing `AttackRequested { enemy, player }`; `apply_attacks` halves a
  `Defending` target's attack *value* before the formula (cleared
  `OnEnter(PlayerTurn)`, so it lasts exactly one enemy turn); `check_battle_end`
  gained the defeat branch — player death → "Game Over!" + `BattleResult { victory:
  false }` → `BattleOver`, with the `in_state(EnemyTurn)` gate stopping any
  remaining queued attacks; empty queue → back to `PlayerTurn`. Headless
  `enemy_turn` tests use `TimeUpdateStrategy::ManualDuration` for deterministic
  pacing, and `battle_flow` runs the full `PlayerTurn → Targeting → EnemyTurn →
  PlayerTurn` loop; `just ci` green.
- ✅ [#5 — Phase 5: Targeting, player attack, and victory](../../issues/5) —
  `Targeting`-phase cursor over alive enemies (Left/Right cycle with wrap,
  Escape cancels, Enter confirms), `Targeted` yellow tint + a `Mesh2d(Triangle2d)`
  `SelectionIndicator`; `AttackRequested`/`DamageDealt` messages + a `Died`
  `EntityEvent` (observer hides the sprite); `combat::apply_attacks` (variance
  from `DamageRng`, `Health` mutation, log lines) and `check_battle_end`
  ("Victory!" → `BattleOver`, else `EnemyTurn`); `Pickable` enemies with a
  per-entity `On<Pointer<Click>>` observer delegating to a pure
  `try_select_target` (`SpritePickingMode::BoundingBox`); headless
  `battle_combat`/`targeting` tests mirror `BattleEventsTest`/`BattleSceneTest`,
  `just ci` green.
- ✅ [#4 — Phase 4: Turn states + action menu](../../issues/4) — `TurnPhase`
  state machine + chained `BattleSet { Input, Resolve, Cleanup, Ui }`; a
  Fight/Items/Defend/Flee action menu with a visibility-toggled yellow `>`
  cursor, player-turn-gated wrap-around Up/Down + Enter nav, the Fight→Targeting
  / Items·Flee→log+EnemyTurn / Defend→`Defending`+message→EnemyTurn actions, a
  frame-buffered `LogMessage`, and `Defending` cleared `OnEnter(PlayerTurn)`;
  headless tests mirror `ActionMenuTest`/`BattleSceneTest`, `just ci` green.
- ✅ [#3 — Phase 3: Character RON assets + battle spawning](../../issues/3) —
  `CharacterDef` as a loadable `Asset` + `*.character.ron` `AssetLoader`,
  `hero`/`goblin` templates, and a seeded `spawn.rs` that rolls 1–4 enemies,
  suffixes duplicate names, and lays out player + enemy row from `BattleLayout`
  with a `Camera2d`; headless spawn tests mirror `BattleSceneTest`, `just ci` green.
- ✅ [#2 — Phase 2: Core domain logic](../../issues/2) — character components &
  serde defaults (100/10/5), `parse_seed`/`read_seed_file`, `SpawnRng`/`DamageRng`
  (`ChaCha8Rng`), pure `compute_damage` (rounds per spec vs Godot's truncation),
  and `suffix_duplicate_names`; unit tests mirror the GdUnit4 coverage, `just ci` green.
- ✅ [#10 — Tighten clippy configuration (pedantic)](../../issues/10) — enabled
  `clippy::pedantic` with documented Bevy-friendly allow-backs; `just ci` green
  with no source-level violations.
- ✅ [#1 — Phase 1: Project scaffold, toolchain, and assets](../../issues/1) — Cargo
  scaffold (Bevy 0.18.1), 1152×648 window, assets ported from the Godot repo,
  justfile quality gates (`just ci`), headless smoke test.

## Implementation Order

Strictly sequential: #2 → #3 → #4 → #5 → #6 → #7 → #8. Each phase ships
independently with `just ci` green.

## Issue Status Summary

- **Port phases:** 8 total — 6 done (#1, #2, #3, #4, #5, #6), 2 open (#7–#8); critical remaining: 0
- **Tooling & quality:** 1 total — 1 done (#10); all complete

## Changelog

- **2026-06-13** — Completed Phase 6 (#6): `EnemyTurnQueue` built `OnEnter(EnemyTurn)` from alive enemies in index order, immediate first attack + 1.0 s-paced `tick_enemy_turn`, `Defending` halving the attack value for one enemy turn, and the `check_battle_end` defeat branch ("Game Over!" + `BattleResult { victory: false }` → `BattleOver`) with the empty queue looping back to `PlayerTurn`.
- **2026-06-13** — Completed Phase 5 (#5): targeting cursor over alive enemies (cycle/wrap/cancel/confirm), `Targeted` tint + `Mesh2d(Triangle2d)` selection indicator, `AttackRequested`/`DamageDealt` messages + `Died` observer, `apply_attacks` + `check_battle_end` (Victory!), and click-to-attack sprite picking.
- **2026-06-13** — Completed Phase 4 (#4): `TurnPhase` state machine + chained `BattleSet`s, Fight/Items/Defend/Flee action menu with yellow `>` cursor and wrap-around keyboard nav, `LogMessage`, and the `Defending` marker lifecycle.
- **2026-06-13** — Completed Phase 3 (#3): character RON assets + `AssetLoader`, `hero`/`goblin` templates, and seeded 1–4 enemy battle spawning with layout and `Camera2d`.
- **2026-06-13** — Completed Phase 2 (#2): core domain logic — damage formula, seed parsing, RNG, name suffixing.
- **2026-06-13** — Completed tooling task #10 (tightened clippy config to pedantic).
- **2026-06-13** — Added tooling task #10 (tighten clippy config) to the roadmap.
- **2026-06-12** — Roadmap created; #1 (Phase 1 scaffold) completed.
