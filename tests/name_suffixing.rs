//! Coverage for `suffix_duplicate_names`, mirroring the duplicate-name loop in
//! Godot `BattleScene.SpawnEnemies`.

use aliasing::battle::naming::suffix_duplicate_names;

#[test]
fn unique_names_are_left_untouched() {
    assert_eq!(
        suffix_duplicate_names(&["Hero", "Goblin", "Slime"]),
        vec!["Hero", "Goblin", "Slime"],
    );
}

#[test]
fn a_single_occurrence_is_not_suffixed() {
    // One Goblin stays "Goblin" — only collisions get lettered.
    assert_eq!(suffix_duplicate_names(&["Goblin"]), vec!["Goblin"]);
}

#[test]
fn duplicates_are_lettered_in_order_of_appearance() {
    assert_eq!(
        suffix_duplicate_names(&["Goblin", "Goblin"]),
        vec!["Goblin A", "Goblin B"],
    );
}

#[test]
fn three_duplicates_get_a_b_c() {
    assert_eq!(
        suffix_duplicate_names(&["Goblin", "Goblin", "Goblin"]),
        vec!["Goblin A", "Goblin B", "Goblin C"],
    );
}

#[test]
fn interleaved_duplicates_keep_appearance_order() {
    assert_eq!(
        suffix_duplicate_names(&["Goblin", "Slime", "Goblin"]),
        vec!["Goblin A", "Slime", "Goblin B"],
    );
}

#[test]
fn distinct_duplicate_groups_letter_independently() {
    assert_eq!(
        suffix_duplicate_names(&["Goblin", "Slime", "Goblin", "Slime"]),
        vec!["Goblin A", "Slime A", "Goblin B", "Slime B"],
    );
}

#[test]
fn empty_roster_yields_empty() {
    assert!(suffix_duplicate_names(&[]).is_empty());
}
