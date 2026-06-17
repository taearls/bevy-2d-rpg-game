# bevy-2d-rpg-game

A turn-based RPG battle vertical slice built in [Bevy](https://bevyengine.org)
0.18 — a port of an existing Godot 4.6 / C# game. See
[PORT_PLAN.md](PORT_PLAN.md) for the full design and the phased
[ROADMAP.md](ROADMAP.md) for status.

## Gameplay

The game boots into a **main menu** (New Game / Options / Credits). Picking
**New Game** drops you onto an **overworld map** — walk the avatar around with
the **arrow keys or WASD**. Wandering far enough triggers a **random encounter**,
switching to a **turn-based battle**. Winning returns you to the map with your
hit points carried over (damage persists between fights); losing takes you to a
**game-over screen** that offers "Restart Game" (back to the map at full health)
or "Return to Title Screen".

## Requirements

- Rust (edition 2024 — toolchain 1.85+) via [rustup](https://rustup.rs)
- [`just`](https://github.com/casey/just) for the task recipes (optional; the
  underlying `cargo` commands work directly too)
- On Linux, Bevy needs ALSA and udev dev headers: `libasound2-dev libudev-dev`

## Running

```sh
just run            # launch the game window
just run-debug      # launch with the egui debug inspector (right-click a sprite to inspect)
just run-fast       # launch with Bevy dynamic linking (fastest iterative builds)
```

### Faster compiles

This repo applies Bevy's [recommended build optimizations](https://bevy.org/learn/quick-start/getting-started/setup/):

- **Dev `opt-level` profiles** in `Cargo.toml` — light optimization for our code,
  full optimization for dependencies.
- **Bevy dynamic linking** behind the `dynamic_linking` feature — `just run-fast`
  (or `cargo run --features dynamic_linking`) gives the biggest iterative
  build-time win. It is opt-in only; never ship a release build with it enabled.
- **`.cargo/config.toml`** mirroring Bevy's upstream `config_fast_builds.toml`.
  On Linux and macOS this file is **documentation-only today** — every linker and
  nightly directive is commented out, and the toolchain defaults (LLD on Linux,
  ld64 on macOS) are already the fast path, so it yields no build-time delta on
  those platforms on its own. It is there to make opting into
  [mold](https://github.com/rui314/mold) (`sudo apt-get install mold clang`, then
  uncomment the mold line) and the nightly-only `share-generics` /
  parallel-frontend / `no-embed-metadata` flags a one-line change. The only live
  directive is `rust-lld` for the `x86_64-pc-windows-msvc` target.

`just run-debug` builds with the `debug-inspector` feature. **Right-click any
sprite** (or Control+left-click on a trackpad) to open a focused
[`bevy-inspector-egui`](https://github.com/jakobhellermann/bevy-inspector-egui)
panel showing just that entity's components, editable live — `BattleLayout`
(enemy spacing/position), `UiConfig` (panel widths), and per-entity `Health` /
`CombatStats` / `DamageVariance`, replacing the original's `[Export(Range)]`
tuning. The panel is sticky (it follows the last-clicked entity until you click
another); dismiss it with **Esc** or the window's **×**. An **Enemies** list
window lets you pick a target without hunting for its sprite, and right-clicking
a component jumps to the source line where it last changed. The feature is
compiled out of `just run` and all tests, so egui never ships in a normal build.

## Development

```sh
just ci             # full quality gate: fmt check + clippy + tests
just test           # run tests (just test verbose for --nocapture output)
just format         # auto-fix formatting
just lint           # clippy with warnings as errors
```

### Spawn RNG

Battles roll enemies from a seeded RNG. Pin the seed for deterministic spawns:

```sh
just shuffle        # pin a random u64 seed to battle.seed
just shuffle 42     # pin a specific seed
just unshuffle      # drop the pinned seed (fresh RNG each launch)
```

## License

MIT — see [LICENSE](LICENSE). The Heroine Lyuba sprite assets are CC-BY 3.0;
see [`assets/sprites/lyuba/ATTRIBUTION.md`](assets/sprites/lyuba/ATTRIBUTION.md).
