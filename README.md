# bevy-2d-rpg-game

A turn-based RPG battle vertical slice built in [Bevy](https://bevyengine.org)
0.18 — a port of an existing Godot 4.6 / C# game. See
[PORT_PLAN.md](PORT_PLAN.md) for the full design and the phased
[ROADMAP.md](ROADMAP.md) for status.

## Requirements

- Rust (edition 2024 — toolchain 1.85+) via [rustup](https://rustup.rs)
- [`just`](https://github.com/casey/just) for the task recipes (optional; the
  underlying `cargo` commands work directly too)
- On Linux, Bevy needs ALSA and udev dev headers: `libasound2-dev libudev-dev`

## Running

```sh
just run            # launch the game window
just run-debug      # launch with the egui debug inspector (F12 to toggle)
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
  LLD is already Rust's default linker on Linux, so no extra setup is needed
  there; the file documents how to opt into [mold](https://github.com/rui314/mold)
  (`sudo apt-get install mold clang`, then uncomment the mold line) and the
  nightly-only `share-generics` / parallel-frontend / `no-embed-metadata` flags.

`just run-debug` builds with the `debug-inspector` feature; press **F12** in-game
to open the [`bevy-inspector-egui`](https://github.com/jakobhellermann/bevy-inspector-egui)
world inspector and live-tune the registered knobs — `BattleLayout` (enemy
spacing/position), `UiConfig` (panel widths), and per-entity `Health` /
`CombatStats` / `DamageVariance` — replacing the original's `[Export(Range)]`
tuning. The feature is compiled out of `just run` and all tests, so egui never
ships in a normal build.

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
