use bevy::prelude::*;

/// Root plugin for the battle game. Phase 1 only sets the clear color;
/// later phases add `CharactersPlugin`, `BattlePlugin`, and `BattleUiPlugin` here.
pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ClearColor(Color::srgb(0.18, 0.18, 0.24)));
    }
}
