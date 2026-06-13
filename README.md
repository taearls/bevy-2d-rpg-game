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
```

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
