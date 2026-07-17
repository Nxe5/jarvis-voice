//! Command dispatcher — the pluggable bridge between transcribed speech and the
//! agent backend (referred to in the architecture as "open claw").
//!
//! The real backend is intentionally swappable: it may end up being a
//! subprocess, an HTTP API, or a native SDK call. Everything the rest of the app
//! touches goes through the [`CommandBackend`] trait, so wiring in the real
//! integration later never has to touch the voice pipeline.

use async_trait::async_trait;
use std::fmt;

pub mod http;

/// An error produced while dispatching a command to the backend.
#[derive(Debug)]
pub struct DispatchError(pub String);

impl fmt::Display for DispatchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "dispatch error: {}", self.0)
    }
}

impl std::error::Error for DispatchError {}

/// A swappable command backend. Implementations forward transcribed text to
/// whatever actually fulfils the request and return a textual response.
#[async_trait]
pub trait CommandBackend: Send + Sync {
    async fn dispatch(&self, text: String) -> Result<String, DispatchError>;
}

/// Local Kimi/Moonshot gateway defaults. The user's server listens on
/// 127.0.0.1:18789 and speaks the OpenAI-compatible chat-completions schema.
const DEFAULT_AGENT_URL: &str = "http://127.0.0.1:18789/v1/chat/completions";
const DEFAULT_AGENT_MODEL: &str = "moonshot-v1-8k";

/// Choose the backend from the environment. Defaults to the local Kimi gateway;
/// set `JARVIS_AGENT_URL=echo` for an offline loopback test.
pub fn default_backend() -> Box<dyn CommandBackend> {
    let url = std::env::var("JARVIS_AGENT_URL")
        .ok()
        .filter(|u| !u.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_AGENT_URL.to_string());

    if url == "echo" {
        return Box::new(EchoBackend);
    }

    let key = std::env::var("JARVIS_AGENT_KEY").ok().filter(|k| !k.is_empty());
    let model = std::env::var("JARVIS_AGENT_MODEL")
        .ok()
        .filter(|m| !m.is_empty())
        .unwrap_or_else(|| DEFAULT_AGENT_MODEL.to_string());
    Box::new(http::HttpBackend::new(url, key, model))
}

/// Fallback stub backend: echoes the transcript back so the full state machine
/// (`IDLE → … → RESPONDING → IDLE`) can be exercised end to end even before an
/// external agent API is configured.
pub struct EchoBackend;

#[async_trait]
impl CommandBackend for EchoBackend {
    async fn dispatch(&self, text: String) -> Result<String, DispatchError> {
        if text.trim().is_empty() {
            return Err(DispatchError("empty transcript".into()));
        }
        Ok(format!("You said: \"{}\"", text.trim()))
    }
}
