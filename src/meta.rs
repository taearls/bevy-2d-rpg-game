//! Project-level metadata derived at compile time from the crate manifest.

use bevy::{asset::Asset, reflect::TypePath};

use crate::util::capitalize_first;

/// Display metadata about the project itself (e.g. the window title), sourced
/// from Cargo so it stays in sync with `Cargo.toml` without a separate constant.
#[derive(Asset, TypePath, Debug)]
pub struct Meta {
    /// Human-facing project name: the `CARGO_PKG_NAME` with its first letter
    /// capitalized (see [`capitalize_first`](crate::util::capitalize_first)).
    pub project_name: String,
}

impl Default for Meta {
    /// Build the metadata from the compile-time `CARGO_PKG_NAME`.
    fn default() -> Self {
        let project_name = env!("CARGO_PKG_NAME");
        let formatted_name = capitalize_first(project_name);

        Meta {
            project_name: formatted_name,
        }
    }
}
