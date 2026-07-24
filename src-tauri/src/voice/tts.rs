//! Local neural text-to-speech via [Piper](https://github.com/rhasspy/piper).
//!
//! Runs entirely offline (ONNX + espeak phonemizer). The English voice model is
//! downloaded once into the app data / `models/` folder on first use.

use base64::{engine::general_purpose::STANDARD as B64, Engine};
use piper_rs::Piper;
use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager, path::BaseDirectory};

/// Natural US-English male voice (medium quality, ~63 MB).
const VOICE_ID: &str = "en_US-ryan-medium";
const ONNX_URL: &str = "https://huggingface.co/rhasspy/piper-voices/resolve/main/en/en_US/ryan/medium/en_US-ryan-medium.onnx";
const CONFIG_URL: &str = "https://huggingface.co/rhasspy/piper-voices/resolve/main/en/en_US/ryan/medium/en_US-ryan-medium.onnx.json";

pub struct PiperTts {
    piper: Piper,
}

impl PiperTts {
    pub fn load(onnx: &Path, config: &Path) -> Result<Self, String> {
        let piper = Piper::new(onnx, config).map_err(|e| format!("piper load: {e}"))?;
        Ok(Self { piper })
    }

    /// Synthesize `text` to a base64-encoded WAV (16-bit mono PCM).
    pub fn synthesize_wav_base64(&mut self, text: &str) -> Result<String, String> {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return Err("empty text".into());
        }
        // Slightly slower than default reads more naturally for assistant replies.
        let (samples, sample_rate) = self
            .piper
            .create(trimmed, false, None, Some(1.05), None, None)
            .map_err(|e| format!("piper synthesize: {e}"))?;
        let wav = pcm_f32_to_wav(&samples, sample_rate)?;
        Ok(B64.encode(wav))
    }
}

fn pcm_f32_to_wav(samples: &[f32], sample_rate: u32) -> Result<Vec<u8>, String> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut cursor = Cursor::new(Vec::with_capacity(samples.len() * 2 + 44));
    {
        let mut writer = hound::WavWriter::new(&mut cursor, spec)
            .map_err(|e| format!("wav writer: {e}"))?;
        for &s in samples {
            let clipped = (s * i16::MAX as f32).clamp(i16::MIN as f32, i16::MAX as f32) as i16;
            writer
                .write_sample(clipped)
                .map_err(|e| format!("wav sample: {e}"))?;
        }
        writer.finalize().map_err(|e| format!("wav finalize: {e}"))?;
    }
    Ok(cursor.into_inner())
}

/// Resolve (and if needed download) the Piper voice files.
pub fn ensure_voice_files(app: &AppHandle) -> Result<(PathBuf, PathBuf), String> {
    let dir = voice_dir(app)?;
    fs::create_dir_all(&dir).map_err(|e| format!("create models dir: {e}"))?;

    let onnx = dir.join(format!("{VOICE_ID}.onnx"));
    let config = dir.join(format!("{VOICE_ID}.onnx.json"));

    if !onnx.exists() {
        eprintln!("[tts] downloading {VOICE_ID}.onnx …");
        download_file(ONNX_URL, &onnx)?;
        eprintln!("[tts] downloaded {}", onnx.display());
    }
    if !config.exists() {
        eprintln!("[tts] downloading {VOICE_ID}.onnx.json …");
        download_file(CONFIG_URL, &config)?;
    }

    Ok((onnx, config))
}

fn voice_dir(app: &AppHandle) -> Result<PathBuf, String> {
    // Prefer bundled / checkout models next to the whisper weights.
    for candidate in [
        PathBuf::from("models"),
        PathBuf::from("src-tauri/models"),
        PathBuf::from("../src-tauri/models"),
    ] {
        if candidate.exists() {
            return Ok(candidate);
        }
    }
    if let Ok(p) = app.path().resolve("models", BaseDirectory::Resource) {
        if p.exists() {
            return Ok(p);
        }
    }
    // Fall back to writable app data (first-run download).
    app.path()
        .app_data_dir()
        .map(|d| d.join("models"))
        .map_err(|e| format!("app data dir: {e}"))
}

fn download_file(url: &str, dest: &Path) -> Result<(), String> {
    let tmp = dest.with_extension("download");
    let bytes = tauri::async_runtime::block_on(async {
        let resp = reqwest::get(url)
            .await
            .map_err(|e| format!("download {url}: {e}"))?;
        if !resp.status().is_success() {
            return Err(format!("download {url}: HTTP {}", resp.status()));
        }
        resp.bytes()
            .await
            .map_err(|e| format!("download body {url}: {e}"))
    })?;
    fs::write(&tmp, &bytes).map_err(|e| format!("write {}: {e}", tmp.display()))?;
    fs::rename(&tmp, dest).map_err(|e| format!("rename {}: {e}", dest.display()))?;
    Ok(())
}
