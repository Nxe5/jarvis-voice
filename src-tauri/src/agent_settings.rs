//! Persisted, user-editable agent-gateway settings (Settings panel → Agent).
//!
//! Defaults target a local OpenClaw Gateway (`openclaw gateway --port 18789`,
//! its default port) via its OpenAI-compatible `/v1/chat/completions` surface
//! — `model: "openclaw/default"` is OpenClaw's stable alias for "whatever the
//! configured default agent is" (see docs/gateway/openai-http-api.md in the
//! OpenClaw repo). That endpoint is disabled by default on the gateway side;
//! enable it with `gateway.http.endpoints.chatCompletions.enabled: true`.
//!
//! Any other OpenAI-compatible endpoint (Kimi/Moonshot direct, etc.) works
//! too — just point the URL/model elsewhere from the Settings panel. Defaults
//! come from the `JARVIS_AGENT_*` env vars; whatever the user saves in the UI
//! overrides those and survives restarts as a small JSON file in the app's
//! config dir.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

pub const DEFAULT_AGENT_URL: &str = "http://127.0.0.1:18789/v1/chat/completions";
pub const DEFAULT_AGENT_MODEL: &str = "openclaw/default";
pub const DEFAULT_AGENT_PORT: u16 = 18789;

/// Resolve the OpenClaw gateway URL for a gateway running inside WSL.
///
/// Prefers the WSL eth0 IP when Windows can reach it. Falls back to
/// `127.0.0.1` (WSL's localhost relay) when eth0 is blocked — but only after
/// any native Windows OpenClaw on that port has been stopped (see
/// `openclaw::ensure_gateway`), otherwise localhost would hit the wrong
/// gateway.
pub fn openclaw_wsl_url() -> String {
    if let Some(lan) = openclaw_wsl_lan_url() {
        if tcp_open_url(&lan) {
            return lan;
        }
    }
    DEFAULT_AGENT_URL.to_string()
}

/// eth0-style URL for a LAN-bound WSL gateway.
pub fn openclaw_wsl_lan_url() -> Option<String> {
    resolve_wsl_ipv4()
        .map(|ip| format!("http://{ip}:{DEFAULT_AGENT_PORT}/v1/chat/completions"))
}

fn tcp_open_url(chat_completions_url: &str) -> bool {
    let Some((host, port)) = host_port(chat_completions_url) else {
        return false;
    };
    use std::net::{TcpStream, ToSocketAddrs};
    use std::time::Duration;
    let Ok(addrs) = (host.as_str(), port).to_socket_addrs() else {
        return false;
    };
    for addr in addrs {
        if TcpStream::connect_timeout(&addr, Duration::from_secs(1)).is_ok() {
            return true;
        }
    }
    false
}

fn host_port(url: &str) -> Option<(String, u16)> {
    let rest = url
        .strip_prefix("http://")
        .or_else(|| url.strip_prefix("https://"))?;
    let hostport = rest.split('/').next()?;
    if let Some((h, p)) = hostport.split_once(':') {
        Some((h.to_string(), p.parse().ok()?))
    } else {
        Some((hostport.to_string(), DEFAULT_AGENT_PORT))
    }
}

pub fn resolve_wsl_ipv4() -> Option<String> {
    #[cfg(windows)]
    {
        let output = std::process::Command::new("wsl")
            .args(["-e", "hostname", "-I"])
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        stdout
            .split_whitespace()
            .find(|tok| tok.parse::<std::net::Ipv4Addr>().is_ok())
            .map(|ip| ip.to_string())
    }
    #[cfg(not(windows))]
    {
        None
    }
}

/// Read `gateway.auth.token` for the OpenClaw install matching `preset`.
/// Used when the Settings API key field is empty so chat-completions auth
/// still works (OpenClaw returns plain `404 Not Found` without a bearer).
pub fn resolve_openclaw_gateway_token(preset: AgentPreset) -> Option<String> {
    if let Ok(t) = std::env::var("OPENCLAW_GATEWAY_TOKEN") {
        let t = t.trim().to_string();
        if !t.is_empty() {
            return Some(t);
        }
    }
    match preset {
        AgentPreset::OpenClawWsl => read_openclaw_token_wsl().or_else(read_openclaw_token_native),
        AgentPreset::OpenClaw => read_openclaw_token_native().or_else(read_openclaw_token_wsl),
        AgentPreset::Custom => None,
    }
}

fn read_openclaw_token_native() -> Option<String> {
    let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME"))?;
    let path = PathBuf::from(home).join(".openclaw").join("openclaw.json");
    read_token_from_openclaw_json(&path)
}

fn read_openclaw_token_wsl() -> Option<String> {
    #[cfg(windows)]
    {
        let output = std::process::Command::new("wsl")
            .args([
                "-e",
                "bash",
                "-lc",
                r#"python3 -c 'import json,pathlib; p=pathlib.Path.home()/".openclaw"/"openclaw.json"; print(json.load(p.open())["gateway"]["auth"]["token"])'"#,
            ])
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        let token = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if token.is_empty() {
            None
        } else {
            Some(token)
        }
    }
    #[cfg(not(windows))]
    {
        None
    }
}

fn read_token_from_openclaw_json(path: &std::path::Path) -> Option<String> {
    let raw = fs::read_to_string(path).ok()?;
    let v: serde_json::Value = serde_json::from_str(&raw).ok()?;
    let token = v
        .pointer("/gateway/auth/token")
        .and_then(|t| t.as_str())
        .map(str::trim)
        .filter(|t| !t.is_empty())?;
    Some(token.to_string())
}

/// Which URL/model the dispatcher actually uses (see `dispatcher::backend_for`).
/// `OpenClaw` always uses the localhost OpenClaw defaults; `OpenClawWsl`
/// resolves the live WSL eth0 URL each dispatch (see `openclaw_wsl_url`).
/// Both ignore whatever's saved in `url`/`model`. `Custom` uses them as saved.
/// The API key applies in all cases (OpenClaw presets also auto-read
/// `gateway.auth.token` when the field is empty).
#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum AgentPreset {
    #[default]
    #[serde(rename = "openclaw")]
    OpenClaw,
    #[serde(rename = "openclaw_wsl")]
    OpenClawWsl,
    #[serde(rename = "custom")]
    Custom,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct AgentSettings {
    #[serde(default)]
    pub preset: AgentPreset,
    pub url: String,
    #[serde(default)]
    pub key: Option<String>,
    pub model: String,
}

impl Default for AgentSettings {
    fn default() -> Self {
        let url = std::env::var("JARVIS_AGENT_URL").ok().filter(|u| !u.trim().is_empty());
        let model = std::env::var("JARVIS_AGENT_MODEL").ok().filter(|m| !m.trim().is_empty());
        // An env var pointing anywhere other than the OpenClaw defaults means
        // the user already intends a custom endpoint.
        let is_custom = url.as_deref().is_some_and(|u| u != DEFAULT_AGENT_URL)
            || model.as_deref().is_some_and(|m| m != DEFAULT_AGENT_MODEL);
        Self {
            preset: if is_custom {
                AgentPreset::Custom
            } else {
                AgentPreset::OpenClaw
            },
            url: url.unwrap_or_else(|| DEFAULT_AGENT_URL.to_string()),
            key: std::env::var("JARVIS_AGENT_KEY").ok().filter(|k| !k.is_empty()),
            model: model.unwrap_or_else(|| DEFAULT_AGENT_MODEL.to_string()),
        }
    }
}

fn config_path(app: &AppHandle) -> Option<PathBuf> {
    app.path()
        .app_config_dir()
        .ok()
        .map(|d| d.join("agent_settings.json"))
}

impl AgentSettings {
    /// Load the persisted file if present, else fall back to env-var defaults.
    pub fn load(app: &AppHandle) -> Self {
        config_path(app)
            .and_then(|p| fs::read_to_string(p).ok())
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, app: &AppHandle) -> Result<(), String> {
        let path = config_path(app).ok_or("no app config directory available")?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let json = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        fs::write(path, json).map_err(|e| e.to_string())
    }
}
