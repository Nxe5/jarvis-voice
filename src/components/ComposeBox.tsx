import { useState, type FormEvent, type KeyboardEvent } from "react";
import { submitText } from "../lib/tauriEvents";
import { useVoiceStore } from "../state/voiceStore";

/** Chat-style input that dispatches typed turns the same way spoken ones do. */
export function ComposeBox() {
  const state = useVoiceStore((s) => s.state);
  const [draft, setDraft] = useState("");
  const [error, setError] = useState("");
  const [sending, setSending] = useState(false);

  const midTurn =
    state === "PROCESSING" || state === "DISPATCHING" || state === "RESPONDING";
  const busy = midTurn || sending;
  const canSend = draft.trim().length > 0 && !busy;

  const send = async () => {
    const text = draft.trim();
    if (!text || busy) return;
    setError("");
    setSending(true);
    try {
      await submitText(text);
      setDraft("");
    } catch (e) {
      const msg =
        typeof e === "string"
          ? e
          : e && typeof e === "object" && "message" in e
            ? String((e as { message: unknown }).message)
            : "Couldn't send — try again.";
      setError(msg);
    } finally {
      setSending(false);
    }
  };

  const onSubmit = (e: FormEvent) => {
    e.preventDefault();
    void send();
  };

  const onKeyDown = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      void send();
    }
  };

  return (
    <form className="compose" onSubmit={onSubmit}>
      <input
        className="compose__input"
        type="text"
        value={draft}
        onChange={(e) => {
          setDraft(e.target.value);
          if (error) setError("");
        }}
        onKeyDown={onKeyDown}
        placeholder={busy ? "Jarvis is busy…" : "Type a message…"}
        aria-label="Message to Jarvis"
        autoComplete="off"
      />
      <button
        type="submit"
        className="compose__send"
        disabled={!canSend}
        aria-label="Send message"
      >
        {sending ? "…" : "Send"}
      </button>
      {error && <p className="compose__error">{error}</p>}
    </form>
  );
}
