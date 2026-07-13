# Build Optimization Benchmarks

Measured comparison of the `.cargo/config.toml` / `Cargo.toml` build settings that
PR #23 added (mostly commented out), to decide which are worth enabling.

**Machine:** Apple Silicon (`aarch64-apple-darwin`), macOS. Default toolchain
`stable`, `nightly` also installed. Crate: `aliasing` (then named
`bevy-2d-rpg-game`, Bevy 0.18).

**Method:** `/usr/bin/time -p cargo build`. Three numbers per config:

- **Clean build** — `cargo clean` first, then build everything (Bevy + deps + crate).
  Dominates total wall-clock but you rarely run it.
- **Incremental** — touch one game source file (`src/battle/ui/hud.rs`), rebuild.
  This is the real edit→compile loop, run hundreds of times a day.
- **`target/` size** — disk footprint after the build.

> Caveat: clean-build times vary ±5–10% run to run (thermals, background load).
> Treat differences under ~10% as noise. The incremental numbers are the ones
> that matter for day-to-day iteration.

## Results

| Config | Clean build | Incremental | `target/` | Toolchain | Breaks stable `cargo`? |
|---|---|---|---|---|---|
| **Baseline** (current `main`) | 336.7 s | **1.94 s** | — | stable | no |
| **`debug = 1`** | 307.6 s | 1.95 s | 9.0 G | stable | no |
| **Nightly: `share-generics` + `threads=0`** | 284.7 s | **~36 s** ⚠️ | 3.4 G | nightly only | **yes** ⚠️ |
| **LLD / Mold linker** | not benchmarked — see below | | | | |
| **`no-embed-metadata`** | not benchmarked — see below | | | | |

## Config-by-config

### Baseline — stable, current settings
The reference. Incremental rebuild is already **~2 s**, which is genuinely fast;
the dependency optimization profiles (`opt-level = 1` dev, `3` for deps) are
already in place. There is very little headroom in the incremental loop to begin
with — most "fast compile" advice targets projects that don't already have these.

### `debug = 1` (line-tables-only debug info) — ✅ recommended
- **Clean build ~9% faster** (336.7 → 307.6 s, ~29 s saved). Comes from less debug
  info to write and link.
- **Incremental unchanged** (~2 s) — already too fast to improve.
- **Stable, zero risk** to the toolchain.
- **Tradeoff:** backtraces keep file/line numbers, but the debugger loses some
  local-variable detail. For a game you mostly run-and-watch (not step-debug),
  this is a fine trade. The Bevy config file specifically calls this out as the
  macOS win.

**Verdict:** worth enabling. Small but free, no downside for this workflow.

### Nightly `-Zshare-generics=y` + `-Zthreads=0` — ❌ not recommended here
The config file bills these as the biggest win, and the **clean build was indeed
fastest** (284.7 s, ~15% under baseline). But measuring the *incremental* loop
exposed two dealbreakers:

1. **Incremental rebuild collapsed to ~36 s — roughly 18× slower than stable's
   ~2 s.** `share-generics` makes crates share monomorphized generic code, so
   touching one file invalidates and re-monomorphizes shared generics across far
   more of the (generic-heavy) Bevy graph. Great for a single cold compile,
   terrible for edit→rebuild.
2. **Uncommenting them breaks plain `cargo` / `just ci`.** The `-Z` flags are
   nightly-only, so every `cargo build`, `cargo test`, `just ci` errors with
   *"the option `Z` is only accepted on the nightly compiler"* unless you prefix
   `+nightly` everywhere or `rustup override set nightly`. That commits the whole
   project to nightly ("experimental, may contain bugs").

The smaller `target/` (3.4 G vs 9.0 G) is real but is mostly a side effect of the
nightly build artifacts, not a reason to switch.

#### Why the build breaks on stable (the mechanism)

The `-Z…` prefix marks a flag as **unstable / nightly-only** — that's the core
of Rust's stability contract. Putting `-Zshare-generics`/`-Zthreads` in
`.cargo/config.toml`'s `rustflags` makes Cargo pass them to *whatever* rustc runs.
On the default `stable` toolchain, rustc rejects any `-Z` flag outright:

```
error: the option `Z` is only accepted on the nightly compiler
```

It is **not** a compile error in your code or in Bevy — it fires before your
crate is even looked at. Cargo runs a tiny target-probe build first
(`rustc --print=file-names …`), that probe inherits the `-Z` flags, and stable
rustc kills it immediately. So the whole toolchain is blocked for *every* command
(`cargo build`, `cargo test`, `just ci`) until you either prefix `+nightly`
everywhere or `rustup override set nightly` for the directory — committing the
project to nightly ("experimental, may contain bugs").

That is exactly why the upstream Bevy config (which this mirrors) ships these
lines commented under a **"Nightly"** heading: they can't be on by default without
forcing every contributor onto nightly. The slowdown (~36 s incremental) is the
*separate* reason not to use them even once you're on nightly — there the flags
are accepted and compile fine, they just widen the per-edit recompile blast
radius.

**Verdict:** net negative for this project. The clean-build win is dwarfed by the
incremental-loop regression and the stable-toolchain breakage. Leave commented.

### LLD / Mold linker — ❌ not applicable on Apple Silicon
Not benchmarked because the config file itself says not to:

> *"The default ld64 linker is faster, you should continue using it instead."*

On macOS, Apple's `ld64` already beats LLD; Mold doesn't support macOS at all.
Enabling these would require `brew install llvm` (not installed) and would slow
builds down or do nothing. These lines are only relevant on Linux/Windows.

**Verdict:** leave commented on macOS. (They're the live default on Linux, where
LLD *is* the win — the config is correct to ship them per-platform.)

### `no-embed-metadata` — ⚠️ nightly-only, negligible
Avoids metadata duplication; the upstream blog claims ~5% smaller `target/` on dev
builds. Requires nightly (same toolchain-lock downside as the `-Z` flags above),
and the time impact is "negligible" per the config's own notes. Not worth pulling
the project onto nightly for a ~5% disk saving.

**Verdict:** skip unless already on nightly for another reason.

## Bottom line

| Want… | Do this |
|---|---|
| **Faster clean builds, zero risk** | Enable `debug = 1` (stable, ~9% off clean build) |
| **Faster incremental loop** | Nothing to do — already ~2 s; the nightly path makes it *worse* |
| **Smallest `target/`** | `no-embed-metadata` (nightly) — but rarely worth the toolchain lock |
| **Linux/Windows builds** | The shipped LLD config already helps there; leave as-is |

The single clearly-worthwhile change on this machine is **`debug = 1`**. Everything
else either doesn't apply to Apple Silicon, regresses the incremental loop, or
forces a nightly toolchain for a marginal gain.

The headline insight: this project's incremental build is *already fast* (~2 s)
because the dependency `opt-level` profiles are in place. The "fast compiles"
config's biggest levers (nightly generics sharing, alternate linkers) target
problems that either don't exist here (macOS linker is already optimal) or trade
the wrong direction (cold build faster, hot loop much slower).
