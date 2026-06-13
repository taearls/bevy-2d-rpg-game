//! Battle log messaging.
//!
//! Replaces the Godot `BattleEvents.LogMessage` signal with a frame-buffered
//! Bevy [`Message`]. Any system can `MessageWriter::write` a [`LogMessage`];
//! readers drain it each frame. Phase 4 renders messages to tracing/stdout;
//! Phase 7 adds the on-screen battle-log panel reading the same stream.

use bevy::prelude::*;

/// A single line destined for the battle log, e.g. `"Hero is defending!"`.
///
/// Frame-buffered: written by action handlers and combat resolution, drained by
/// the log renderer. Carrying an owned `String` keeps producers decoupled from
/// however the log chooses to format or store the text.
#[derive(Message, Debug, Clone, PartialEq, Eq)]
pub struct LogMessage(pub String);

impl LogMessage {
    /// Convenience constructor accepting anything string-like.
    pub fn new(text: impl Into<String>) -> Self {
        Self(text.into())
    }
}

/// Drain the [`LogMessage`] stream to tracing each frame. Phase 7 replaces this
/// with (or runs it alongside) the on-screen battle-log panel.
pub fn render_log_messages(mut messages: MessageReader<LogMessage>) {
    for LogMessage(text) in messages.read() {
        info!("{text}");
    }
}
