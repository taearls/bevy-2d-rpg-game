use bevy::{asset::Asset, reflect::TypePath};

use crate::util::capitalize_first;

#[derive(Asset, TypePath, Debug)]
pub struct Meta {
    pub project_name: String,
}

impl Default for Meta {
    fn default() -> Self {
        let project_name = env!("CARGO_PKG_NAME");
        let formatted_name = capitalize_first(project_name);

        Meta {
            project_name: formatted_name,
        }
    }
}
