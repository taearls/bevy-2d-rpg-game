//! Diagnostics overlay — an on-screen FPS / frame-time readout, compiled in only
//! under the `debug-overlay` cargo feature.
//!
//! This wires Bevy's **official** [`FpsOverlayPlugin`] (from `bevy_dev_tools`),
//! which draws an FPS counter — plus a frame-time graph on the native/WebGPU
//! renderer — in the top-left corner. Alongside it we register a small custom
//! [`Diagnostic`] (a monotonically incrementing frame counter) and draw it in a
//! `Text` node just below the FPS readout. Press **F12** to toggle both on and off.
//!
//! It replaces the previous `bevy_egui` / `bevy-inspector-egui` community
//! inspector, which had no Bevy 0.19-compatible release. The official overlay
//! ships inside Bevy itself, so the migration drops the third-party egui
//! dependency entirely while keeping a zero-cost-when-off dev tool: the whole
//! module is `#[cfg(feature = "debug-overlay")]` at the call site in
//! [`GamePlugin`](crate::game::GamePlugin), so a default `cargo build` compiles
//! `bevy_dev_tools` out — tests and release/wasm bundles never pull it in.

use bevy::dev_tools::fps_overlay::{FPS_OVERLAY_ZINDEX, FpsOverlayConfig, FpsOverlayPlugin};
use bevy::diagnostic::{
    Diagnostic, DiagnosticPath, Diagnostics, DiagnosticsStore, RegisterDiagnostic,
};
use bevy::prelude::*;
use bevy::render::RenderApp;

/// Path of the custom frame-counter diagnostic registered by this module.
const FRAME_COUNTER: DiagnosticPath = DiagnosticPath::const_new("debug/frame_counter");

/// Marker for the `Text` node that renders our custom frame counter, so the
/// toggle system can find it and flip its visibility in lockstep with the FPS
/// overlay.
#[derive(Component)]
struct FrameCounterText;

/// Adds the official FPS / frame-time diagnostics overlay plus a custom
/// frame-counter readout, toggled together with F12.
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
        .register_diagnostic(Diagnostic::new(FRAME_COUNTER))
        .add_systems(Startup, spawn_frame_counter_text)
        .add_systems(Update, (tick_frame_counter, update_frame_counter_text))
        .add_systems(Update, toggle_overlay);
}

/// Spawn the text node that displays the custom frame counter, positioned just
/// below Bevy's FPS overlay (which sits flush in the top-left corner). It starts
/// hidden or visible to match the overlay's initial `enabled` state.
fn spawn_frame_counter_text(mut commands: Commands, config: Res<FpsOverlayConfig>) {
    commands.spawn((
        FrameCounterText,
        Text::new("frames: 0"),
        config.text_config.clone(),
        TextColor(config.text_color),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(40.0),
            left: Val::ZERO,
            ..default()
        },
        GlobalZIndex(FPS_OVERLAY_ZINDEX),
        if config.enabled {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        },
    ));
}

/// Increment the custom frame-counter diagnostic once per frame.
fn tick_frame_counter(mut diagnostics: Diagnostics, mut count: Local<f64>) {
    *count += 1.0;
    diagnostics.add_measurement(&FRAME_COUNTER, || *count);
}

/// Mirror the latest frame-counter measurement into the on-screen text.
fn update_frame_counter_text(
    store: Res<DiagnosticsStore>,
    mut text: Single<&mut Text, With<FrameCounterText>>,
) {
    if let Some(value) = store
        .get(&FRAME_COUNTER)
        .and_then(Diagnostic::measurement)
        .map(|m| m.value)
    {
        ***text = format!("frames: {value:.0}");
    }
}

/// Toggle the diagnostics overlay on and off with F12. The overlay starts
/// enabled (a `--features debug-overlay` build is asking to see diagnostics);
/// F12 flips [`FpsOverlayConfig::enabled`], which the plugin watches to show or
/// hide the FPS readout, and we mirror that flag onto our custom counter's
/// visibility so the two toggle together.
fn toggle_overlay(
    keys: Res<ButtonInput<KeyCode>>,
    mut config: ResMut<FpsOverlayConfig>,
    mut counter: Single<&mut Visibility, With<FrameCounterText>>,
) {
    if keys.just_pressed(KeyCode::F12) {
        config.enabled = !config.enabled;
        **counter = if config.enabled {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
}
