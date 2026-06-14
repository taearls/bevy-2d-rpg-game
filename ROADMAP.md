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
| [#7](../../issues/7) | Phase 7: HUD + battle log UI parity | 🟢 Medium | ~1 day | ✅ Done |
| [#8](../../issues/8) | Phase 8: Debug inspector (bevy-inspector-egui) + polish | 🟢 Medium | ~0.5 day | ✅ Done |

## Current Sprint

**🎉 Port complete — all 8 phases shipped.** The Godot 4.6 C# turn-based RPG
vertical slice is fully reimplemented in Bevy 0.18.1 at feature parity (see the
[Parity audit](PORT_PLAN.md#parity-audit-phase-8) in PORT_PLAN.md). No open
issues remain.

### Recently Completed

- ✅ [#8 — Phase 8: Debug inspector (bevy-inspector-egui) + polish](../../issues/8) —
  a `debug/mod.rs` `DebugPlugin` behind the `debug-inspector` cargo feature wiring
  `EguiPlugin::default()` + `WorldInspectorPlugin` (bevy-inspector-egui 0.36 / the
  Bevy 0.18 line) gated on an F12-toggled `InspectorEnabled` resource via `run_if`
  (a no-op without a `RenderApp`, so headless `--all-features` tests stay green);
  `#[derive(Reflect)]` + `register_type` on `BattleLayout`, `UiConfig`, `Health`,
  `CombatStats`, and `DamageVariance` so every former Godot `[Export(Range)]` knob
  is live-editable; a parity audit table in PORT_PLAN.md; README run-debug/F12
  docs; and a finalized `tests/smoke.rs` driving the full `GamePlugin` stack
  headless on a pinned seed (player + 1..=`MAX_ENEMIES` enemies spawn, 10 frames
  run without panic) — `just ci` green and `cargo build` with no feature proving
  egui compiles out entirely.
- ✅ [#7 — Phase 7: HUD + battle log UI parity](../../issues/7) — a `BattleUiPlugin`
  driving the player name + percentage-width `PlayerHpFill`
  (`Val::Percent(100 * current/max)`, "(defeated)" suffix on death) off
  `Changed<Health>`; dynamic alive-enemy name labels rebuilt on death with a
  yellow highlight following the `Targeted` marker; world-space enemy mini HP
  bars (track + fill quad scaled by health fraction under each sprite); a
  battle-log panel spawning a `Text` child per `LogMessage` into
  `BattleLogContainer`; and a `UiConfig { action_menu_half_width: 100.0,
  battle_log_half_width: 175.0 }` swapping the centre panel 200 px ↔ 350 px
  (menu shown in `PlayerTurn`/`Targeting`, log shown during
  `EnemyTurn`/`BattleOver`, cleared `OnEnter(PlayerTurn)`); headless
  `tests/battle_ui.rs` mirrors the GdUnit4 `BattleUITest` (fill percent,
  defeated suffix, label-count drop, targeting highlight, log append + width
  swap, live `UiConfig` edit), `just ci` green.
- ✅ [#6 — Phase 6: Enemy turn, Defend resolution, and game over](../../issues/6) —
  an `EnemyTurnQueue { pending: VecDeque<Entity>, timer: Timer }` built
  `OnEnter(EnemyTurn)` from alive enemies in index order; a `tick_enemy_turn`
  releaser (first attack immediate, then 1.0 s gaps) writing `AttackRequested`
  per pop; `apply_attacks` halving the attacker's *attack value* before the
  formula while the target carries `Defending` (cleared `OnEnter(PlayerTurn)`);
  `check_battle_end` extended with a player-death "Game Over!" branch
  (`BattleOver`, queue cleared so mid-queue death stops remaining attacks) and an
  empty-queue hand-back to `PlayerTurn`, plus a queryable `BattleResult { victory }`
  resource recording the outcome; deterministic headless tests with
  `TimeUpdateStrategy::ManualDuration` plus a full-loop `tests/battle_flow.rs`,
  `just ci` green.
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

- **Port phases:** 8 total — 8 done (#1–#8), 0 open; critical remaining: 0; high remaining: 0; **milestone complete**
- **Tooling & quality:** 1 total — 1 done (#10); all complete

## Changelog

- **2026-06-13** — Completed Phase 8 (#8) — **port milestone complete**: `DebugPlugin` (`debug-inspector` feature) wiring `EguiPlugin` + `WorldInspectorPlugin` behind an F12-toggled `InspectorEnabled` resource (no-op without a `RenderApp` so headless `--all-features` tests pass); `#[derive(Reflect)]` + `register_type` on `BattleLayout`, `UiConfig`, `Health`, `CombatStats`, `DamageVariance`; a parity-audit table in PORT_PLAN.md; README run-debug/F12 docs; and a finalized full-stack seeded `tests/smoke.rs` (spawns a battle, runs 10 frames, no panic). `just ci` green; `cargo build` without the feature proves egui compiles out.
- **2026-06-13** — Completed Phase 7 (#7): `BattleUiPlugin` HUD + battle log — player name/`PlayerHpFill` percentage bar with "(defeated)" suffix (`Changed<Health>`), dynamic alive-enemy labels with the `Targeted` yellow highlight, world-space enemy mini HP bars (track + scaled fill), a `LogMessage`-driven log panel, and a `UiConfig` swapping the centre panel 200 px ↔ 350 px (log shown during `EnemyTurn`/`BattleOver`, cleared `OnEnter(PlayerTurn)`) — with `tests/battle_ui.rs` mirroring `BattleUITest`.
- **2026-06-13** — Completed Phase 6 (#6): `EnemyTurnQueue` + `tick_enemy_turn` (immediate first attack, 1.0 s gaps, empty-queue hand-back to `PlayerTurn`), `Defending` halving the attack value before the formula for one turn, and a player-death "Game Over!" branch in `check_battle_end` (`BattleOver`, queue cleared) — with deterministic `ManualDuration` tests and a full-loop `tests/battle_flow.rs`.
- **2026-06-13** — Completed Phase 5 (#5): targeting cursor over alive enemies (cycle/wrap/cancel/confirm), `Targeted` tint + `Mesh2d(Triangle2d)` selection indicator, `AttackRequested`/`DamageDealt` messages + `Died` observer, `apply_attacks` + `check_battle_end` (Victory!), and click-to-attack sprite picking.
- **2026-06-13** — Completed Phase 4 (#4): `TurnPhase` state machine + chained `BattleSet`s, Fight/Items/Defend/Flee action menu with yellow `>` cursor and wrap-around keyboard nav, `LogMessage`, and the `Defending` marker lifecycle.
- **2026-06-13** — Completed Phase 3 (#3): character RON assets + `AssetLoader`, `hero`/`goblin` templates, and seeded 1–4 enemy battle spawning with layout and `Camera2d`.
- **2026-06-13** — Completed Phase 2 (#2): core domain logic — damage formula, seed parsing, RNG, name suffixing.
- **2026-06-13** — Completed tooling task #10 (tightened clippy config to pedantic).
- **2026-06-13** — Added tooling task #10 (tighten clippy config) to the roadmap.
- **2026-06-12** — Roadmap created; #1 (Phase 1 scaffold) completed.
