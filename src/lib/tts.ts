/**
 * Text-to-speech via the browser's Web Speech API (WKWebView on macOS, WebView2
 * on Windows both support it). This is the "text → voice, played from the
 * browser" stage: the external agent's response is spoken aloud in the webview.
 */

let cachedVoice: SpeechSynthesisVoice | null = null;

function pickVoice(): SpeechSynthesisVoice | null {
  if (!("speechSynthesis" in window)) return null;
  const voices = window.speechSynthesis.getVoices();
  if (voices.length === 0) return null;
  if (cachedVoice && voices.includes(cachedVoice)) return cachedVoice;
  cachedVoice =
    voices.find((v) => /Samantha|Daniel|Alex|Google US English/i.test(v.name)) ??
    voices.find((v) => /^en[-_]/i.test(v.lang)) ??
    voices[0];
  return cachedVoice;
}

// Voices load asynchronously in some engines.
if ("speechSynthesis" in window) {
  window.speechSynthesis.onvoiceschanged = () => {
    cachedVoice = null;
    pickVoice();
  };
}

export function speak(text: string): void {
  if (!text || !("speechSynthesis" in window)) return;
  const synth = window.speechSynthesis;
  synth.cancel();
  const u = new SpeechSynthesisUtterance(text);
  const voice = pickVoice();
  if (voice) u.voice = voice;
  u.rate = 1.02;
  u.pitch = 1.0;
  synth.speak(u);
}

export function stopSpeaking(): void {
  if ("speechSynthesis" in window) window.speechSynthesis.cancel();
}
