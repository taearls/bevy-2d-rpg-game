//! Diagnostics overlay — an on-screen FPS / frame-time readout, compiled in only
//! under the `debug-overlay` cargo feature.
//!
//! This wires Bevy's **official** [`FpsOverlayPlugin`] (from `bevy_dev_tools`),
//! which draws an FPS counter — plus a frame-time graph on the native/WebGPU
//! renderer — in the top-left corner. Press **F12** to toggle it on and off.
//!
//! It replaces the previous `bevy_egui` / `bevy-inspector-egui` community
//! inspector, which had no Bevy 0.19-compatible release. The official overlay
//! ships inside Bevy itself, so the migration drops the third-party egui
//! dependency entirely while keeping a zero-cost-when-off dev tool: the whole
//! module is `#[cfg(feature = "debug-overlay")]` at the call site in
//! [`GamePlugin`](crate::game::GamePlugin), so a default `cargo build` compiles
//! `bevy_dev_tools` out — tests and release/wasm bundles never pull it in.

use bevy::dev_tools::fps_overlay::{FpsOverlayConfig, FpsOverlayPlugin};
use bevy::prelude::*;
use bevy::render::RenderApp;

/// Adds the official FPS / frame-time diagnostics overlay, toggled with F12.
///
/// `FpsOverlayPlugin` pulls in a UI material + shader for its frame-time graph,
/// whose setup reaches for render-only assets that live behind the renderer's
/// `RenderApp` sub-app. A headless app built on `MinimalPlugins` (the smoke test)
/// has no `RenderApp`, so this plugin is a no-op there rather than panicking —
/// which is correct anyway, since there is no window to draw the overlay on.
///
/// `pub` (unlike the `pub(crate)` sibling feature plugins) so the headless no-op
/// integration test in `tests/smoke.rs` can reference it across the crate
/// boundary.
pub fn plugin(app: &mut App) {
    if app.get_sub_app(RenderApp).is_none() {
        return;
    }

    app.add_plugins(FpsOverlayPlugin::default())
        .add_systems(Update, toggle_overlay);
}

/// Toggle the diagnostics overlay on and off with F12. The overlay starts
/// enabled (a `--features debug-overlay` build is asking to see diagnostics);
/// F12 flips [`FpsOverlayConfig::enabled`], which the plugin watches to show or
/// hide the readout.
fn toggle_overlay(keys: Res<ButtonInput<KeyCode>>, mut config: ResMut<FpsOverlayConfig>) {
    if keys.just_pressed(KeyCode::F12) {
        config.enabled = !config.enabled;
    }
}
