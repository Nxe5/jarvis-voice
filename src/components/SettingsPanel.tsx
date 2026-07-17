import { useVoiceStore } from "../state/voiceStore";

/**
 * Settings panel. The speak/TTS toggle is live; the mic-device and model
 * controls activate as Phases 3–5 (VAD, wake word) land.
 */
export function SettingsPanel({ onClose }: { onClose: () => void }) {
  const speakEnabled = useVoiceStore((s) => s.speakEnabled);
  const toggleSpeak = useVoiceStore((s) => s.toggleSpeak);

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

      <div className="settings__note">
        <strong>Agent</strong>: transcripts POST to the Kimi/Moonshot gateway at{" "}
        <code>127.0.0.1:18789</code> (OpenAI chat schema); the reply is spoken
        aloud. Override with <code>JARVIS_AGENT_URL</code>,{" "}
        <code>JARVIS_AGENT_KEY</code>, <code>JARVIS_AGENT_MODEL</code>. Use{" "}
        <code>JARVIS_AGENT_URL=echo</code> for an offline test.
      </div>
    </div>
  );
}
