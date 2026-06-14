//! Debug inspector — the Bevy replacement for the Godot custom F12 inspector
//! addon, compiled in only under the `debug-inspector` cargo feature.
//!
//! Wires `bevy_egui`'s `EguiPlugin` and `bevy-inspector-egui`'s
//! `WorldInspectorPlugin` (the Bevy 0.18 line), then gates the inspector window
//! behind an F12-toggled [`InspectorEnabled`] resource via `run_if`. Every former
//! Godot `[Export(Range)]` tuning knob (`BattleLayout`, `UiConfig`,
//! `DamageVariance`, `Health`, `CombatStats`, …) is registered for reflection by
//! its owning plugin, so toggling the inspector and editing those values
//! reposition enemies, resize panels, and retune combat live.
//!
//! The whole module is `#[cfg(feature = "debug-inspector")]` at the call site in
//! [`GamePlugin`](crate::game::GamePlugin), so a default `cargo build` compiles
//! egui out entirely — tests and release binaries never pull it in.

use bevy::prelude::*;
use bevy::render::RenderApp;
use bevy_inspector_egui::bevy_egui::EguiPlugin;
use bevy_inspector_egui::quick::WorldInspectorPlugin;

/// Whether the world inspector window is currently shown. Toggled with F12,
/// matching the Godot inspector addon's hotkey. Starts hidden so the game opens
/// to a clean view; press F12 to reveal the live tuning panel.
#[derive(Resource, Debug, Default)]
pub struct InspectorEnabled(pub bool);

/// Adds the egui-backed world inspector behind an F12 toggle.
///
/// Only added to the app under the `debug-inspector` feature; see the module
/// docs. Pulls in `EguiPlugin` (its render/context machinery) and the
/// `WorldInspectorPlugin`, the latter gated on [`InspectorEnabled`] so the panel
/// is hidden until the player presses F12.
///
/// `EguiPlugin` requires the renderer (it registers shader assets into the
/// `RenderApp` sub-app), so when no `RenderApp` is present — a headless app built
/// on `MinimalPlugins`, as the smoke test does — this plugin is a no-op rather
/// than panicking. That is the correct behaviour anyway: with no window there is
/// nowhere to draw the inspector, so `cargo test --all-features` stays green.
pub struct DebugPlugin;

impl Plugin for DebugPlugin {
    fn build(&self, app: &mut App) {
        if app.get_sub_app(RenderApp).is_none() {
            return;
        }

        app.init_resource::<InspectorEnabled>()
            .add_plugins(EguiPlugin::default())
            .add_plugins(
                WorldInspectorPlugin::new().run_if(|enabled: Res<InspectorEnabled>| enabled.0),
            )
            .add_systems(Update, toggle_inspector);
    }
}

/// Flip [`InspectorEnabled`] each time F12 is pressed.
fn toggle_inspector(keys: Res<ButtonInput<KeyCode>>, mut enabled: ResMut<InspectorEnabled>) {
    if keys.just_pressed(KeyCode::F12) {
        enabled.0 = !enabled.0;
    }
}
