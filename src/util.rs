//! Small, dependency-free helpers shared across the crate.

/// Return `str` with its first character ASCII-uppercased, leaving the rest
/// untouched. An empty string is returned unchanged. Non-ASCII leading
/// characters (which have no ASCII-uppercase form) are also returned as-is.
#[must_use]
pub fn capitalize_first(str: &str) -> String {
    match str.chars().next() {
        Some(c) => format!(
            "{}{}",
            c.to_ascii_uppercase(),
            str.chars().skip(1).collect::<String>()
        ),
        None => str.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::capitalize_first;

    #[test]
    fn empty_string_is_unchanged() {
        assert_eq!(capitalize_first(""), "");
    }

    #[test]
    fn single_char_is_uppercased() {
        assert_eq!(capitalize_first("a"), "A");
    }

    #[test]
    fn only_the_first_char_changes() {
        assert_eq!(capitalize_first("hello world"), "Hello world");
    }

    #[test]
    fn already_capitalized_is_unchanged() {
        assert_eq!(capitalize_first("Hello"), "Hello");
    }

    #[test]
    fn non_ascii_leading_char_is_left_as_is() {
        // `é` has no ASCII-uppercase form, so it passes through unchanged.
        assert_eq!(capitalize_first("élan"), "élan");
    }
}
