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
