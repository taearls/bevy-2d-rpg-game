//! Deterministic RNG resources.
//!
//! Two `ChaCha8Rng`-backed resources: `SpawnRng` (optionally pinned by
//! `battle.seed` so a roster reproduces exactly) and `DamageRng` (entropy-seeded
//! at runtime, fixed-seed in tests so damage variance is assertable).
//! `ChaCha8Rng` is used over `rand`'s default PRNG
//! because it guarantees a stable byte stream across `rand` releases — a pinned
//! seed keeps producing the same battle after a dependency bump.

use bevy::prelude::*;
use rand_chacha::ChaCha8Rng;
use rand_chacha::rand_core::SeedableRng;

/// RNG governing enemy count and roster selection at battle start. Pinned from
/// `battle.seed` when that file is present so spawns are reproducible.
#[derive(Resource, Debug)]
pub struct SpawnRng(pub ChaCha8Rng);

impl SpawnRng {
    /// Seed deterministically from an integer (e.g. a pinned `battle.seed`).
    #[must_use]
    pub fn from_seed(seed: u64) -> Self {
        Self(ChaCha8Rng::seed_from_u64(seed))
    }

    /// Seed from OS entropy for a fresh, non-reproducible battle.
    #[must_use]
    pub fn from_entropy() -> Self {
        Self(ChaCha8Rng::from_os_rng())
    }
}

/// RNG governing per-hit damage variance. Entropy-seeded in normal play;
/// tests seed it explicitly so variance rolls are deterministic.
#[derive(Resource, Debug)]
pub struct DamageRng(pub ChaCha8Rng);

impl DamageRng {
    /// Seed deterministically from an integer (used in tests).
    #[must_use]
    pub fn from_seed(seed: u64) -> Self {
        Self(ChaCha8Rng::seed_from_u64(seed))
    }

    /// Seed from OS entropy for live play.
    #[must_use]
    pub fn from_entropy() -> Self {
        Self(ChaCha8Rng::from_os_rng())
    }
}
