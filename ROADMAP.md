# Roadmap

Godot 4.6 C# turn-based RPG → Bevy 0.18.1 port. One GitHub issue per phase,
sequential (each blocked by the prior). Full design context in
[PORT_PLAN.md](PORT_PLAN.md).

## Open Issues Summary

| Issue | Title | Priority | Effort | Status |
|-------|-------|----------|--------|--------|
| [#1](../../issues/1) | Phase 1: Project scaffold, toolchain, and assets | 🔴 Critical | ~0.5 day | ✅ Done |
| [#2](../../issues/2) | Phase 2: Core domain logic (damage formula, seed parsing, RNG, name suffixing) | 🔴 Critical | ~0.5 day | Open |
| [#3](../../issues/3) | Phase 3: Character RON assets + battle spawning | 🟡 High | ~1 day | Open |
| [#4](../../issues/4) | Phase 4: Turn states + action menu | 🟡 High | ~1 day | Open |
| [#5](../../issues/5) | Phase 5: Targeting, player attack, and victory | 🟡 High | ~1 day | Open |
| [#6](../../issues/6) | Phase 6: Enemy turn, Defend resolution, and game over | 🟡 High | ~1 day | Open |
| [#7](../../issues/7) | Phase 7: HUD + battle log UI parity | 🟢 Medium | ~1 day | Open |
| [#8](../../issues/8) | Phase 8: Debug inspector (bevy-inspector-egui) + polish | 🟢 Medium | ~0.5 day | Open |

## Tooling & Quality

Orthogonal to the numbered port phases — improves maintainability and tooling,
does not block gameplay parity.

| Issue | Title | Priority | Effort | Status |
|-------|-------|----------|--------|--------|
| [#10](../../issues/10) | Tighten clippy configuration (pedantic) and resolve violations | 🟢 Medium | ~0.5 day | Open |

## Current Sprint

**Next up:** [#2 — Phase 2: Core domain logic](../../issues/2) (🔴 critical, unblocked now that #1 is done)

### Recently Completed

- ✅ [#1 — Phase 1: Project scaffold, toolchain, and assets](../../issues/1) — Cargo
  scaffold (Bevy 0.18.1), 1152×648 window, assets ported from the Godot repo,
  justfile quality gates (`just ci`), headless smoke test.

## Implementation Order

Strictly sequential: #2 → #3 → #4 → #5 → #6 → #7 → #8. Each phase ships
independently with `just ci` green.

## Issue Status Summary

- **Port phases:** 8 total — 1 done (#1), 7 open (#2–#8); critical remaining: 1 (#2)
- **Tooling & quality:** 1 open (#10)

## Changelog

- **2026-06-13** — Added tooling task #10 (tighten clippy config) to the roadmap.
- **2026-06-12** — Roadmap created; #1 (Phase 1 scaffold) completed.
