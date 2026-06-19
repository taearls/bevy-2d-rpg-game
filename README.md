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

## Play in the browser

The latest `main` is built to WebAssembly and deployed to **Cloudflare Pages**
on every push. The live site is **gated by Cloudflare Access (Zero Trust)** with
email login restricted to an allow-list, so only approved addresses can reach
it. Access is enforced at Cloudflare's edge in front of the site — visitors are
sent to Cloudflare's login page, receive a one-time code by email, and are only
let through if their address is on the policy. No game files are served to
unauthenticated visitors.

Live at **`https://bevy-2d-rpg-game.pages.dev`** — unauthenticated visitors are
redirected to Cloudflare's login instead of the game.

> **Deployment configuration** (already set up — kept here for reference and
> for managing the allow-list):
>
> 1. **Pages project** — a Cloudflare Pages project (Direct Upload / Wrangler)
>    named `bevy-2d-rpg-game` (matching [`wrangler.toml`](wrangler.toml)).
> 2. **GitHub secrets** (**Settings → Secrets and variables → Actions**):
>    `CLOUDFLARE_API_TOKEN` (a token with the *Cloudflare Pages: Edit*
>    permission) and `CLOUDFLARE_ACCOUNT_ID`. The
>    [`deploy-cloudflare`](.github/workflows/deploy-cloudflare.yml) workflow
>    builds and redeploys on each push to `main`.
> 3. **Cloudflare Access policy** — in the Cloudflare **Zero Trust** dashboard:
>    - **Settings → Authentication → Login methods** has **One-time PIN**
>      enabled (emails a code; no identity provider needed).
>    - **Access → Applications** has a **Self-hosted** application whose domain
>      is `bevy-2d-rpg-game.pages.dev`.
>    - Its policy is **Action: Allow**, **Include → Emails →** the allow-listed
>      address(es). Access denies everyone not matched. **To share access add an
>      email to that policy; to revoke, remove one.**
>
> This is real per-user authentication: access is tied to an email you control
> and can be revoked at any time by editing the policy.

To build or serve the web version locally you need the wasm target and
[`trunk`](https://trunkrs.dev):

```sh
rustup target add wasm32-unknown-unknown
cargo install trunk

just run-web        # serve at http://127.0.0.1:8080 with hot reload
just build-web      # produce an optimized bundle in ./dist
```

The browser build uses Bevy's WebGL2 backend and routes RNG entropy through the
browser's `crypto.getRandomValues` (see the `wasm32` config in `Cargo.toml` and
`.cargo/config.toml`). The optional `battle.seed` pinning is desktop-only — the
web build has no local filesystem, so it always rolls fresh entropy.

The bundle is built with a dedicated size-optimized `wasm-release` cargo profile
(`opt-level = "s"`, fat LTO, one codegen unit, `panic = "abort"`) plus
`wasm-opt -Oz` (Trunk's wasm-bindgen step strips the debug and name sections),
so only the native desktop `release` profile keeps `opt-level = 3` for runtime
speed.

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
