//! Character identity, stats, and data-driven templates.

pub mod asset_loader;
pub mod components;
pub mod definition;

use bevy::prelude::*;

use asset_loader::CharacterDefLoader;
use definition::CharacterDef;

/// Registers the [`CharacterDef`] asset type and its `*.character.ron` loader so
/// the roster can be authored as data assets and hot-reloaded.
pub struct CharactersPlugin;

impl Plugin for CharactersPlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<CharacterDef>()
            .init_asset_loader::<CharacterDefLoader>();
    }
}
