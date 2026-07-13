//! Pure roster name disambiguation.
//!
//! When the spawn RNG rolls the same enemy archetype more than once, the
//! duplicates are lettered ("Goblin A", "Goblin B", …) so the player and battle
//! log can tell them apart. Names that occur exactly once are left untouched.

use std::collections::HashMap;

/// Disambiguate duplicate display names by appending " A", " B", … in order of
/// appearance. Names appearing exactly once are returned unchanged.
///
/// ```
/// # use aliasing::battle::naming::suffix_duplicate_names;
/// assert_eq!(
///     suffix_duplicate_names(&["Goblin", "Slime", "Goblin"]),
///     vec!["Goblin A", "Slime", "Goblin B"],
/// );
/// ```
#[must_use]
pub fn suffix_duplicate_names(names: &[&str]) -> Vec<String> {
    let mut totals: HashMap<&str, usize> = HashMap::new();
    for &name in names {
        *totals.entry(name).or_insert(0) += 1;
    }

    let mut seen: HashMap<&str, usize> = HashMap::new();
    names
        .iter()
        .map(|&name| {
            if totals.get(name).copied().unwrap_or(0) > 1 {
                let index = seen.entry(name).or_insert(0);
                // 26 distinct duplicates is far beyond any real roster (max 4
                // enemies), so a single ASCII letter always suffices.
                let letter = (b'A' + u8::try_from(*index).unwrap_or(u8::MAX)) as char;
                *index += 1;
                format!("{name} {letter}")
            } else {
                name.to_string()
            }
        })
        .collect()
}
