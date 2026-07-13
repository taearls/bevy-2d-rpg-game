//! Pure combat math.
//!
//! The damage formula is isolated here so it can be unit-tested without an ECS
//! world. The resolution system that consumes it lives in [`super::resolve`].

/// Compute the damage a single attack deals.
///
/// - `attack` ≤ 0 deals `0` (a disabled/defeated attacker does nothing).
/// - Otherwise the base hit is `max(1, attack - defense)` — armour can never
///   fully negate a blow — scaled by `variance` (a per-character roll, usually
///   in `[0.8, 1.2]`), with the result floored at `1` so every connecting hit
///   chips at least one point of health.
///
/// The scaled damage rounds to the nearest integer (`.round()`) per the Phase 2
/// spec.
#[must_use]
pub fn compute_damage(attack: i32, defense: i32, variance: f32) -> i32 {
    if attack <= 0 {
        return 0;
    }
    let base = (attack - defense).max(1);
    let scaled = (base as f32 * variance).round() as i32;
    scaled.max(1)
}
