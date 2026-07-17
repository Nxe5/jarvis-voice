import { useEffect, useRef } from "react";
import { useVoiceStore } from "../state/voiceStore";

type Msg = { id: string; role: "user" | "jarvis" | "error"; text: string };

/** A scrollable, two-person chat log of the whole conversation. */
export function Transcript() {
  const transcript = useVoiceStore((s) => s.transcript);
  const history = useVoiceStore((s) => s.history);
  const state = useVoiceStore((s) => s.state);
  const scrollRef = useRef<HTMLDivElement>(null);

  // Completed turns, oldest → newest.
  const messages: Msg[] = [];
  [...history].reverse().forEach((h, i) => {
    messages.push({ id: `u${h.at}-${i}`, role: "user", text: h.transcript || "…" });
    messages.push({
      id: `a${h.at}-${i}`,
      role: h.ok ? "jarvis" : "error",
      text: h.response,
    });
  });

  const listening = state === "LISTENING" || state === "WAKE_DETECTED";
  const thinking = state === "PROCESSING" || state === "DISPATCHING";

  // Keep pinned to the newest message.
  useEffect(() => {
    const el = scrollRef.current;
    if (el) el.scrollTop = el.scrollHeight;
  }, [messages.length, state, transcript]);

  const empty = messages.length === 0 && !listening && !thinking;

  return (
    <section className="transcript">
      <header className="transcript__head">Conversation</header>

      <div className="chat" ref={scrollRef}>
        {empty && (
          <p className="chat__empty">
            No messages yet. Double-clap or hold <strong>Space</strong>, then speak.
          </p>
        )}

        {messages.map((m) => (
          <div key={m.id} className={"msg msg--" + m.role}>
            <div className="msg__who">{m.role === "user" ? "You" : m.role === "error" ? "Error" : "Jarvis"}</div>
            <div className={"bubble bubble--" + m.role}>{m.text}</div>
          </div>
        ))}

        {/* Live, in-progress turn (not yet in history). */}
        {listening && (
          <div className="msg msg--user">
            <div className="msg__who">You</div>
            <div className="bubble bubble--user bubble--live">
              <span className="rec-dot" /> Listening…
            </div>
          </div>
        )}
        {thinking && (
          <>
            <div className="msg msg--user">
              <div className="msg__who">You</div>
              <div className="bubble bubble--user">{transcript || "…"}</div>
            </div>
            <div className="msg msg--jarvis">
              <div className="msg__who">Jarvis</div>
              <div className="bubble bubble--jarvis bubble--typing">
                <span />
                <span />
                <span />
              </div>
            </div>
          </>
        )}
      </div>
    </section>
  );
}
