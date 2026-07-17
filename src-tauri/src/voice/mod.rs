//! The always-listening voice engine.
//!
//! Phase status against the build plan (§9 of the architecture):
//!
//!   - `capture`     — cpal microphone stream            ✅ Phase 2
//!   - `stt`         — whisper.cpp via whisper-rs         ✅ Phase 5
//!   - `vad`         — Silero VAD via ort                 ⏳ Phase 3
//!   - `wake_word`   — openWakeWord `hey_jarvis` via ort  ⏳ Phase 4
//!
//! Today the "Hey Jarvis" button (or `trigger_wake`) stands in for the wake
//! word: it records a real utterance from the mic and runs it through the real
//! local Whisper model. VAD + wake word make it hands-free next.

pub mod capture;
pub mod state_machine;
pub mod stt;

pub use state_machine::{VoiceEngine, VoiceState};
