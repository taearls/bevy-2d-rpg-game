use bevy::asset::AssetMetaCheck;
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
                // We ship no `.meta` files. On the web, trunk's dev server answers
                // unknown paths (like `foo.ron.meta`) with `index.html` and a 200,
                // so Bevy's default meta probe parses HTML as asset meta and every
                // asset load fails. Skip the probe entirely; harmless on native.
                .set(AssetPlugin {
                    meta_check: AssetMetaCheck::Never,
                    ..default()
                })
                .set(ImagePlugin::default_nearest()),
            GamePlugin,
        ))
        .run();
}
