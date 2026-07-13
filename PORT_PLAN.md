# Port Plan: Godot 4.6 C# Turn-Based RPG → Bevy 0.18.1

## Context

The WIP RPG at [`taearls/rpg-game`](https://github.com/taearls/rpg-game) (local: `../rpg-game`) is a Godot 4.6 / C# (.NET 8) turn-based battle vertical slice: a single battle scene with an action menu (Fight / Items / Defend / Flee), keyboard + mouse targeting, 1–4 randomly spawned enemies from data-driven stat templates, an event-bus architecture, seeded spawn RNG for deterministic testing, a custom F12 debug inspector addon, and 15 GdUnit4 test classes. The goal is to reimplement the same game at feature parity in **Bevy 0.18.1** (latest stable) in this repo.

**Design decisions (locked in):**
1. **Debug tooling:** use `bevy-inspector-egui` (gated behind a cargo feature, F12 toggle) instead of porting the custom Godot inspector addon.
2. **Architecture:** idiomatic Bevy ECS — same behavior, native expression (Messages/Observers instead of the `BattleEvents` signal bus, `States` for turn phases, `Changed<T>` detection instead of `HealthUpdated` signals). No 1:1 Godot node mirroring.
3. **Testing:** each phase ships Rust tests mirroring the GdUnit4 coverage for what's ported (headless `App` tests + pure unit tests), with clippy/rustfmt gates and a justfile mirroring the Godot repo's recipes.

Work is tracked as one GitHub issue per phase (sequential; each blocked by the prior).

## Verified Bevy 0.18 API facts (researched June 2026 — do not "correct" these to older idioms)

- Buffered events are **Messages**: `#[derive(Message)]`, `App::add_message::<M>()`, `MessageWriter<M>` / `MessageReader<M>`. Observer events use `Event`/`EntityEvent` with observer systems taking `On<E>`; register via `App::add_observer` or `EntityCommands::observe`. (Split landed in 0.17.)
- `bevy_state` is a default feature: `#[derive(States)]`, `init_state`, `OnEnter`/`OnExit`, `in_state(...)`, `NextState`. 0.17 renamed `StateScoped` → `DespawnOnExit`. **0.18 change:** setting `NextState` to the *same* state re-fires `OnExit`/`OnEnter` — guard transitions.
- Picking: `picking`, `sprite_picking`, `ui_picking` are default features; `SpritePickingPlugin` ships in `DefaultPlugins`. Clicks via per-entity observer for `On<Pointer<Click>>` + `Pickable` component. `SpritePickingSettings { picking_mode: SpritePickingMode::BoundingBox }` for full-rect hits (parity with Godot's rectangular click areas).
- UI: `Node` is the layout component; text via `Text` + `TextFont` + `TextColor`; world-space text `Text2d`. No stable built-in ProgressBar — HP bars are hand-rolled nested `Node`s. `clear_children` → `detach_all_children` (0.17 rename).
- `bevy-inspector-egui = "0.36"` is the Bevy 0.18 line (pulls `bevy_egui 0.39`).
- `bevy_asset 0.18.1` uses `ron ^0.12` — pin `ron = "0.12"`.
- Headless deterministic time: insert `TimeUpdateStrategy::ManualDuration(d)` resource; each `app.update()` advances time by `d`.

## Key design decisions

- **RNG:** `rand 0.9` + `rand_chacha 0.9` (not `bevy_rand`). Two `Resource` wrappers around `ChaCha8Rng`: `SpawnRng` (optionally pinned by a `battle.seed` file, like Godot) and `DamageRng` (entropy-seeded, fixed-seed in tests). `ChaCha8Rng` guarantees a stable stream across rand releases.
- **Character data:** RON assets (`assets/characters/*.character.ron`) via a custom `AssetLoader`, mirroring the `.tres` data-driven design (roster edits without recompiling, hot reload). Phase 2 starts with plain Rust structs so domain logic is testable before the asset pipeline exists.
- **Events:** frame-buffered streams → `Message` (`LogMessage`, `AttackRequested`, `DamageDealt`); entity-scoped immediate reactions → `EntityEvent` + observers (`Died`, `Pointer<Click>`); health → UI via `Changed<Health>` (no event needed). This deletes the entire `BattleEvents` + `BusSubscriptions` disconnect-bookkeeping pattern — observers/messages are despawn-safe by construction.
- **Enemy-turn delays:** one resource `EnemyTurnQueue { pending: VecDeque<Entity>, timer: Timer }` built `OnEnter(TurnPhase::EnemyTurn)`, ticked by a single system — replaces Godot's chained `SceneTreeTimer`s and is deterministic under manual time.
- **Damage formula** (pure fn in `combat.rs`): `0` if attack ≤ 0; else `max(1, round(max(1, attack - defense) * variance))`, variance uniform in per-character `[min, max]` (defaults 0.8/1.2). Note: the C# implementation truncated instead of rounding; we round per spec and document the deliberate one-point divergence.

## Cargo.toml

```toml
[dependencies]
bevy = "0.18.1"            # defaults include bevy_state, bevy_ui, sprite_picking, png
rand = "0.9"
rand_chacha = "0.9"
serde = { version = "1", features = ["derive"] }
ron = "0.12"
bevy-inspector-egui = { version = "0.36", optional = true }

[features]
debug-inspector = ["dep:bevy-inspector-egui"]

[profile.dev]
opt-level = 1
[profile.dev.package."*"]
opt-level = 3

[lints.clippy]
type_complexity = "allow"
too_many_arguments = "allow"
```

`debug-inspector` is a cargo feature (not just `cfg(debug_assertions)`) so tests/release never compile egui; `just run-debug` enables it.

## Module layout

```
src/
  main.rs                  # App: DefaultPlugins + window (1152x648), GamePlugin
  lib.rs                   # pub modules so tests/ can import
  characters/              # CharactersPlugin
    components.rs          # DisplayName, Health, CombatStats, DamageVariance
    definition.rs          # CharacterDef / CombatStatsDef / DamageVarianceDef (serde, no defaults — RON is source of truth)
    asset_loader.rs        # AssetLoader for *.character.ron
  battle/                  # BattlePlugin: states, sets, messages, systems
    state.rs               # TurnPhase, BattleSet, BattleResult
    seed.rs                # parse_seed(&str) -> Option<u64>, read_seed_file()
    rng.rs                 # SpawnRng, DamageRng resources
    spawn.rs               # player + 1-4 enemies, name suffixing, layout
    combat.rs              # compute_damage(), apply_attacks, Died observer, win/loss
    targeting.rs           # cycle/confirm/cancel, tint, indicator, click handling
    enemy_turn.rs          # EnemyTurnQueue + timer system, Defend halving
    messages.rs            # LogMessage, AttackRequested, DamageDealt, Died, PlayerAction
  ui/                      # BattleUiPlugin
    layout.rs              # bottom panel scaffold, UiConfig resource
    action_menu.rs         # 4 rows, ">" cursor, highlight/disabled states
    hud.rs                 # player name + HP bar, enemy name labels
    battle_log.rs          # log lines from MessageReader<LogMessage>, panel widening
  debug/                   # #[cfg(feature = "debug-inspector")] DebugPlugin, F12 toggle
tests/
  battle_flow.rs           # full-loop headless integration
  smoke.rs                 # app-builds-and-spawns (BattleTscnSmokeTest equivalent)
assets/
  characters/hero.character.ron, goblin.character.ron
  sprites/hero.png, enemy.png, lyuba/*   # lyuba unused, copied for future (CC-BY 3.0 attribution)
  icons/icon.svg
```

## ECS design

- **State:** `enum TurnPhase { #[default] PlayerTurn, Targeting, EnemyTurn, BattleOver }` + `Resource BattleResult { victory: bool }`. Input systems gated by `in_state(...)` — "battle over disables input" falls out for free.
- **Components:** `Player`, `Enemy { index }`, `DisplayName(String)`, `Health { current, max }`, `CombatStats { attack, defense }`, `DamageVariance { min, max }`, `Defending` (marker; inserted by Defend, removed `OnEnter(PlayerTurn)`), `Targeted` (drives yellow sprite tint), `SelectionIndicator` (yellow `Mesh2d(Triangle2d)`, visibility-toggled), `EnemyHealthBar { owner }` (two child sprite quads: track + fill scaled by health fraction). UI markers: `ActionMenuPanel`, `MenuRow(usize)`, `MenuCursor`, `MenuLabel(usize)`, `PlayerHpFill`, `EnemyNameLabel(String)`, `BattleLogContainer`.
- **Resources:** `SpawnRng`, `DamageRng`, `BattleLayout { enemy_start_x, enemy_spacing, enemy_y, indicator_offset }`, `UiConfig { action_menu_half_width: 100.0, battle_log_half_width: 175.0 }`, `MenuState { selected }`, `SelectedTarget(Option<Entity>)`, `EnemyTurnQueue`, `LastPlayerAction`. The layout/UI resources are exactly what the inspector tunes in Phase 8 (replacing Godot `[Export(Range)]` knobs).
- **System sets:** `enum BattleSet { Input, Resolve, Cleanup, Ui }`, chained in `Update`. Input = keyboard nav + enemy-turn timer tick; Resolve = `apply_attacks` (drain `AttackRequested`, sample variance, mutate `Health`, trigger `Died`, write `DamageDealt`/`LogMessage`); Cleanup = `check_battle_end`; Ui = HP bars/labels from `Changed<Health>`, cursor, log, panel width, indicator transform.
- **Enemy turn:** `OnEnter(EnemyTurn)` flushes pending player-action messages and builds the queue from alive enemies in index order; first attack immediate, then 1.0 s between attacks; `Defending` halves the attack *value* before the formula; empty queue → back to `PlayerTurn`; player death short-circuits to `BattleOver`.
- **Clicking:** enemies spawned with `Pickable` + `.observe(on_enemy_clicked)`; observer delegates to a plain `try_select_target` fn (unit-testable without a renderer); in `Targeting`, a click on an alive enemy selects + confirms in one step (Godot parity).
- **UI:** bottom-anchored root `Node` (absolute, full width): left = dynamic alive-enemy name `Text` labels; center = action menu panel, width `Val::Px(200.)` ↔ `Val::Px(350.)` when the battle log shows (from `UiConfig`); right = player name + HP bar (outer `Node` track + inner `PlayerHpFill` with `width = Val::Percent(100. * current/max)`). Menu rows: yellow `>` cursor (visibility-toggled) + name label (yellow highlighted / grey disabled, cursor kept on selected row during targeting).

## Phases (each independently shippable, `just ci` green)

### Phase 1 — Scaffold, toolchain, assets
`cargo init`; Cargo.toml above; `main.rs` opens 1152×648 window with `DefaultPlugins` + `ImagePlugin::default_nearest()` (pixel art) + `ClearColor`; copy assets from the Godot repo (incl. lyuba with attribution note); `.gitignore` (`/target`, `battle.seed`); `rustfmt.toml`; **justfile**: `run`, `run-debug` (`--features debug-inspector`), `build`, `build-release`, `test [verbose]`, `format`, `format-check`, `lint` (`cargo clippy --all-targets --all-features -- -D warnings`), `ci`, and `shuffle [SEED]` / `unshuffle` ported from the Godot justfile (the `od -An -N8 -tu8` seed generation works unchanged).
**Tests:** one headless smoke test (MinimalPlugins, one update, no panic) proving the harness.

### Phase 2 — Core domain logic (pure, no rendering)
`characters/components.rs` + `definition.rs` (RON is the source of truth — no serde defaults; stats mirror `CombatStats.cs`); `battle/seed.rs`; `battle/rng.rs`; `combat::compute_damage`; pure `suffix_duplicate_names` ("Goblin A/B/C…").
**Tests (plain `#[test]`):** mirrors CharacterStatsTest (composition + no-defaults/RON contract), BattleCharacterTest damage cases (min-1 damage, floor at 0, is_alive, halved attack, attack ≤ 0 → 0), BattleSceneSeedParsingTest (valid, u64::MAX, whitespace trim, non-numeric, empty), suffixing.

### Phase 3 — Character assets + battle spawn
`CharacterDef` as `Asset` + `AssetLoader` (extension `character.ron`); `hero.character.ron` (Hero 120/12/8) and `goblin.character.ron` (Goblin 80/10/4); `spawn.rs` reads `battle.seed`, seeds `SpawnRng`, rolls 1–4 enemies from the roster, suffixes duplicates, spawns player + enemies (`Sprite`, `Transform` from `BattleLayout`, stats components); `Camera2d`. Visible: hero + a random enemy row.
**Tests:** headless spawn tests (stats injected directly): seeded count in 1–4, correct stats, spacing = `start_x + i * spacing`, indices, suffixes, same seed ⇒ identical roster twice; empty roster spawns none. (BattleSceneTest spawn coverage.)

### Phase 4 — Turn states + action menu
`TurnPhase` + `BattleSet`; menu UI (4 rows, yellow `>` cursor, wrap-around Up/Down, Enter confirm) gated `in_state(PlayerTurn)`; actions: Fight → `Targeting`; Items → log placeholder → `EnemyTurn`; Defend → insert `Defending` + queue message → `EnemyTurn`; Flee → log placeholder → `EnemyTurn`; `LogMessage` registered (stdout for now).
**Tests:** headless (MinimalPlugins + **`bevy::state::app::StatesPlugin`** — not in MinimalPlugins!): mirrors ActionMenuTest (cycle/wrap, confirm dispatch, 4 rows, cursor on exactly one row) + menu-related BattleSceneTest cases. Input simulated via `ButtonInput<KeyCode>::press` + `app.update()`.

### Phase 5 — Targeting + player attack + victory
`OnEnter(Targeting)`: select first alive enemy, indicator + tint + UI-name highlight, menu greyed with cursor kept; Left/Right cycles alive-only; Escape cancels; Enter confirms → `AttackRequested` → `apply_attacks` → `Died` observer hides sprite → `check_battle_end` ("Victory!" → `BattleOver`, else `EnemyTurn`). Sprite picking select+confirm on click.
**Tests:** BattleEventsTest parity (attack emits `DamageDealt`, lethal triggers `Died`, zero attack emits nothing) + targeting (cycle skips dead/wraps, cancel restores state, confirm damages + transitions, victory on last kill, `try_select_target` rejects dead).

### Phase 6 — Enemy turn, Defend resolution, game over
`EnemyTurnQueue` + tick system (immediate first attack, 1.0 s gaps); Defend halves attack for exactly one enemy turn; player death → "Game Over!" → `BattleOver { victory: false }`; round-trip back to `PlayerTurn`.
**Tests:** headless with `TimeUpdateStrategy::ManualDuration`: each alive enemy attacks once per round, dead skipped, no second attack before 1.0 s virtual time, Defend halving + expiry, mid-queue player death stops remaining attacks; full-loop integration in `tests/battle_flow.rs`.

### Phase 7 — HUD + battle log (UI parity)
Player name + HP fill from `Changed<Health>` ("(defeated)" suffix); dynamic enemy labels (alive only, removed on death, yellow target highlight); battle log: timestamped `Text` children from `MessageReader<LogMessage>`, menu↔log swap with 200 px ↔ 350 px panel width, log cleared `OnEnter(PlayerTurn)` (`detach_all_children`/despawn); enemy world-space mini HP bars.
**Tests:** BattleUITest parity — fill percent reflects damage, defeated suffix, label count drops on death, targeting greys menu but keeps cursor, log appends as children, menu restore clears log + 200 px, log mode = 350 px, `UiConfig` edit changes live width.

### Phase 8 — Debug inspector + polish
`debug/mod.rs` behind `debug-inspector`: `EguiPlugin` + `WorldInspectorPlugin` (bevy-inspector-egui 0.36) with F12-toggled `run_if` resource; `#[derive(Reflect)]` + `register_type` on `BattleLayout`, `UiConfig`, `DamageVariance`, `Health` etc. so all Godot `[Export]` knobs are live-tunable; parity audit vs the feature inventory; README (run/test/shuffle docs, lyuba CC-BY 3.0 attribution); finalize `tests/smoke.rs` (full plugin stack headless, seeded battle, 10 frames, no panic).
**Tests:** smoke; `cargo build` without the feature proves egui compiles out; `just ci` green with `--all-features`.

## Testing strategy (cross-phase)

```rust
let mut app = App::new();
app.add_plugins((MinimalPlugins, bevy::state::app::StatesPlugin))
   .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(250)))
   .add_plugins(BattlePlugin::headless());  // skips sprite/mesh-dependent systems
```
Assert via `query_filtered`, `State<TurnPhase>`, draining `Messages<LogMessage>`; UI tests assert ECS facts (`Text` content, `Node.width`, `Visibility`), never pixels. Tests seed `ChaCha8Rng::seed_from_u64` for exact assertions, including damage variance.

## Verification

Every phase: `just ci` (fmt-check + clippy -D warnings + test), then `just run` and check visible behavior:
- P1: window opens. P3: hero + 1–4 enemies; `just shuffle 42` twice ⇒ identical roster; `just unshuffle` ⇒ varies.
- P4: menu navigation/cursor. P5: targeting cycle, click-to-attack, victory message.
- P6: sequential enemy attacks with 1 s gaps, Defend halves damage in log numbers, game over on player death.
- P7: full HUD/log behavior incl. panel widening. P8: `just run-debug` + F12 shows inspector; tune `BattleLayout` live.

## Reference files (Godot source, read-only)
- `../rpg-game/scenes/battle/BattleScene.cs` — orchestration, spawn, targeting, seed logic
- `../rpg-game/scenes/battle/BattleUI.cs` — panel widths, log, label/highlight behavior
- `../rpg-game/scenes/battle/BattleCharacter.cs` — damage application, tint, health bars
- `../rpg-game/Resources/*.cs` + `*.tres` — data schema and instance values
- `../rpg-game/justfile` — recipe parity target

## Parity audit (Phase 8)

Final audit of the Bevy port against the Godot original's feature inventory.
Every phase shipped with `just ci` green; the table below maps each capability
from the original to its Bevy implementation. Status: **all phases complete.**

| Godot capability | Bevy implementation | Status |
|------------------|---------------------|--------|
| 1152×648 window, pixel-art sprites, clear color | `main.rs` `DefaultPlugins` + `ImagePlugin::default_nearest()` + `ClearColor` | ✅ |
| Data-driven character stats (`.tres`) | `CharacterDef` `Asset` + `*.character.ron` `AssetLoader`, `hero`/`goblin` templates | ✅ |
| Seeded spawn RNG (`battle.seed`) | `SpawnRng` (`ChaCha8Rng`), `read_seed_file`, `just shuffle`/`unshuffle` | ✅ |
| 1–4 random enemies, duplicate-name suffixing | `roll_roster` (`1..=MAX_ENEMIES`), `suffix_duplicate_names` | ✅ |
| Damage formula (variance, min-1, floor-0, Defend halving) | `combat::compute_damage` + `apply_attacks` (`DamageRng` variance, `Defending`) | ✅ |
| Turn state machine | `TurnPhase` `States` + chained `BattleSet { Input, Resolve, Cleanup, Ui }` | ✅ |
| Action menu (Fight/Items/Defend/Flee), `>` cursor, wrap nav | `menu.rs` keyboard nav gated `in_state(PlayerTurn)` | ✅ |
| Keyboard targeting (cycle alive, wrap, cancel, confirm) | `targeting.rs` Left/Right/Escape/Enter | ✅ |
| Mouse targeting (click selects + confirms) | `Pickable` + `on_enemy_clicked` observer → `try_select_target` (`BoundingBox`) | ✅ |
| Selection indicator + targeted tint | `SelectionIndicator` `Mesh2d(Triangle2d)`, `Targeted` yellow tint | ✅ |
| Enemy turn (queue, 1 s gaps, immediate first) | `EnemyTurnQueue` + `tick_enemy_turn` | ✅ |
| Victory / game over | `check_battle_end` ("Victory!"/"Game Over!"), `BattleResult { victory }` | ✅ |
| Event bus (`BattleEvents`) | `AttackRequested`/`DamageDealt`/`LogMessage` messages + `Died` observer | ✅ |
| HUD: player name + HP fill ("(defeated)") | `BattleUiPlugin` `PlayerHpFill` off `Changed<Health>` | ✅ |
| Dynamic enemy labels + target highlight | `refresh_enemy_labels` / `update_enemy_label_highlight` | ✅ |
| World-space enemy mini HP bars | `EnemyHealthBar` track + scaled fill, `sync_enemy_health_bars` | ✅ |
| Battle log + menu↔log panel swap (200 ↔ 350 px) | `render_log_panel`, `swap_panel_for_phase`, `UiConfig` | ✅ |
| Custom F12 debug inspector addon | Diagnostics overlay (`debug-overlay` feature): Bevy's official `FpsOverlayPlugin` (`bevy_dev_tools`), an F12-toggled FPS / frame-time readout — see note below | ✅ |
| GdUnit4 test suite | Headless `App` + pure unit tests across `tests/` and `src/`, `just ci` green | ✅ |

**Intentional design departures** (documented in Context above, not parity gaps):
the Godot signal bus is replaced by Bevy messages/observers, `Changed<T>`
detection replaces the `HealthUpdated` signal, and the bespoke Godot inspector
addon is replaced by Bevy's official diagnostics overlay (`FpsOverlayPlugin`) —
same dev-tooling intent, idiomatic Bevy expression. The original port used the
`bevy-inspector-egui` community crate for this, but it shipped no Bevy
0.19-compatible release, so the overlay swapped to the first-party `bevy_dev_tools`
plugin and the egui dependency was dropped.
