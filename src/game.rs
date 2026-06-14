use bevy::prelude::*;

use crate::battle::BattlePlugin;
use crate::characters::CharactersPlugin;

/// Root plugin for the battle game: sets the clear color and wires in the
/// character-asset and battle plugins (the latter pulls in the HUD/log UI).
pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ClearColor(Color::srgb(0.18, 0.18, 0.24)))
            .add_plugins((CharactersPlugin, BattlePlugin));

        // The egui debug inspector (right-click a sprite to inspect it) is
        // compiled in only under the `debug-inspector` feature, so default/release
        // builds and headless tests never link egui.
        #[cfg(feature = "debug-inspector")]
        app.add_plugins(crate::debug::DebugPlugin);
    }
}
