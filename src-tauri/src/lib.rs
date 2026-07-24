//! Jarvis desktop assistant — Rust core.
//!
//! Owns the voice engine (state machine + event bridge) and exposes a small set
//! of `#[tauri::command]`s the webview calls. The webview only ever renders; it
//! listens for `voice-state-changed` / `voice-audio-level` events and reacts.

mod agent_settings;
mod dispatcher;
mod openclaw;
mod voice;

use agent_settings::AgentSettings;
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

/// Submit a typed message through the same agent dispatch path as speech.
#[tauri::command]
fn submit_text(
    app: tauri::AppHandle,
    engine: tauri::State<'_, VoiceEngine>,
    text: String,
) -> Result<(), String> {
    engine.submit_text(&app, text)
}

/// Report the current voice-engine state (handy for the UI on first mount).
#[tauri::command]
fn current_state(engine: tauri::State<'_, VoiceEngine>) -> voice::VoiceState {
    engine.state()
}

/// Top-bar toggle — while on, every turn (double-clap, spacebar, or the Talk
/// button) echoes the transcript back as text + speech instead of dispatching
/// to the agent backend.
#[tauri::command]
fn set_test_mode(engine: tauri::State<'_, VoiceEngine>, enabled: bool) {
    engine.set_test_mode(enabled);
}

/// Current agent gateway URL/key/model (Settings panel, on open).
#[tauri::command]
fn get_agent_settings(engine: tauri::State<'_, VoiceEngine>) -> AgentSettings {
    engine.agent_settings()
}

/// Save new agent gateway settings — applies immediately and persists to disk
/// so other backends (not just the Kimi/Moonshot default) can be wired in
/// without rebuilding the app.
#[tauri::command]
fn set_agent_settings(
    app: tauri::AppHandle,
    engine: tauri::State<'_, VoiceEngine>,
    settings: AgentSettings,
) -> Result<(), String> {
    engine.set_agent_settings(settings.clone());
    settings.save(&app)
}

/// Live OpenClaw-on-WSL gateway URL (localhost relay when possible, else eth0).
#[tauri::command]
fn resolve_openclaw_wsl_url() -> String {
    agent_settings::openclaw_wsl_url()
}

/// Local Piper neural TTS → base64 WAV. Errors if the voice isn't ready yet
/// (front end falls back to browser speechSynthesis).
#[tauri::command]
fn synthesize_speech(
    engine: tauri::State<'_, VoiceEngine>,
    text: String,
) -> Result<String, String> {
    engine.synthesize_speech(&text)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(VoiceEngine::new())
        .setup(|app| {
            let engine = app.state::<VoiceEngine>().inner().clone();
            engine.set_agent_settings(AgentSettings::load(app.handle()));
            // Load the local Whisper model in the background.
            engine.load_model(app.handle().clone());
            // Download/load local Piper neural TTS in the background.
            engine.load_tts(app.handle().clone());
            // Open the mic and start listening for a double-clap.
            engine.spawn_listener(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            trigger_wake,
            set_test_mode,
            get_agent_settings,
            set_agent_settings,
            resolve_openclaw_wsl_url,
            synthesize_speech,
            push_to_talk_start,
            push_to_talk_stop,
            submit_text,
            current_state
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
