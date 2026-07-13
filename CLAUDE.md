# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

A turn-based RPG battle vertical slice in **Bevy 0.19** (Rust edition 2024), ported from a Godot 4.6 / C# original. `PORT_PLAN.md` holds the design and the parity mapping back to the Godot source; `ROADMAP.md` tracks phased status. Many modules carry `// Mirrors the Godot ...` comments — when a behavior seems arbitrary, the original's parity is usually the reason, and the doc comment names the source file.

## Commands

Recipes live in `justfile`; the underlying `cargo` commands work directly too.

```sh
just run            # launch the game window
just run-fast       # launch with Bevy dynamic linking (fastest iterative builds)
just run-web        # serve the wasm build at http://127.0.0.1:8080 (needs trunk + wasm32 target)
just build-web      # size-optimized wasm bundle into ./dist (uses the wasm-release profile)

just ci             # FULL QUALITY GATE: fmt check + clippy (warnings as errors) + tests
just test           # cargo test (just test verbose → --nocapture)
just lint           # clippy --all-targets --all-features -D warnings
just format         # cargo fmt
```

Gate every change on `just ci` locally and rely on its **exit code** (see "CI" below). To run a single test: `cargo test <name>` (e.g. `cargo test --test battle_flow`, or `cargo test compute_damage`).

`just run-debug` launches with two debug tools, both gated behind the `debug-overlay` cargo feature (`src/debug/`):

1. **Diagnostics overlay** (`src/debug/mod.rs`) — Bevy's official `FpsOverlayPlugin` (from `bevy_dev_tools`) showing an FPS readout, plus a custom `debug/frame_counter` `Diagnostic` rendered in a `Text` node beneath it. Toggled in-game with **F12**. The graph half of `FpsOverlayPlugin` (`FrameTimeGraphConfig.enabled`) is turned **off** — its custom-material shader renders as a solid red block on the Metal backend, and only the numbers are wanted.
2. **Entity inspector** (`src/debug/inspector.rs`) — an egui-free, in-window stat editor. **Click** a combatant sprite to select it (a debug-only system inserts `Pickable` onto any entity with `Health`, so the player — which has no `Pickable` in gameplay code — is also clickable); the top-right panel lists its gameplay stats. **Tab**/**Shift+Tab** move the field cursor, **+/-** (or arrows) edit, **Shift** ×10, **Esc** closes. The editable allow-list is *gameplay only* (`Health`, `CombatStats`, `DamageVariance`) with clamping — never Bevy internals like `Transform`/`Sprite`. Selection ignores propagated ancestor hits (`Pointer<Click>` is an auto-propagating `EntityEvent`) by accepting only entities that carry an editable component.

   The inspector is **modal**: while an entity is selected it owns the keyboard, and battle input is mutually excluded so a keypress (or click) never acts on both. This rides on `DebugInputCapture` (`src/state.rs`) — an **always-compiled** `bool` resource, default `false` and only ever set by the inspector, so gameplay can gate on it without a `#[cfg]` dance and it's a no-op in default/release/wasm builds. The whole `BattleSet::Input` set is gated `run_if(not(DebugInputCapture::active))` (Resolve/Cleanup/Ui keep running, so HP bars and the log still redraw as stats change live). The inspector's *global* click observer sets the flag synchronously; because Bevy fires global observers before entity observers, the enemy's `on_enemy_clicked` entity observer (which early-returns on the flag) sees it already set, so an inspect-click during the `Targeting` phase never doubles as an attack. `sync_input_capture` clears the flag when the selection is dropped (Esc or despawn).

Both replaced the earlier `bevy-inspector-egui` community inspector, which had no Bevy 0.19 release — the egui dependency is gone. The overlay plugin is a no-op without a `RenderApp` (so headless `--features debug-overlay` tests stay green); the whole module is `#[cfg(feature = "debug-overlay")]` so default/release/wasm builds and tests never link `bevy_dev_tools`.

### Deterministic spawns

```sh
just shuffle [SEED]   # pin a u64 seed to battle.seed (random if omitted) → reproducible rosters
just unshuffle        # drop battle.seed → fresh entropy each launch
```

`battle.seed` is gitignored and desktop-only (wasm has no filesystem). Pinning seeds `SpawnRng` only; `DamageRng` is entropy-seeded at runtime (fixed-seed in tests so damage is assertable).

## Architecture

### Feature-plugin organization

Each gameplay feature is a module exposing a **free `fn plugin(app: &mut App)`** (not a `Plugin` struct), composed by `game::GamePlugin` in `src/game.rs`. Internal plugins are `pub(crate) fn plugin`; do not introduce `Plugin` structs for new internal features — match the existing convention. `src/lib.rs` re-exports all modules publicly so integration tests in `tests/` can build headless `App`s against the same plugins the binary uses.

`main.rs` is a thin shell: `DefaultPlugins` (window, nearest-neighbor sampling, `AssetMetaCheck::Never` — required so the wasm dev server doesn't break asset loads) + `GamePlugin`.

### UI via `bsn!`

UI and menu hierarchies are authored with the Bevy 0.19 **`bsn!` macro** + `commands.spawn_scene(...)`, not nested `.with_children(...)` closures. This covers the action menu (`battle/menu.rs`), the HUD (`battle/ui/hud.rs`), the battle log (`battle/ui/battle_log.rs`), the main menu / game-over screens, and the enemy + world-space HP-bar hierarchy (`battle/spawn.rs`). The conventions that fall out of `bsn!`'s `Template` system:

- **Components used in a `bsn!` need a `Template`.** A marker just needs `#[derive(Default, Clone)]` (the blanket `Default + Clone → Template` impl). A data component written in struct/tuple form (`Health { current, max }`, `Enemy { index }`) — or any component with an `Entity`/`Handle` field — needs `#[derive(FromTemplate)]`. Deriving `FromTemplate` **replaces** the blanket impl, so such a type is no longer a plain `Template` value.
- **Constructor-form values go through `template_value(...)`** — e.g. `template_value(DespawnOnExit(GameState::InBattle))`, `template_value(Transform::from_xyz(...))`, `template_value(BorderColor::all(...))`. `template_value` requires the argument to *be* a `Template` (a `Default + Clone` type like `Transform`/`Sprite`/`BorderColor`). A `FromTemplate`-deriving type (`Health`, `Sprite { ... }` field form) must instead use the macro's struct/tuple syntax — passing one to `template_value` fails with a misleading "does not implement `FromTemplate` … `Unpin`" error.
- **`DespawnOnExit<S>` can't be written bare** in `bsn!` (the macro can't infer the generic); always `template_value(DespawnOnExit(...))`.
- **Loops:** `bsn!` has no loop syntax. Index-parametrized rows (menus) are built as a `Vec<impl Scene>` (which is a `SceneList`) outside the macro and spliced into a `Children [ ... {rows} ]` block; each row is a small `fn menu_row(index, label) -> impl Scene`.
- **Self-references:** an entity's own id is reached with `#name` (e.g. the enemy is tagged `#enemy`, and its HP-bar children carry `EnemyHealthBar { owner: #enemy }`). Keep such children **inline in the same `bsn!`** — the `#name` scope is per-invocation, so the enemy overlay is built inline in `spawn.rs` (the HP-bar styling constants are `pub(crate)` for that), not in a separate helper.
- **Observers:** attach with `on(handler_fn)` inside the `bsn!` (the enemy's `on(on_enemy_clicked)`), replacing the old `.observe(...)` chain.
- **Tests:** `bsn!` resolution needs `AssetPlugin` + `ScenePlugin` in the `App`. The binary gets both from `DefaultPlugins`; headless tests in `tests/` that spawn UI/enemy scenes add them explicitly alongside `MinimalPlugins`/`StatesPlugin`.

### Two-level state machine

- **`GameState`** (`src/state.rs`): which screen — `MainMenu → Map → InBattle → GameOver`. All battle systems are gated `run_if(in_state(GameState::InBattle))`, so battle UI/combatants exist only during a fight.
- **`TurnPhase`** (`src/battle/state.rs`): turn flow *within* a battle — `PlayerTurn → Targeting → EnemyTurn → BattleOver`. Input is accepted only in the phase that owns it via `run_if(in_state(...))`, so "battle over disables input" needs no manual flag.

The battle outcome (victory/defeat) rides in a separate `BattleResult` resource, not on the `TurnPhase::BattleOver` variant, so every `in_state(BattleOver)` gate stays a plain unit match.

### Battle frame ordering

Battle systems run through four **chained `BattleSet`s** every frame (`src/battle/state.rs`), wired in `src/battle/mod.rs`:

```
Input → Resolve → Cleanup → Ui
```

The whole chain is `.chain().run_if(in_state(GameState::InBattle))`. `Input` queues `AttackRequested` messages; `Resolve` (`apply_attacks`) applies them and emits `DamageDealt`; `Cleanup` (`check_battle_end`) runs **only `on_message::<DamageDealt>`** — gating on the message rather than the state prevents a *cancelled* targeting from being wrongly pushed into EnemyTurn/BattleOver; `Ui` redraws cursor/HP-bars/log from world state. Combat is event-driven: producers (targeting, enemy turn) write `AttackRequested`; consumers read `DamageDealt`.

### Battle log (two views) (`src/battle/ui/battle_log.rs`)

`LogMessage`s feed **two** on-screen views, both fed off the same message stream (each system has its own `MessageReader` cursor):

- **Recent-lines box** (`BattleLogContainer`) — the auto-show during the enemy turn / battle-over (and the player's last attack), cleared each time the player commits an action (`clear_log_on_player_action`, `OnExit(PlayerTurn)`) so it only ever holds the last turn. To keep a freshly written line from flashing on a quick phase flip, `swap_panel_for_phase` holds the panel visible for ≥`LOG_VISIBLE_HOLD` (1.5s, wall-clock via `Time<Real>`) after the last write, tracked in `LogHold`; committing an action drops the lines *and* the hold.
- **Full history** (the menu's `Log` command) — a `BattleHistory` resource accumulates **every** line of the fight (reset `OnEnter(InBattle)`), rendered into a scrollable `HistoryViewport` (`HistoryContainer` inner column) shown only while `LogView::open`. The viewport uses **`max_height` + `Overflow::scroll_y`** so it hugs short content and caps + scrolls when long, and is toggled with **`Node.display`** (not `Visibility`) so a closed viewport takes no layout space (a hidden-but-laid-out node would shift the recent box). `manage_history_view` (Ui set) swaps the two boxes and rebuilds the lines on change, requesting a snap-to-bottom; `scroll_history` (Input set, `PlayerTurn`) does keyboard (Up/Down, PageUp/PageDown) + mouse-wheel scrolling and the deferred snap. **Scroll bounds come from the measured `ComputedNode`** (`content_size` minus the inner area inside padding/border, physical→logical via `inverse_scale_factor`), never a line-count estimate — so the bottom line lands fully visible regardless of font/padding.

### Combat is split for testability

`src/combat/` separates the **pure** damage math (`damage.rs`, `compute_damage` — unit-testable with no ECS) from the event vocabulary (`events.rs`) and the resolution systems (`resolve.rs`). When adding combat logic, keep formulas pure and push them down to `damage.rs`; reserve `resolve.rs` for the ECS plumbing.

### Shared components

`src/components.rs` is the shared ECS vocabulary (`Player`, `Enemy`, `Health`, `CombatStats`, `DamageVariance`, `Defending`, `Targeted`, …) used across `battle`, `combat`, and `map`. **Shared combat components live here, not under `characters/`.** `src/prelude.rs` re-exports the high-traffic types + `GameState` + `compute_damage`; keep it small and genuinely cross-cutting — single-feature types stay with their feature.

### Data-driven characters

The roster is authored as RON assets in `assets/characters/*.character.ron`, deserialized into `CharacterDef` (`src/characters/definition.rs`) via a custom asset loader. The RON files are the **single source of truth**: there are no serde defaults, so every stat field (`max_health`, `attack`, `defense`) and the `damage_variance` (`min`/`max`) block must be present — a template that omits any field fails to deserialize rather than silently falling back. The roster is preloaded at `Startup`; combatants spawn only when a battle begins, gated by `roster_ready` (all handles loaded) so spawning never races the async asset load. To add/tune a character, edit or add a `.character.ron` file — no Rust changes needed.

### Cross-battle persistence

A battle spawns a fresh player entity tagged `DespawnOnExit(InBattle)`, so HP can't live on the entity between fights. `PlayerProgress` (`src/progress.rs`) is the resource that survives transitions: a victory writes surviving `Health` back into it; the next battle seeds from it; New Game / Restart call `reset()`. The same `DespawnOnExit(State)` pattern tears down map and battle scenes on every state change.

## Conventions & gotchas

- **Bevy 0.19** — APIs differ from older tutorials (messages via `add_message`/`MessageWriter`/`on_message`, `DespawnOnExit`, `single()` returning `Result`). Match surrounding code; don't port 0.15-era idioms.
- **Clippy pedantic is on** (`Cargo.toml` `[lints.clippy]`), with deliberate allow-backs for Bevy-hostile lints (`needless_pass_by_value`, `cast_precision_loss`, `type_complexity`, …). Don't fight these; add a justified allow-back to `Cargo.toml` if a new one fires on idiomatic Bevy code.
- **Bevy features are hand-picked** in `Cargo.toml` with `default-features = false` to keep the wasm bundle and compile times small (no 3D/PBR/glTF/audio). If a new Bevy API fails to compile, you may be missing a feature flag — add the granular feature, not the `default`/`2d`/`ui` bundles. `bevy_picking`/`sprite_picking` are kept because the targeting indicator relies on them. **`bevy_scene` is enabled** (in both the shared and the wasm-target feature blocks) for the `bsn!` macro — see "UI via `bsn!`" below.
- **wasm build** uses the WebGL2 backend, routes RNG through `crypto.getRandomValues` (`getrandom` `wasm_js` feature + the `getrandom_backend` cfg in `.cargo/config.toml`), and ships via the size-optimized `wasm-release` profile. In `.cargo/config.toml` the *fast-build linker* directives are commented out (so the file is inert for native Linux/macOS builds); the only live directive is the wasm-target `getrandom_backend="wasm_js"` cfg, which fires only when building for `wasm32-unknown-unknown`.
- **Pure functions for headless tests** — encounter rolls, input→direction mapping, the damage formula, and seed parsing are all pure so the integration tests in `tests/` assert gameplay without a renderer or input device. Follow this when adding logic: extract the decision into a pure fn the system calls.

## CI

GitHub Actions CI is intentionally **`workflow_dispatch`-only** (manual) for iteration velocity — do **not** wait on PR checks to gate work. Gate locally on `just ci` instead. A separate `deploy-cloudflare` workflow builds the wasm bundle and redeploys to Cloudflare Pages on every push to `main`; the live site is gated behind Cloudflare Access (email allow-list) — see `README.md` for managing it.
