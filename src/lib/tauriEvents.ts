import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import type { AgentSettings, AudioLevel, StateChanged, VoiceState } from "../types";
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

  // Sync in case the engine was already mid-state before the webview mounted
  // (e.g. mic error flash) so the compose box isn't stuck thinking we're busy.
  try {
    const current = await invoke<VoiceState>("current_state");
    store.setState(current);
  } catch {
    /* webview-only / early invoke — events will catch up */
  }

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

/** Arm/disarm test mode: while on, every turn echoes the transcript back instead of dispatching to the agent. */
export function setTestMode(enabled: boolean): Promise<void> {
  return invoke("set_test_mode", { enabled });
}

/** Spacebar pressed — begin a push-to-talk turn (records until released). */
export function pushToTalkStart(): Promise<void> {
  return invoke("push_to_talk_start");
}

/** Spacebar released — end the push-to-talk turn and send it. */
export function pushToTalkStop(): Promise<void> {
  return invoke("push_to_talk_stop");
}

/** Submit a typed message through the same agent dispatch path as speech. */
export function submitText(text: string): Promise<void> {
  return invoke("submit_text", { text });
}

/** Current agent gateway URL/key/model. */
export function getAgentSettings(): Promise<AgentSettings> {
  return invoke("get_agent_settings");
}

/** Save new agent gateway settings — applies immediately and persists to disk. */
export function setAgentSettings(settings: AgentSettings): Promise<void> {
  return invoke("set_agent_settings", { settings });
}

/** Live OpenClaw-on-WSL gateway URL (localhost relay or eth0). */
export function resolveOpenclawWslUrl(): Promise<string> {
  return invoke("resolve_openclaw_wsl_url");
}
