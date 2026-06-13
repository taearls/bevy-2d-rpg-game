# Roadmap

Godot 4.6 C# turn-based RPG → Bevy 0.18.1 port. One GitHub issue per phase,
sequential (each blocked by the prior). Full design context in
[PORT_PLAN.md](PORT_PLAN.md).

## Open Issues Summary

| Issue | Title | Priority | Effort | Status |
|-------|-------|----------|--------|--------|
| [#1](../../issues/1) | Phase 1: Project scaffold, toolchain, and assets | 🔴 Critical | ~0.5 day | ✅ Done |
| [#2](../../issues/2) | Phase 2: Core domain logic (damage formula, seed parsing, RNG, name suffixing) | 🔴 Critical | ~0.5 day | ✅ Done |
| [#3](../../issues/3) | Phase 3: Character RON assets + battle spawning | 🟡 High | ~1 day | Open |
| [#4](../../issues/4) | Phase 4: Turn states + action menu | 🟡 High | ~1 day | Open |
| [#5](../../issues/5) | Phase 5: Targeting, player attack, and victory | 🟡 High | ~1 day | Open |
| [#6](../../issues/6) | Phase 6: Enemy turn, Defend resolution, and game over | 🟡 High | ~1 day | Open |
| [#7](../../issues/7) | Phase 7: HUD + battle log UI parity | 🟢 Medium | ~1 day | Open |
| [#8](../../issues/8) | Phase 8: Debug inspector (bevy-inspector-egui) + polish | 🟢 Medium | ~0.5 day | Open |

## Current Sprint

**Next up:** [#3 — Phase 3: Character RON assets + battle spawning](../../issues/3) (🟡 high, unblocked now that #2 is done)

### Recently Completed

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

- **Port phases:** 8 total — 2 done (#1, #2), 6 open (#3–#8); critical remaining: 0
- **Tooling & quality:** 1 total — 1 done (#10); all complete

## Changelog

- **2026-06-13** — Completed Phase 2 (#2): core domain logic — damage formula, seed parsing, RNG, name suffixing.
- **2026-06-13** — Completed tooling task #10 (tightened clippy config to pedantic).
- **2026-06-13** — Added tooling task #10 (tighten clippy config) to the roadmap.
- **2026-06-12** — Roadmap created; #1 (Phase 1 scaffold) completed.
