//! Microphone capture via cpal (CoreAudio on macOS, WASAPI on Windows).
//!
//! Opens the default input device once at startup and streams mono f32 samples
//! into a channel. The voice engine consumes that stream continuously — first to
//! listen for a double-clap trigger, then to buffer the spoken command. Helpers
//! here stay dumb (open + downmix + resample); all the state logic lives in the
//! engine (`state_machine.rs`).

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, Sample, SizedSample};
use std::sync::mpsc::Sender;

/// A live input stream plus the device's native sample rate. The `stream` must
/// be kept alive (on the thread that created it) for capture to continue.
pub struct MicStream {
    #[allow(dead_code)]
    pub stream: cpal::Stream,
    pub sample_rate: u32,
}

/// Build a typed input stream that downmixes to mono and forwards chunks.
fn build_stream<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    channels: usize,
    tx: Sender<Vec<f32>>,
) -> Result<cpal::Stream, String>
where
    T: SizedSample,
    f32: FromSample<T>,
{
    let ch = channels.max(1);
    device
        .build_input_stream(
            config.clone(),
            move |data: &[T], _: &cpal::InputCallbackInfo| {
                let mut out = Vec::with_capacity(data.len() / ch);
                for frame in data.chunks(ch) {
                    let mut sum = 0.0f32;
                    for &s in frame {
                        sum += f32::from_sample(s);
                    }
                    out.push(sum / ch as f32);
                }
                let _ = tx.send(out);
            },
            move |err| eprintln!("audio stream error: {err}"),
            None,
        )
        .map_err(|e| e.to_string())
}

/// Open the default input device and start streaming mono samples to `tx`.
pub fn open_input(tx: Sender<Vec<f32>>) -> Result<MicStream, String> {
    let host = cpal::default_host();
    let device = host
        .default_input_device()
        .ok_or_else(|| "no input device available".to_string())?;
    let supported = device
        .default_input_config()
        .map_err(|e| format!("no default input config: {e}"))?;

    let sample_format = supported.sample_format();
    let sample_rate = supported.sample_rate(); // cpal 0.18: SampleRate = u32
    let channels = supported.channels() as usize;
    let config: cpal::StreamConfig = supported.into();

    let stream = match sample_format {
        cpal::SampleFormat::F32 => build_stream::<f32>(&device, &config, channels, tx),
        cpal::SampleFormat::I16 => build_stream::<i16>(&device, &config, channels, tx),
        cpal::SampleFormat::U16 => build_stream::<u16>(&device, &config, channels, tx),
        cpal::SampleFormat::I32 => build_stream::<i32>(&device, &config, channels, tx),
        cpal::SampleFormat::I8 => build_stream::<i8>(&device, &config, channels, tx),
        other => Err(format!("unsupported sample format: {other:?}")),
    }?;
    stream.play().map_err(|e| e.to_string())?;

    Ok(MicStream {
        stream,
        sample_rate,
    })
}

/// Simple linear resampler to Whisper's required 16 kHz.
pub fn resample_linear(input: &[f32], from: u32, to: u32) -> Vec<f32> {
    if input.is_empty() || from == to {
        return input.to_vec();
    }
    let ratio = to as f32 / from as f32;
    let out_len = ((input.len() as f32) * ratio) as usize;
    let mut out = Vec::with_capacity(out_len);
    let last = input.len() - 1;
    for i in 0..out_len {
        let src = i as f32 / ratio;
        let idx = src.floor() as usize;
        let frac = src - idx as f32;
        let a = input[idx.min(last)];
        let b = input[(idx + 1).min(last)];
        out.push(a + (b - a) * frac);
    }
    out
}
