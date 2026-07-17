import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import type { AudioLevel, StateChanged } from "../types";
import { useVoiceStore } from "../state/voiceStore";

/**
 * Subscribe the Zustand store to the Rust core's events. Call once on mount;
 * the returned function tears down all listeners.
 *
 * The webview never touches the microphone — the Rust core already owns it and
 * streams state transitions + audio levels here (architecture §5.3, §7.3).
 */
export async function bindVoiceEvents(): Promise<UnlistenFn> {
  const store = useVoiceStore.getState();

  const unlisteners = await Promise.all([
    listen<StateChanged>("voice-state-changed", (e) => {
      store.setState(e.payload.state, e.payload.detail);
    }),
    listen<AudioLevel>("voice-audio-level", (e) => {
      store.setAudioLevel(e.payload);
    }),
    listen<StateChanged>("voice-transcript", (e) => {
      if (e.payload.detail) store.setTranscript(e.payload.detail);
    }),
  ]);

  return () => unlisteners.forEach((u) => u());
}

/** Start a turn that auto-ends on silence (the "Talk" button). */
export function triggerWake(): Promise<void> {
  return invoke("trigger_wake");
}

/** Spacebar pressed — begin a push-to-talk turn (records until released). */
export function pushToTalkStart(): Promise<void> {
  return invoke("push_to_talk_start");
}

/** Spacebar released — end the push-to-talk turn and send it. */
export function pushToTalkStop(): Promise<void> {
  return invoke("push_to_talk_stop");
}
