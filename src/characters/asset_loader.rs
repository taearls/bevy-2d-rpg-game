//! RON [`AssetLoader`] for `*.character.ron` character templates.
//!
//! Roster entries live as text assets so they can be edited (and hot-reloaded)
//! without recompiling. The loader reads the whole file and deserializes it into
//! a [`CharacterDef`] with `ron`; every stat field must be present in the asset
//! (no serde defaults).

use bevy::asset::io::Reader;
use bevy::asset::{AssetLoader, LoadContext};
use bevy::prelude::*;
use thiserror::Error;

use super::definition::CharacterDef;

/// File extension (without the leading dot) handled by [`CharacterDefLoader`].
pub const CHARACTER_EXTENSION: &str = "character.ron";

/// Loads a [`CharacterDef`] from a `*.character.ron` file.
#[derive(Default, TypePath)]
pub struct CharacterDefLoader;

/// Errors surfaced while loading a character template.
#[derive(Debug, Error)]
pub enum CharacterDefLoaderError {
    /// The asset bytes could not be read from the source.
    #[error("failed to read character asset: {0}")]
    Io(#[from] std::io::Error),
    /// The bytes were read but were not valid RON for a [`CharacterDef`].
    #[error("failed to parse character RON: {0}")]
    Ron(#[from] ron::error::SpannedError),
}

impl AssetLoader for CharacterDefLoader {
    type Asset = CharacterDef;
    type Settings = ();
    type Error = CharacterDefLoaderError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &Self::Settings,
        _load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        let def = ron::de::from_bytes::<CharacterDef>(&bytes)?;
        Ok(def)
    }

    fn extensions(&self) -> &[&str] {
        &[CHARACTER_EXTENSION]
    }
}
