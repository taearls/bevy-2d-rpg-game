//! Parity with the Godot `BattleSceneSeedParsingTest`: the pure `parse_seed`
//! helper and reading a seed file from disk.

use aliasing::battle::seed::{parse_seed, read_seed_file_at};

#[test]
fn parses_a_valid_seed() {
    assert_eq!(parse_seed("12345"), Some(12345));
}

#[test]
fn parses_zero() {
    assert_eq!(parse_seed("0"), Some(0));
}

#[test]
fn parses_u64_max() {
    assert_eq!(parse_seed("18446744073709551615"), Some(u64::MAX));
}

#[test]
fn trims_surrounding_whitespace_and_newline() {
    assert_eq!(parse_seed("  42  "), Some(42));
    assert_eq!(parse_seed("42\n"), Some(42));
    assert_eq!(parse_seed("\t100\r\n"), Some(100));
}

#[test]
fn rejects_non_numeric() {
    assert_eq!(parse_seed("abc"), None);
    assert_eq!(parse_seed("12.5"), None);
    assert_eq!(parse_seed("0x1F"), None);
    assert_eq!(parse_seed("-1"), None); // unsigned only
}

#[test]
fn rejects_overflow_beyond_u64() {
    assert_eq!(parse_seed("18446744073709551616"), None); // u64::MAX + 1
}

#[test]
fn rejects_empty_and_whitespace_only() {
    assert_eq!(parse_seed(""), None);
    assert_eq!(parse_seed("   "), None);
    assert_eq!(parse_seed("\n"), None);
}

#[test]
fn reads_seed_from_a_file() {
    let dir = std::env::temp_dir();
    let path = dir.join("bevy_rpg_test_seed_valid.seed");
    std::fs::write(&path, "987654321\n").unwrap();
    assert_eq!(read_seed_file_at(&path), Some(987_654_321));
    let _ = std::fs::remove_file(&path);
}

#[test]
fn missing_file_yields_none() {
    let path = std::env::temp_dir().join("bevy_rpg_test_seed_does_not_exist.seed");
    let _ = std::fs::remove_file(&path);
    assert_eq!(read_seed_file_at(&path), None);
}

#[test]
fn malformed_file_yields_none() {
    let path = std::env::temp_dir().join("bevy_rpg_test_seed_malformed.seed");
    std::fs::write(&path, "not-a-number").unwrap();
    assert_eq!(read_seed_file_at(&path), None);
    let _ = std::fs::remove_file(&path);
}
