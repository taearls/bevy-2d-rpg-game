use aliasing::game::GamePlugin;
use aliasing::meta::Meta;
use bevy::asset::AssetMetaCheck;
use bevy::prelude::*;

fn main() {
    let meta = Meta::default();
    App::new()
        .add_plugins((
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: meta.project_name,
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
