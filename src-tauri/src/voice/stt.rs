//! Speech-to-text via whisper.cpp (whisper-rs bindings).
//!
//! This is the "voice → text" stage, and it is deliberately **not** an LLM: it
//! is a local, offline Whisper model (`ggml-base.en.bin`) running natively in
//! this process. The model file is a portable binary blob — the same file works
//! on macOS and Windows; only this binary is rebuilt per platform (§5.5).

use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

pub struct Transcriber {
    ctx: WhisperContext,
}

impl Transcriber {
    /// Load a GGML Whisper model from disk. Takes ~0.5s for `base.en`.
    pub fn load(model_path: &str) -> Result<Self, String> {
        let ctx = WhisperContext::new_with_params(model_path, WhisperContextParameters::default())
            .map_err(|e| format!("failed to load whisper model: {e}"))?;
        Ok(Self { ctx })
    }

    /// Transcribe 16 kHz mono f32 audio to text.
    pub fn transcribe(&self, audio_16k_mono: &[f32]) -> Result<String, String> {
        if audio_16k_mono.is_empty() {
            return Err("no audio captured".into());
        }
        let mut state = self.ctx.create_state().map_err(|e| e.to_string())?;

        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        let threads = std::thread::available_parallelism()
            .map(|n| n.get() as i32)
            .unwrap_or(4);
        params.set_n_threads(threads);
        params.set_language(Some("en"));
        params.set_translate(false);
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);

        state
            .full(params, audio_16k_mono)
            .map_err(|e| format!("transcription failed: {e}"))?;

        let n = state.full_n_segments(); // whisper-rs 0.16: returns i32
        let mut text = String::new();
        for i in 0..n {
            if let Some(seg) = state.get_segment(i) {
                if let Ok(s) = seg.to_str_lossy() {
                    text.push_str(&s);
                }
            }
        }
        Ok(text.trim().to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verifies the local Whisper model actually transcribes speech. Generate a
    /// WAV first (e.g. macOS `say -o /tmp/t.wav --data-format=LEF32@16000 "..."`)
    /// and point `JARVIS_TEST_WAV` at it:
    ///   JARVIS_TEST_WAV=/tmp/t.wav cargo test transcribes -- --nocapture
    #[test]
    fn transcribes_generated_speech() {
        let model = "models/ggml-base.en.bin";
        if !std::path::Path::new(model).exists() {
            eprintln!("model missing at {model}, skipping");
            return;
        }
        let wav = match std::env::var("JARVIS_TEST_WAV") {
            Ok(w) => w,
            Err(_) => {
                eprintln!("JARVIS_TEST_WAV not set, skipping");
                return;
            }
        };

        let mut reader = hound::WavReader::open(&wav).expect("open wav");
        let spec = reader.spec();
        let samples: Vec<f32> = match spec.sample_format {
            hound::SampleFormat::Float => reader.samples::<f32>().map(|s| s.unwrap()).collect(),
            hound::SampleFormat::Int => {
                let max = (1i64 << (spec.bits_per_sample - 1)) as f32;
                reader
                    .samples::<i32>()
                    .map(|s| s.unwrap() as f32 / max)
                    .collect()
            }
        };
        assert_eq!(spec.sample_rate, 16_000, "test wav must be 16 kHz mono");

        let t = Transcriber::load(model).expect("load model");
        let text = t.transcribe(&samples).expect("transcribe");
        eprintln!("STT RESULT: {text:?}");
        assert!(!text.trim().is_empty(), "expected non-empty transcription");
    }
}
