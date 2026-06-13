use bevy::prelude::*;
use bevy_2d_rpg_game::game::GamePlugin;

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Bevy 2D RPG".into(),
                        resolution: (1152, 648).into(),
                        ..default()
                    }),
                    ..default()
                })
                .set(ImagePlugin::default_nearest()),
            GamePlugin,
        ))
        .run();
}
