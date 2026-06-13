# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed

- 2026-06-13: Enabled `clippy::pedantic` with documented Bevy-friendly allow-backs in `Cargo.toml`; `just ci` green with no source-level violations (#10)

### Added

- 2026-06-13: Character RON assets + battle spawning — `CharacterDef` as a loadable `Asset` with a `*.character.ron` `AssetLoader`, `hero`/`goblin` templates, and a seeded spawn that rolls 1–4 enemies, suffixes duplicate names, and lays out the player + enemy row from `BattleLayout` with a `Camera2d`; headless spawn tests mirror the GdUnit4 `BattleSceneTest` coverage (#3)
- 2026-06-13: Core domain logic — character components/definitions (serde defaults 100/10/5), `parse_seed`/`read_seed_file`, `SpawnRng`/`DamageRng` (`ChaCha8Rng`), pure `compute_damage` (rounds where Godot truncated), and `suffix_duplicate_names`, with unit tests mirroring the GdUnit4 coverage (#2)
- 2026-06-12: Project scaffold — Bevy 0.18.1 Cargo project, 1152×648 game window, assets ported from the Godot repo (incl. Lyuba CC-BY 3.0 attribution), justfile quality gates, and a headless smoke test (#1)
- 2026-06-13: GitHub Actions CI workflow running the `just ci` quality gate on PRs, MIT `LICENSE` file, edition 2024, and a README run/develop guide (#1 review)
