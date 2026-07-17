//! Jarvis desktop assistant — Rust core.
//!
//! Owns the voice engine (state machine + event bridge) and exposes a small set
//! of `#[tauri::command]`s the webview calls. The webview only ever renders; it
//! listens for `voice-state-changed` / `voice-audio-level` events and reacts.

mod dispatcher;
mod voice;

use tauri::Manager;
use voice::VoiceEngine;

/// Manually start a turn (the "Talk" button). Auto-ends on trailing silence.
#[tauri::command]
fn trigger_wake(engine: tauri::State<'_, VoiceEngine>) {
    engine.request_listen();
}

/// Spacebar pressed — begin a push-to-talk turn (records until released).
#[tauri::command]
fn push_to_talk_start(engine: tauri::State<'_, VoiceEngine>) {
    engine.start_hold();
}

/// Spacebar released — end the push-to-talk turn and send it.
#[tauri::command]
fn push_to_talk_stop(engine: tauri::State<'_, VoiceEngine>) {
    engine.stop_hold();
}

/// Report the current voice-engine state (handy for the UI on first mount).
#[tauri::command]
fn current_state(engine: tauri::State<'_, VoiceEngine>) -> voice::VoiceState {
    engine.state()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(VoiceEngine::new())
        .setup(|app| {
            let engine = app.state::<VoiceEngine>().inner().clone();
            // Load the local Whisper model in the background.
            engine.load_model(app.handle().clone());
            // Open the mic and start listening for a double-clap.
            engine.spawn_listener(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            trigger_wake,
            push_to_talk_start,
            push_to_talk_stop,
            current_state
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
