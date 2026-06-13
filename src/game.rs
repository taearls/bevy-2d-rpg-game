use bevy::prelude::*;

use crate::battle::BattlePlugin;
use crate::characters::CharactersPlugin;

/// Root plugin for the battle game: sets the clear color and wires in the
/// character-asset and battle-spawning plugins. Later phases add `BattleUiPlugin`.
pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ClearColor(Color::srgb(0.18, 0.18, 0.24)))
            .add_plugins((CharactersPlugin, BattlePlugin));
    }
}
