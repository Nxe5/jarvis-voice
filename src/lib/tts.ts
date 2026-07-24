/**
 * Text-to-speech: prefers local Piper neural voice (via Rust), falls back to
 * the browser Web Speech API if Piper isn't loaded yet.
 */

import { invoke } from "@tauri-apps/api/core";

let speaking = false;
let currentAudio: HTMLAudioElement | null = null;
let currentObjectUrl: string | null = null;
const listeners = new Set<(value: boolean) => void>();

function setSpeaking(value: boolean): void {
  if (speaking === value) return;
  speaking = value;
  listeners.forEach((fn) => fn(value));
}

/** Whether TTS audio is currently playing. */
export function isSpeaking(): boolean {
  return speaking;
}

/** Subscribe to speaking on/off changes. Returns an unsubscribe. */
export function subscribeSpeaking(fn: (value: boolean) => void): () => void {
  listeners.add(fn);
  return () => {
    listeners.delete(fn);
  };
}

function stopAudio(): void {
  if (currentAudio) {
    currentAudio.pause();
    currentAudio.removeAttribute("src");
    currentAudio = null;
  }
  if (currentObjectUrl) {
    URL.revokeObjectURL(currentObjectUrl);
    currentObjectUrl = null;
  }
}

function pickVoice(): SpeechSynthesisVoice | null {
  if (!("speechSynthesis" in window)) return null;
  const voices = window.speechSynthesis.getVoices();
  if (voices.length === 0) return null;
  return (
    voices.find((v) =>
      /Neural|Natural|Online|Samantha|Daniel|Alex|Google US English|Microsoft.*(Aria|Jenny|Guy|Ryan)/i.test(
        v.name,
      ),
    ) ??
    voices.find((v) => /^en[-_]/i.test(v.lang)) ??
    voices[0]
  );
}

if ("speechSynthesis" in window) {
  window.speechSynthesis.onvoiceschanged = () => {
    pickVoice();
  };
}

function speakBrowser(text: string): void {
  if (!text || !("speechSynthesis" in window)) {
    setSpeaking(false);
    return;
  }
  const synth = window.speechSynthesis;
  synth.cancel();
  const u = new SpeechSynthesisUtterance(text);
  const voice = pickVoice();
  if (voice) u.voice = voice;
  u.rate = 1.02;
  u.pitch = 1.0;
  u.onstart = () => setSpeaking(true);
  u.onend = () => setSpeaking(false);
  u.onerror = () => setSpeaking(false);
  setSpeaking(true);
  synth.speak(u);
}

function playWavBase64(b64: string): Promise<void> {
  return new Promise((resolve, reject) => {
    const binary = atob(b64);
    const bytes = new Uint8Array(binary.length);
    for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i);
    const blob = new Blob([bytes], { type: "audio/wav" });
    const url = URL.createObjectURL(blob);
    const audio = new Audio(url);
    currentAudio = audio;
    currentObjectUrl = url;
    audio.onended = () => {
      stopAudio();
      setSpeaking(false);
      resolve();
    };
    audio.onerror = () => {
      stopAudio();
      setSpeaking(false);
      reject(new Error("audio playback failed"));
    };
    setSpeaking(true);
    audio.play().catch((err) => {
      stopAudio();
      setSpeaking(false);
      reject(err);
    });
  });
}

/** Speak `text` with local Piper neural TTS, or browser fallback. */
export function speak(text: string): void {
  if (!text) return;
  stopSpeaking();

  void (async () => {
    try {
      const wav = await invoke<string>("synthesize_speech", { text });
      await playWavBase64(wav);
    } catch {
      // Piper still downloading / failed — use system voice.
      speakBrowser(text);
    }
  })();
}

export function stopSpeaking(): void {
  stopAudio();
  if ("speechSynthesis" in window) window.speechSynthesis.cancel();
  setSpeaking(false);
}
