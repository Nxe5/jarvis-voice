//! Probe and auto-start a local OpenClaw Gateway when Jarvis needs it.
//!
//! Prefers the install matching the Settings preset (`OpenClaw` → native
//! Windows, `OpenClawWsl` → WSL). Falls back to wherever `openclaw` is found.
//! If WSL is selected and OpenClaw isn't installed there, runs the official
//! install script. Status text is returned so the voice UI can tell the user
//! what's happening.

use crate::agent_settings::{
    openclaw_wsl_lan_url, openclaw_wsl_url, resolve_wsl_ipv4, AgentPreset, AgentSettings,
    DEFAULT_AGENT_PORT, DEFAULT_AGENT_URL,
};
use std::net::TcpStream;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const PROBE_TIMEOUT: Duration = Duration::from_secs(1);
const START_WAIT: Duration = Duration::from_secs(25);
const INSTALL_WAIT: Duration = Duration::from_secs(180);
const POLL_EVERY: Duration = Duration::from_millis(500);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallKind {
    Native,
    Wsl,
}

impl InstallKind {
    fn label(self) -> &'static str {
        match self {
            Self::Native => "locally",
            Self::Wsl => "in WSL",
        }
    }
}

/// Make sure the OpenClaw gateway for `settings` is reachable. On success,
/// returns `Ok(())`. On failure, returns a user-facing error string.
///
/// Calls `on_status` as soon as inactivity is detected (before start / wait),
/// so the UI can tell the user immediately.
pub fn ensure_gateway(
    settings: &AgentSettings,
    mut on_status: impl FnMut(&str),
) -> Result<(), String> {
    if !matches!(
        settings.preset,
        AgentPreset::OpenClaw | AgentPreset::OpenClawWsl
    ) {
        return Ok(());
    }

    // WSL preset: eth0 first. If it's down, stop a native Windows gateway that
    // shadows 127.0.0.1:18789 so the WSL localhost relay can work as fallback.
    if matches!(settings.preset, AgentPreset::OpenClawWsl) {
        if let Some(lan) = openclaw_wsl_lan_url() {
            if probe_url(&lan) {
                return Ok(());
            }
        }
        stop_native_gateway();
        if probe_url(DEFAULT_AGENT_URL) || openclaw_wsl_lan_url().is_some_and(|u| probe_url(&u)) {
            return Ok(());
        }
    } else {
        let url = gateway_url(settings);
        if probe_url(&url) {
            // Native OpenClaw may accept TCP while chat-completions is still off.
            let _ = ensure_native_chat_completions();
            return Ok(());
        }
    }

    let preferred = match settings.preset {
        AgentPreset::OpenClawWsl => InstallKind::Wsl,
        _ => InstallKind::Native,
    };
    let native = openclaw_on_path();
    let mut wsl = openclaw_in_wsl();

    let kind = match preferred {
        // WSL preset must talk to a WSL-hosted gateway (localhost relay or eth0).
        InstallKind::Wsl => {
            if !wsl {
                on_status("OpenClaw isn't installed in WSL — installing it now…");
                install_openclaw_wsl()?;
                wsl = openclaw_in_wsl();
                if !wsl {
                    return Err(
                        "Couldn't install OpenClaw in WSL. In a WSL terminal run: curl -fsSL https://openclaw.ai/install.sh | bash"
                            .into(),
                    );
                }
            }
            InstallKind::Wsl
        }
        InstallKind::Native => pick_install(InstallKind::Native, native, wsl).ok_or_else(|| {
            "OpenClaw isn't active, and it doesn't look installed locally or in WSL.".to_string()
        })?,
    };

    let starting = if kind == preferred {
        format!("OpenClaw isn't active — starting it {}…", kind.label())
    } else {
        format!(
            "OpenClaw isn't active — starting it {} (not found for the selected preset)…",
            kind.label()
        )
    };
    on_status(&starting);

    start_gateway(kind, settings.preset)?;

    let deadline = Instant::now() + START_WAIT;
    while Instant::now() < deadline {
        if wsl_or_default_reachable(settings) {
            return Ok(());
        }
        std::thread::sleep(POLL_EVERY);
    }

    let url = gateway_url(settings);
    Err(format!(
        "OpenClaw isn't active — tried starting it {}, but the gateway at {url} never came up.",
        kind.label()
    ))
}

fn wsl_or_default_reachable(settings: &AgentSettings) -> bool {
    match settings.preset {
        AgentPreset::OpenClawWsl => {
            openclaw_wsl_lan_url().is_some_and(|u| probe_url(&u)) || probe_url(DEFAULT_AGENT_URL)
        }
        _ => probe_url(&gateway_url(settings)),
    }
}

fn gateway_url(settings: &AgentSettings) -> String {
    match settings.preset {
        AgentPreset::OpenClawWsl => openclaw_wsl_url(),
        _ => DEFAULT_AGENT_URL.to_string(),
    }
}

fn pick_install(preferred: InstallKind, native: bool, wsl: bool) -> Option<InstallKind> {
    match preferred {
        InstallKind::Native if native => Some(InstallKind::Native),
        InstallKind::Wsl if wsl => Some(InstallKind::Wsl),
        InstallKind::Native if wsl => Some(InstallKind::Wsl),
        InstallKind::Wsl if native => Some(InstallKind::Native),
        _ => None,
    }
}

fn openclaw_on_path() -> bool {
    resolve_native_openclaw().is_some()
}

/// Resolve the native OpenClaw CLI on PATH.
///
/// npm installs `openclaw.cmd` on Windows. `Command::new("openclaw")` only
/// finds `.exe`, so detection via `where` succeeds while start fails with
/// "program not found". Prefer an absolute `.cmd` / `.exe` path from `where`.
fn resolve_native_openclaw() -> Option<std::path::PathBuf> {
    #[cfg(windows)]
    {
        let output = Command::new("where")
            .arg("openclaw")
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        let text = String::from_utf8_lossy(&output.stdout);
        let mut cmd_path = None;
        let mut exe_path = None;
        let mut other = None;
        for line in text.lines().map(str::trim).filter(|l| !l.is_empty()) {
            let p = std::path::PathBuf::from(line);
            let ext = p
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            match ext.as_str() {
                // Prefer .cmd — overwrite so a bare `openclaw` shim listed first
                // doesn't win over `openclaw.cmd`.
                "cmd" | "bat" => cmd_path = Some(p),
                "exe" if exe_path.is_none() => exe_path = Some(p),
                _ if other.is_none() => other = Some(p),
                _ => {}
            }
        }
        cmd_path.or(exe_path).or(other)
    }
    #[cfg(not(windows))]
    {
        let output = Command::new("sh")
            .args(["-c", "command -v openclaw"])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        let path = String::from_utf8_lossy(&output.stdout)
            .lines()
            .next()?
            .trim()
            .to_string();
        if path.is_empty() {
            None
        } else {
            Some(std::path::PathBuf::from(path))
        }
    }
}

/// Build a `Command` that can actually invoke the npm OpenClaw shim on Windows.
fn native_openclaw_command(args: &[&str]) -> Result<Command, String> {
    let bin = resolve_native_openclaw()
        .ok_or_else(|| "OpenClaw CLI not found on PATH.".to_string())?;
    #[cfg(windows)]
    {
        // CreateProcess cannot run `.cmd` directly — go through `cmd /C`.
        let is_script = bin
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| matches!(e.to_ascii_lowercase().as_str(), "cmd" | "bat"))
            .unwrap_or(false);
        if is_script {
            let mut cmd = Command::new("cmd");
            cmd.arg("/D").arg("/C").arg(bin.as_os_str());
            for a in args {
                cmd.arg(a);
            }
            return Ok(cmd);
        }
    }
    let mut cmd = Command::new(bin);
    cmd.args(args);
    Ok(cmd)
}

fn openclaw_in_wsl() -> bool {
    #[cfg(windows)]
    {
        Command::new("wsl")
            .args(["-e", "bash", "-lc", "command -v openclaw >/dev/null"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }
    #[cfg(not(windows))]
    {
        false
    }
}

/// Official Linux installer inside the default WSL distro.
fn install_openclaw_wsl() -> Result<(), String> {
    #[cfg(windows)]
    {
        // Non-interactive: install CLI only; gateway start happens afterward.
        let script = "curl -fsSL --proto '=https' --tlsv1.2 https://openclaw.ai/install.sh | bash -s -- --no-onboard";
        let mut child = Command::new("wsl")
            .args(["-e", "bash", "-lc", script])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("Couldn't launch WSL installer: {e}"))?;

        let deadline = Instant::now() + INSTALL_WAIT;
        loop {
            match child.try_wait() {
                Ok(Some(status)) if status.success() => return Ok(()),
                Ok(Some(status)) => {
                    let err = child
                        .stderr
                        .take()
                        .and_then(|mut s| {
                            let mut buf = String::new();
                            std::io::Read::read_to_string(&mut s, &mut buf).ok()?;
                            Some(buf)
                        })
                        .unwrap_or_default();
                    let hint = err.lines().rev().take(6).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join("\n");
                    return Err(format!(
                        "OpenClaw install in WSL failed (exit {status}). {}",
                        if hint.is_empty() {
                            "Try: curl -fsSL https://openclaw.ai/install.sh | bash".into()
                        } else {
                            hint
                        }
                    ));
                }
                Ok(None) if Instant::now() < deadline => {
                    std::thread::sleep(POLL_EVERY);
                }
                Ok(None) => {
                    let _ = child.kill();
                    return Err("OpenClaw install in WSL timed out.".into());
                }
                Err(e) => return Err(format!("OpenClaw install in WSL failed: {e}")),
            }
        }
    }
    #[cfg(not(windows))]
    {
        Err("WSL install is only available on Windows.".into())
    }
}

fn start_gateway(kind: InstallKind, preset: AgentPreset) -> Result<(), String> {
    match kind {
        InstallKind::Native => match start_native() {
            Ok(()) => Ok(()),
            // npm shim / PATH issues on Windows — WSL OpenClaw is a valid fallback.
            Err(native_err) if openclaw_in_wsl() => start_wsl(preset).map_err(|wsl_err| {
                format!("{native_err} (WSL fallback also failed: {wsl_err})")
            }),
            Err(e) => Err(e),
        },
        InstallKind::Wsl => start_wsl(preset),
    }
}

fn start_native() -> Result<(), String> {
    // Make sure chat-completions is enabled — otherwise POST /v1/chat/completions
    // returns a bare "404 Not Found".
    let _ = ensure_native_chat_completions();

    // Prefer the installed service (Scheduled Task on Windows).
    let status = native_openclaw_command(&["gateway", "start"])?
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .status()
        .map_err(|e| format!("Couldn't run openclaw gateway start: {e}"))?;
    if status.success() {
        return Ok(());
    }
    // Fall back to a detached foreground run.
    let bin = resolve_native_openclaw()
        .ok_or_else(|| "OpenClaw CLI not found on PATH.".to_string())?;
    let port = DEFAULT_AGENT_PORT.to_string();
    let args = [
        "gateway",
        "run",
        "--port",
        port.as_str(),
    ];
    spawn_detached_openclaw(&bin, &args)
        .map_err(|e| format!("Couldn't start OpenClaw locally: {e}"))
}

fn start_wsl(preset: AgentPreset) -> Result<(), String> {
    // Free 127.0.0.1:18789 so WSL's localhost relay (and eth0) can be reached
    // from Windows. A native Windows OpenClaw otherwise owns the port.
    if matches!(preset, AgentPreset::OpenClawWsl) {
        stop_native_gateway();
    }

    // Login shell so nvm / ~/.local/bin installs resolve (same as detection).
    let _ = Command::new("wsl")
        .args(["-e", "bash", "-lc", "openclaw gateway start"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();

    std::thread::sleep(Duration::from_millis(800));

    // OpenClaw-on-WSL: prefer eth0; localhost relay works once native is stopped.
    if matches!(preset, AgentPreset::OpenClawWsl) {
        if wsl_gateway_reachable() {
            return Ok(());
        }
        // Patch bind=lan + chatCompletions, restart, then force a LAN run if needed.
        ensure_wsl_lan_config()?;
        let _ = Command::new("wsl")
            .args(["-e", "bash", "-lc", "openclaw gateway restart"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        std::thread::sleep(Duration::from_millis(1200));
        if wsl_gateway_reachable() {
            return Ok(());
        }
        return spawn_wsl_lan_gateway();
    }

    // Native-fallback path using WSL: loopback inside WSL is enough when no
    // Windows process owns 127.0.0.1:18789.
    if probe_url(DEFAULT_AGENT_URL) {
        return Ok(());
    }
    spawn_wsl_lan_gateway()
}

fn wsl_gateway_reachable() -> bool {
    openclaw_wsl_lan_url().is_some_and(|u| probe_url(&u)) || probe_url(DEFAULT_AGENT_URL)
}

/// Stop the native Windows OpenClaw scheduled-task gateway so it cannot steal
/// `127.0.0.1:18789` from the WSL install.
fn stop_native_gateway() {
    #[cfg(windows)]
    {
        if let Ok(mut cmd) = native_openclaw_command(&["gateway", "stop"]) {
            let _ = cmd.stdout(Stdio::null()).stderr(Stdio::null()).status();
        }
        // Brief pause so the port is released before WSL / wslrelay bind.
        std::thread::sleep(Duration::from_millis(400));
    }
}

/// Persist `gateway.bind=lan` + chatCompletions enabled inside WSL so Windows
/// can reach the eth0 listener.
fn ensure_wsl_lan_config() -> Result<(), String> {
    let script = r#"
python3 - <<'PY'
import json
from pathlib import Path
p = Path.home() / ".openclaw" / "openclaw.json"
p.parent.mkdir(parents=True, exist_ok=True)
d = json.loads(p.read_text()) if p.exists() else {}
g = d.setdefault("gateway", {})
g["bind"] = "lan"
g.setdefault("mode", "local")
http = g.setdefault("http", {})
ep = http.setdefault("endpoints", {})
ep.setdefault("chatCompletions", {})["enabled"] = True
p.write_text(json.dumps(d, indent=2) + "\n")
print("ok")
PY
"#;
    let output = Command::new("wsl")
        .args(["-e", "bash", "-lc", script])
        .output()
        .map_err(|e| format!("Couldn't update WSL OpenClaw config: {e}"))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "Couldn't update WSL OpenClaw config: {}",
            String::from_utf8_lossy(&output.stderr)
        ))
    }
}

/// Enable chat-completions on the native Windows OpenClaw config if missing.
fn ensure_native_chat_completions() -> Result<(), String> {
    let home = std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .ok_or_else(|| "no home directory".to_string())?;
    let path = std::path::PathBuf::from(home)
        .join(".openclaw")
        .join("openclaw.json");
    if !path.exists() {
        return Ok(());
    }
    let raw = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let mut v: serde_json::Value =
        serde_json::from_str(&raw).map_err(|e| format!("bad openclaw.json: {e}"))?;
    let enabled = v
        .pointer("/gateway/http/endpoints/chatCompletions/enabled")
        .and_then(|x| x.as_bool())
        == Some(true);
    if enabled {
        return Ok(());
    }
    let gateway = v
        .as_object_mut()
        .ok_or("openclaw.json root")?
        .entry("gateway")
        .or_insert_with(|| serde_json::json!({}));
    let http = gateway
        .as_object_mut()
        .ok_or("gateway")?
        .entry("http")
        .or_insert_with(|| serde_json::json!({}));
    let endpoints = http
        .as_object_mut()
        .ok_or("http")?
        .entry("endpoints")
        .or_insert_with(|| serde_json::json!({}));
    let chat = endpoints
        .as_object_mut()
        .ok_or("endpoints")?
        .entry("chatCompletions")
        .or_insert_with(|| serde_json::json!({}));
    chat.as_object_mut()
        .ok_or("chatCompletions")?
        .insert("enabled".into(), serde_json::json!(true));
    std::fs::write(&path, serde_json::to_string_pretty(&v).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    // Restart so the running native gateway picks up the endpoint.
    if let Ok(mut cmd) = native_openclaw_command(&["gateway", "restart"]) {
        let _ = cmd.stdout(Stdio::null()).stderr(Stdio::null()).status();
    }
    Ok(())
}

fn spawn_wsl_lan_gateway() -> Result<(), String> {
    // --force frees the port if a loopback-only gateway is already up.
    let script = format!(
        "nohup openclaw gateway run --bind lan --force --port {DEFAULT_AGENT_PORT} \
         >/tmp/jarvis-openclaw-gateway.log 2>&1 & disown"
    );
    Command::new("wsl")
        .args(["-e", "bash", "-lc", &script])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|e| e.to_string())
        .and_then(|s| {
            if s.success() {
                Ok(())
            } else {
                Err(format!("wsl openclaw gateway run exited with {s}"))
            }
        })
}

fn spawn_detached_openclaw(bin: &std::path::Path, args: &[&str]) -> Result<(), String> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        let is_script = bin
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| matches!(e.to_ascii_lowercase().as_str(), "cmd" | "bat"))
            .unwrap_or(false);
        let mut cmd = if is_script {
            let mut c = Command::new("cmd");
            c.arg("/D").arg("/C").arg(bin.as_os_str());
            for a in args {
                c.arg(a);
            }
            c
        } else {
            let mut c = Command::new(bin);
            c.args(args);
            c
        };
        cmd.stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .creation_flags(CREATE_NO_WINDOW | DETACHED_PROCESS)
            .spawn()
            .map_err(|e| e.to_string())?;
        Ok(())
    }
    #[cfg(not(windows))]
    {
        Command::new(bin)
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}

/// Cheap reachability check: TCP connect to the gateway host:port derived from
/// the chat-completions URL.
fn probe_url(chat_completions_url: &str) -> bool {
    let Some((host, port)) = host_port(chat_completions_url) else {
        return false;
    };
    tcp_open(&host, port)
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

fn tcp_open(host: &str, port: u16) -> bool {
    use std::net::ToSocketAddrs;
    let Ok(addrs) = (host, port).to_socket_addrs() else {
        return false;
    };
    for addr in addrs {
        if TcpStream::connect_timeout(&addr, PROBE_TIMEOUT).is_ok() {
            return true;
        }
    }
    false
}

/// Re-export helper used by settings UI / tests.
#[allow(dead_code)]
pub fn wsl_ip() -> Option<String> {
    resolve_wsl_ipv4()
}
