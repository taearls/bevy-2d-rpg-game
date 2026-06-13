//! Optional pinned-seed support for deterministic spawning.
//!
//! Mirrors the Godot `battle.seed` mechanism: a local, gitignored file holding
//! a single unsigned integer, written by `just shuffle` and removed by
//! `just unshuffle`. When present and valid, the spawn RNG is seeded from it so
//! the same roster is produced every launch; otherwise spawning rolls fresh
//! entropy. Parity with `BattleScene.TryParseSeed` / `LoadSeededRng`.

use std::fs;
use std::path::Path;

/// File name read from the working directory at launch.
pub const SEED_FILE_PATH: &str = "battle.seed";

/// Parse the contents of a `battle.seed` file into a `u64`.
///
/// Returns `Some` when `raw` (after trimming surrounding whitespace, including
/// a trailing newline) is a valid unsigned 64-bit integer, and `None`
/// otherwise. Mirrors Godot `TryParseSeed`, which trims and parses with
/// invariant-culture integer rules.
#[must_use]
pub fn parse_seed(raw: &str) -> Option<u64> {
    raw.trim().parse::<u64>().ok()
}

/// Read and parse `battle.seed` from the working directory.
///
/// Returns `None` when the file is missing, unreadable, or does not contain a
/// valid unsigned integer — in every such case the caller falls back to a
/// fresh, entropy-seeded RNG (matching the Godot behaviour).
#[must_use]
pub fn read_seed_file() -> Option<u64> {
    read_seed_file_at(SEED_FILE_PATH)
}

/// [`read_seed_file`] against an explicit path; the seam tests use to read a
/// fixture without depending on the process working directory.
#[must_use]
pub fn read_seed_file_at(path: impl AsRef<Path>) -> Option<u64> {
    fs::read_to_string(path)
        .ok()
        .and_then(|raw| parse_seed(&raw))
}
