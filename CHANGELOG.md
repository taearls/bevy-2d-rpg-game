# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed

- 2026-06-13: Enabled `clippy::pedantic` with documented Bevy-friendly allow-backs in `Cargo.toml`; `just ci` green with no source-level violations (#10)

### Added

- 2026-06-13: Enemy turn, Defend resolution, and game over — an `EnemyTurnQueue { pending: VecDeque, timer }` built `OnEnter(EnemyTurn)` from the alive enemies in layout order (`enemy_turn_order`), with the first enemy attacking immediately (Godot `ProcessEnemyAttacks(0)`) and a repeating 1.0 s `tick_enemy_turn` pacing the rest — each pop writing `AttackRequested { enemy, player }`; `apply_attacks` now halves a `Defending` target's attack *value* before the damage formula, lasting exactly one enemy turn (`Defending` clears `OnEnter(PlayerTurn)`); `check_battle_end` gained the defeat branch — a player death writes "Game Over!" and a `BattleResult { victory: false }` resource and moves to `BattleOver`, and because the tick is `in_state(EnemyTurn)`-gated, that flip stops any remaining queued attacks — while an empty queue loops back to `PlayerTurn`; headless `enemy_turn` tests drive the pacing/Defend/defeat cases under `TimeUpdateStrategy::ManualDuration`, and `battle_flow` exercises the full `PlayerTurn → Targeting → EnemyTurn → PlayerTurn` round (#6)
- 2026-06-13: Targeting, player attack, and victory — a `Targeting`-phase cursor cycling alive enemies (Left/Right wrap, Escape cancels, Enter confirms) with a `Targeted` yellow sprite tint and a `Mesh2d(Triangle2d)` selection indicator; `AttackRequested`/`DamageDealt` messages and a `Died` `EntityEvent` whose observer hides the defeated sprite; `combat::apply_attacks` (variance from `DamageRng`, `Health` mutation, "attacks/defeated" log lines) and `check_battle_end` ("Victory!" → `BattleOver`, else `EnemyTurn`); `Pickable` enemies with a per-entity `On<Pointer<Click>>` observer delegating to a pure `try_select_target` (`SpritePickingMode::BoundingBox`); headless tests mirror the GdUnit4 `BattleEventsTest`/`BattleSceneTest` targeting cases (#5)
- 2026-06-13: Turn states + action menu — `TurnPhase` state machine and chained `BattleSet { Input, Resolve, Cleanup, Ui }` system sets; a bottom-anchored Fight/Items/Defend/Flee action menu with a visibility-toggled yellow `>` cursor and yellow/white highlight; player-turn-gated Up/Down (wrap) + Enter keyboard nav; Fight→Targeting, Items/Flee→log + EnemyTurn, Defend→`Defending` marker + queued message → EnemyTurn; a frame-buffered `LogMessage`; `Defending` cleared `OnEnter(PlayerTurn)`; headless tests mirror the GdUnit4 `ActionMenuTest`/`BattleSceneTest` menu cases (#4)
- 2026-06-13: Character RON assets + battle spawning — `CharacterDef` as a loadable `Asset` with a `*.character.ron` `AssetLoader`, `hero`/`goblin` templates, and a seeded spawn that rolls 1–4 enemies, suffixes duplicate names, and lays out the player + enemy row from `BattleLayout` with a `Camera2d`; headless spawn tests mirror the GdUnit4 `BattleSceneTest` coverage (#3)
- 2026-06-13: Core domain logic — character components/definitions (serde defaults 100/10/5), `parse_seed`/`read_seed_file`, `SpawnRng`/`DamageRng` (`ChaCha8Rng`), pure `compute_damage` (rounds where Godot truncated), and `suffix_duplicate_names`, with unit tests mirroring the GdUnit4 coverage (#2)
- 2026-06-12: Project scaffold — Bevy 0.18.1 Cargo project, 1152×648 game window, assets ported from the Godot repo (incl. Lyuba CC-BY 3.0 attribution), justfile quality gates, and a headless smoke test (#1)
- 2026-06-13: GitHub Actions CI workflow running the `just ci` quality gate on PRs, MIT `LICENSE` file, edition 2024, and a README run/develop guide (#1 review)
