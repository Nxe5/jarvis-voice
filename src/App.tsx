import { useEffect, useState, useSyncExternalStore } from "react";
import { JarvisOrb } from "./components/JarvisOrb";
import { Transcript } from "./components/Transcript";
import { SettingsPanel } from "./components/SettingsPanel";
import {
  bindVoiceEvents,
  triggerWake,
  setTestMode,
  pushToTalkStart,
  pushToTalkStop,
} from "./lib/tauriEvents";
import { isSpeaking, speak, stopSpeaking, subscribeSpeaking } from "./lib/tts";
import { useVoiceStore } from "./state/voiceStore";
import type { VoiceState } from "./types";
import "./App.css";

const LABELS: Record<VoiceState, string> = {
  IDLE: "Ready",
  WAKE_DETECTED: "Waking",
  LISTENING: "Listening",
  PROCESSING: "Processing",
  DISPATCHING: "Thinking",
  RESPONDING: "Responding",
  ERROR: "Error",
};

function App() {
  const state = useVoiceStore((s) => s.state);
  const response = useVoiceStore((s) => s.response);
  const testMode = useVoiceStore((s) => s.testMode);
  const toggleTestMode = useVoiceStore((s) => s.toggleTestMode);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const speaking = useSyncExternalStore(subscribeSpeaking, isSpeaking, () => false);

  const handleToggleTestMode = () => {
    const next = !testMode;
    toggleTestMode();
    setTestMode(next);
  };

  const statusLabel =
    state === "DISPATCHING" && response ? response : LABELS[state];

  useEffect(() => {
    const unbind = bindVoiceEvents();
    return () => {
      unbind.then((fn) => fn());
    };
  }, []);

  // Speak agent replies / errors only — not DISPATCHING status like
  // "OpenClaw isn't active — starting it in WSL…".
  useEffect(() => {
    if (
      (state === "RESPONDING" || state === "ERROR") &&
      response &&
      useVoiceStore.getState().speakEnabled
    ) {
      speak(response);
    }
  }, [state, response]);

  // A new interaction interrupts any ongoing speech.
  useEffect(() => {
    if (state === "WAKE_DETECTED") stopSpeaking();
  }, [state]);

  // Spacebar = push-to-talk: hold to record, release to send.
  useEffect(() => {
    const holding = { current: false };
    const isTyping = (el: EventTarget | null) => {
      const t = el as HTMLElement | null;
      if (!t) return false;
      const tag = t.tagName;
      return (
        tag === "INPUT" ||
        tag === "TEXTAREA" ||
        tag === "SELECT" ||
        t.isContentEditable
      );
    };
    const onDown = (e: KeyboardEvent) => {
      if (e.code !== "Space" || e.repeat || isTyping(e.target)) return;
      e.preventDefault();
      if (holding.current) return;
      if (useVoiceStore.getState().state !== "IDLE") return;
      holding.current = true;
      pushToTalkStart();
    };
    const onUp = (e: KeyboardEvent) => {
      if (e.code !== "Space" || !holding.current) return;
      e.preventDefault();
      holding.current = false;
      pushToTalkStop();
    };
    window.addEventListener("keydown", onDown);
    window.addEventListener("keyup", onUp);
    return () => {
      window.removeEventListener("keydown", onDown);
      window.removeEventListener("keyup", onUp);
    };
  }, []);

  const busy = state !== "IDLE";
  const canStop = speaking || state === "RESPONDING";

  const handlePrimary = () => {
    if (canStop) {
      stopSpeaking();
      return;
    }
    if (!busy) triggerWake();
  };

  return (
    <main className="app" data-state={state}>
      <header className="topbar">
        <div className="brand">
          <span className="brand__dot" />
          <span className="brand__name">JARVIS</span>
        </div>
        <div className="status">
          <span className="mic-live" title="Microphone is always on, listening for a double-clap">
            <span className="mic-live__ring" />●
          </span>
          <span className={"status__pip status__pip--" + state.toLowerCase()} />
          {statusLabel}
        </div>
        <div className="topbar__actions">
          <button
            className={"icon-btn" + (speaking ? " icon-btn--on" : "")}
            onClick={() => stopSpeaking()}
            disabled={!speaking}
            aria-label="Stop speaking"
            title={speaking ? "Stop speaking" : "Nothing playing"}
          >
            ■
          </button>
          <button
            className={"icon-btn" + (testMode ? " icon-btn--on" : "")}
            onClick={handleToggleTestMode}
            aria-label="Test mode"
            aria-pressed={testMode}
            title={
              testMode
                ? "Test mode is on — turns echo back instead of reaching the agent"
                : "Test mode is off — turns dispatch to the agent as normal"
            }
          >
            🔁
          </button>
          <button
            className="icon-btn"
            onClick={() => setSettingsOpen((v) => !v)}
            aria-label="Settings"
          >
            ⚙
          </button>
        </div>
      </header>

      <div className="stage">
        <div className="stage__hero">
          <div className="orb-wrap">
            <JarvisOrb paused={settingsOpen} />
          </div>

          <div className="controls">
            <button
              className={"wake-btn" + (canStop ? " wake-btn--stop" : "")}
              onClick={handlePrimary}
              disabled={busy && !canStop}
            >
              {canStop ? "Stop" : busy ? LABELS[state] + "…" : "Talk"}
            </button>
          </div>

          <p className="clap-hint">
            <span className="clap-hint__emoji">👏 👏</span>
            Double-clap, hold <strong>Space</strong>, or type a message
          </p>
        </div>

        <Transcript />
      </div>

      {settingsOpen && <SettingsPanel onClose={() => setSettingsOpen(false)} />}
    </main>
  );
}

export default App;
