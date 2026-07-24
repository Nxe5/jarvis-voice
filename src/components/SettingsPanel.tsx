import { useEffect, useRef, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { useVoiceStore } from "../state/voiceStore";
import {
  getAgentSettings,
  resolveOpenclawWslUrl,
  setAgentSettings,
} from "../lib/tauriEvents";
import type { AgentSettings } from "../types";

const OPENCLAW_URL = "http://127.0.0.1:18789/v1/chat/completions";
const OPENCLAW_MODEL = "openclaw/default";

const EMPTY_AGENT: AgentSettings = { preset: "openclaw", url: "", key: "", model: "" };

const PRESETS: { value: AgentSettings["preset"]; label: string }[] = [
  { value: "openclaw", label: "OpenClaw (default)" },
  { value: "openclaw_wsl", label: "OpenClaw on WSL" },
  { value: "custom", label: "Custom" },
];

/**
 * Settings panel. The speak/TTS toggle and agent gateway fields are live; the
 * mic-device and Whisper-model controls activate as Phases 3–5 (VAD, wake
 * word) land. TTS prefers local Piper neural speech when the model is ready.
 */
export function SettingsPanel({ onClose }: { onClose: () => void }) {
  const speakEnabled = useVoiceStore((s) => s.speakEnabled);
  const toggleSpeak = useVoiceStore((s) => s.toggleSpeak);

  const [agent, setAgent] = useState<AgentSettings>(EMPTY_AGENT);
  const [saved, setSaved] = useState(true);
  const [wslUrl, setWslUrl] = useState(OPENCLAW_URL);
  const [presetOpen, setPresetOpen] = useState(false);
  const [appVersion, setAppVersion] = useState("");
  const presetRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    getAgentSettings().then((s) => setAgent({ ...s, key: s.key ?? "" }));
    resolveOpenclawWslUrl().then(setWslUrl).catch(() => setWslUrl(OPENCLAW_URL));
    getVersion().then(setAppVersion).catch(() => setAppVersion(""));
  }, []);

  useEffect(() => {
    if (!presetOpen) return;
    const onPointerDown = (e: PointerEvent) => {
      if (presetRef.current && !presetRef.current.contains(e.target as Node)) {
        setPresetOpen(false);
      }
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setPresetOpen(false);
    };
    window.addEventListener("pointerdown", onPointerDown);
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("pointerdown", onPointerDown);
      window.removeEventListener("keydown", onKey);
    };
  }, [presetOpen]);

  const isOpenClaw = agent.preset === "openclaw";
  const isOpenClawWsl = agent.preset === "openclaw_wsl";
  const isLockedPreset = isOpenClaw || isOpenClawWsl;
  const lockedUrl = isOpenClawWsl ? wslUrl : OPENCLAW_URL;
  const presetLabel =
    PRESETS.find((p) => p.value === agent.preset)?.label ?? "Custom";

  const persist = (next: AgentSettings) => {
    setSaved(false);
    setAgentSettings({ ...next, key: next.key || null }).then(() => setSaved(true));
  };

  const field = (key: "url" | "key" | "model") => ({
    value: agent[key] ?? "",
    onChange: (e: React.ChangeEvent<HTMLInputElement>) => {
      setAgent((prev) => ({ ...prev, [key]: e.target.value }));
    },
    onBlur: () => persist(agent),
  });

  const handlePresetChange = (preset: AgentSettings["preset"]) => {
    const next = { ...agent, preset };
    setAgent(next);
    setPresetOpen(false);
    persist(next);
    if (preset === "openclaw_wsl") {
      resolveOpenclawWslUrl().then(setWslUrl).catch(() => setWslUrl(OPENCLAW_URL));
    }
  };

  return (
    <div className="settings">
      <div className="settings__head">
        <span>Settings</span>
        <button className="icon-btn" onClick={onClose} aria-label="Close settings">
          ✕
        </button>
      </div>

      <label className="field field--row">
        <span>Speak responses (TTS)</span>
        <button
          className={"switch" + (speakEnabled ? " switch--on" : "")}
          onClick={toggleSpeak}
          role="switch"
          aria-checked={speakEnabled}
        >
          <span className="switch__knob" />
        </button>
      </label>
      <div className="settings__note">
        Uses local <strong>Piper</strong> neural voice (
        <code>en_US-ryan-medium</code>) when available; falls back to the
        system voice otherwise. The model downloads once on first launch.
      </div>

      <label className="field">
        <span>Microphone</span>
        <select disabled defaultValue="default">
          <option value="default">System default</option>
        </select>
      </label>

      <label className="field">
        <span>Whisper model</span>
        <select disabled defaultValue="base.en">
          <option value="tiny.en">tiny.en — fastest</option>
          <option value="base.en">base.en — recommended</option>
          <option value="small.en">small.en — most accurate</option>
        </select>
      </label>

      <div className="settings__section">
        <div className="settings__section-head">
          <strong>Agent gateway</strong>
          <span className="settings__saved">{saved ? "Saved" : "Saving…"}</span>
        </div>

        <div className="field field--preset" ref={presetRef}>
          <span id="preset-label">Preset</span>
          <button
            type="button"
            className={"preset-btn" + (presetOpen ? " preset-btn--open" : "")}
            aria-haspopup="listbox"
            aria-expanded={presetOpen}
            aria-labelledby="preset-label"
            onClick={() => setPresetOpen((v) => !v)}
          >
            <span>{presetLabel}</span>
            <span className="preset-btn__chevron" aria-hidden>
              ▾
            </span>
          </button>
          {presetOpen && (
            <ul className="preset-menu" role="listbox" aria-labelledby="preset-label">
              {PRESETS.map((p) => (
                <li key={p.value} role="presentation">
                  <button
                    type="button"
                    role="option"
                    aria-selected={agent.preset === p.value}
                    className={
                      "preset-menu__item" +
                      (agent.preset === p.value ? " preset-menu__item--active" : "")
                    }
                    onClick={() => handlePresetChange(p.value)}
                  >
                    {p.label}
                  </button>
                </li>
              ))}
            </ul>
          )}
        </div>

        <label className="field">
          <span>URL</span>
          <input
            type="text"
            placeholder={OPENCLAW_URL}
            spellCheck={false}
            disabled={isLockedPreset}
            {...field("url")}
            value={isLockedPreset ? lockedUrl : agent.url}
          />
        </label>

        <label className="field">
          <span>API key</span>
          <input type="password" placeholder="optional" autoComplete="off" {...field("key")} />
        </label>

        <label className="field">
          <span>Model</span>
          <input
            type="text"
            placeholder={OPENCLAW_MODEL}
            spellCheck={false}
            disabled={isLockedPreset}
            {...field("model")}
            value={isLockedPreset ? OPENCLAW_MODEL : agent.model}
          />
        </label>

        <div className="settings__note">
          {isOpenClaw ? (
            <>
              Targets the <strong>local Windows</strong> OpenClaw Gateway at{" "}
              <code>127.0.0.1:18789</code>. Jarvis auto-reads{" "}
              <code>OPENCLAW_GATEWAY_TOKEN</code> from{" "}
              <code>~/.openclaw/openclaw.json</code> when API key is empty, and
              enables chat-completions if needed.
            </>
          ) : isOpenClawWsl ? (
            <>
              Targets <strong>OpenClaw</strong> inside <strong>WSL</strong> at
              its eth0 IP (not Windows localhost — a native OpenClaw often owns
              that port). Jarvis sets <code>gateway.bind: "lan"</code> and
              enables chat-completions if needed, and auto-reads the WSL{" "}
              <code>OPENCLAW_GATEWAY_TOKEN</code> when API key is empty.
            </>
          ) : (
            <>
              Any OpenAI-compatible chat-completions endpoint works (Kimi/Moonshot
              direct, another OpenClaw instance, etc). Set URL to{" "}
              <code>echo</code> for an offline loopback test.
            </>
          )}
        </div>
      </div>

      {appVersion ? (
        <div className="settings__note settings__version">Build {appVersion}</div>
      ) : null}
    </div>
  );
}
