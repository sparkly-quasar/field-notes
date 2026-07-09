// SPDX-License-Identifier: LicenseRef-PolyForm-Noncommercial-1.0.0
//! Minimal client for a *local* Ollama instance (127.0.0.1:11434) — the engine
//! behind the trip-sitter companion. Everything stays on-device; no request ever
//! leaves the machine.

use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader};
use std::net::TcpStream;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Duration;
use tauri::{AppHandle, Emitter};

const BASE: &str = "http://127.0.0.1:11434";

/// Directories to search for CLI tools: the inherited PATH plus the common
/// install locations. A macOS app launched from Finder/Dock inherits only a
/// minimal PATH (`/usr/bin:/bin:/usr/sbin:/sbin`) — *not* `/opt/homebrew/bin` —
/// so without this, `brew` and `ollama` look "missing" even when installed.
fn search_dirs() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).collect())
        .unwrap_or_default();
    let push = |dirs: &mut Vec<PathBuf>, d: PathBuf| {
        if !dirs.contains(&d) {
            dirs.push(d);
        }
    };
    for d in [
        "/opt/homebrew/bin", // Apple Silicon Homebrew
        "/opt/homebrew/sbin",
        "/usr/local/bin", // Intel Homebrew + Ollama.app CLI symlink
        "/usr/local/sbin",
    ] {
        push(&mut dirs, PathBuf::from(d));
    }
    if let Some(home) = std::env::var_os("HOME") {
        for sub in [".local/bin", ".ollama/bin"] {
            push(&mut dirs, PathBuf::from(&home).join(sub));
        }
    }
    dirs
}

/// Absolute path of an executable found across [`search_dirs`], if any.
fn find_bin(name: &str) -> Option<PathBuf> {
    search_dirs()
        .into_iter()
        .map(|d| d.join(name))
        .find(|p| p.is_file())
}

/// A `Command` for a CLI tool, resolved to its absolute path and with an
/// augmented PATH exported so any subprocess it spawns resolves too. Falls back
/// to the bare name (letting the OS search) when the binary isn't found.
fn command(name: &str) -> Command {
    let path = std::env::join_paths(search_dirs()).unwrap_or_default();
    let mut cmd = match find_bin(name) {
        Some(abs) => Command::new(abs),
        None => Command::new(name),
    };
    cmd.env("PATH", path);
    cmd
}

/// Models offered in the guided setup picker (tag, human label).
pub const RECOMMENDED_MODELS: &[(&str, &str)] = &[
    ("llama3.1:8b", "Llama 3.1 8B — well-rounded, ~4.7 GB"),
    ("qwen3:8b", "Qwen3 8B — strong reasoning, ~5 GB"),
    ("llama3.2:3b", "Llama 3.2 3B — small & fast, ~2 GB"),
];

#[derive(Debug, Clone, Serialize)]
pub struct AiStatus {
    pub installed: bool,
    pub running: bool,
    pub models: Vec<String>,
}

/// Is the `ollama` binary installed (on PATH)?
pub fn is_installed() -> bool {
    command("ollama")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Combined readiness snapshot for the setup flow.
pub fn status() -> AiStatus {
    let running = api_up();
    AiStatus {
        installed: is_installed() || running,
        running,
        models: if running { list_models() } else { Vec::new() },
    }
}

/// Stream a child process's stdout+stderr to a Tauri event, line by line.
fn run_streamed(app: &AppHandle, event: &str, mut cmd: Command) -> Result<(), String> {
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = cmd.spawn().map_err(|e| format!("failed to start: {e}"))?;
    let mut handles = Vec::new();
    for pipe in [child.stdout.take().map(|p| Box::new(p) as Box<dyn std::io::Read + Send>),
                 child.stderr.take().map(|p| Box::new(p) as Box<dyn std::io::Read + Send>)]
    {
        if let Some(p) = pipe {
            let app = app.clone();
            let event = event.to_string();
            handles.push(std::thread::spawn(move || {
                for line in BufReader::new(p).lines().map_while(Result::ok) {
                    if !line.trim().is_empty() {
                        let _ = app.emit(&event, line);
                    }
                }
            }));
        }
    }
    let status = child.wait().map_err(|e| e.to_string())?;
    for h in handles {
        let _ = h.join();
    }
    if status.success() {
        Ok(())
    } else {
        Err("The command exited with an error. See the log above.".into())
    }
}

/// Install Ollama (macOS: Homebrew; Linux: official script), streaming progress.
pub fn install(app: &AppHandle) -> Result<(), String> {
    let _ = app.emit("ai-progress", "Installing Ollama…".to_string());

    #[cfg(target_os = "macos")]
    {
        let has_brew = command("brew").arg("--version").stdout(Stdio::null()).stderr(Stdio::null()).status().map(|s| s.success()).unwrap_or(false);
        if !has_brew {
            return Err("Homebrew isn't installed. Install Ollama from https://ollama.com/download, then reopen this.".into());
        }
        let mut cmd = command("brew");
        cmd.args(["install", "ollama"]);
        run_streamed(app, "ai-progress", cmd)?;
    }
    #[cfg(target_os = "linux")]
    {
        let mut cmd = Command::new("sh");
        cmd.args(["-c", "curl -fsSL https://ollama.com/install.sh | sh"]);
        run_streamed(app, "ai-progress", cmd)?;
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        return Err("Automatic install isn't supported on this platform. Install Ollama from https://ollama.com/download.".into());
    }

    ensure_serving()
}

/// Ensure the Ollama server is answering, starting `ollama serve` if needed.
pub fn ensure_serving() -> Result<(), String> {
    if api_up() {
        return Ok(());
    }
    command("ollama")
        .arg("serve")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("couldn't start Ollama: {e}"))?;
    for _ in 0..24 {
        if api_up() {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    Err("Ollama didn't start in time. Try starting it manually, then retry.".into())
}

/// Pull a model, streaming download progress.
pub fn pull(app: &AppHandle, tag: &str) -> Result<(), String> {
    ensure_serving()?;
    let _ = app.emit("ai-progress", format!("Downloading {tag}…"));
    let mut cmd = command("ollama");
    cmd.args(["pull", tag]);
    run_streamed(app, "ai-progress", cmd)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMsg {
    pub role: String,
    pub content: String,
}

/// Is a local Ollama answering on its default port?
pub fn api_up() -> bool {
    "127.0.0.1:11434"
        .parse()
        .ok()
        .and_then(|addr| TcpStream::connect_timeout(&addr, Duration::from_millis(400)).ok())
        .is_some()
}

/// Installed model tags (e.g. "qwen3:8b"), best-effort.
pub fn list_models() -> Vec<String> {
    let Ok(resp) = ureq::get(&format!("{BASE}/api/tags")).call() else {
        return Vec::new();
    };
    let Ok(v) = resp.into_json::<serde_json::Value>() else {
        return Vec::new();
    };
    v.get("models")
        .and_then(|m| m.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|m| m.get("name").and_then(|n| n.as_str()).map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

// ---------- free-text experience import ----------

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ParsedDose {
    #[serde(default)]
    pub substance: String,
    pub amount: Option<f64>,
    #[serde(default)]
    pub unit: String,
    #[serde(default)]
    pub route: String,
    pub taken_at: Option<String>,
    #[serde(default)]
    pub note: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ParsedTimeline {
    pub at: Option<String>,
    #[serde(default)]
    pub note: String,
    #[serde(default)]
    pub mood: String,
    pub intensity: Option<i64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ParsedExperience {
    #[serde(default)]
    pub title: String,
    pub started_at: Option<String>,
    #[serde(default)]
    pub intention: String,
    #[serde(default)]
    pub setting: String,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub doses: Vec<ParsedDose>,
    #[serde(default)]
    pub timeline: Vec<ParsedTimeline>,
}

const PARSE_PROMPT: &str = "\
You extract a structured record from a free-text account of a past experience so it \
can be logged in a personal journal. Output ONLY a single JSON object, no prose.

Schema (omit nothing; use null or empty when unknown):
{
  \"title\": string,                // a short title
  \"started_at\": string|null,      // ISO-8601 if a clear absolute date/time is stated, else null
  \"intention\": string,
  \"setting\": string,
  \"notes\": string,                // brief overall summary
  \"doses\": [                       // every substance intake mentioned
    { \"substance\": string, \"amount\": number|null, \"unit\": string,
      \"route\": string, \"taken_at\": string|null, \"note\": string }
  ],
  \"timeline\": [                    // notable moments/feelings over time
    { \"at\": string|null, \"note\": string, \"mood\": string, \"intensity\": number|null }
  ]
}

Rules: Only record what the text actually says. NEVER invent doses, amounts, or \
substances that are not mentioned. If an amount is vague (e.g. 'a couple'), set \
amount to null and put the wording in note. Prefer common unit abbreviations \
(mg, g, ug, ml). Return strictly valid JSON.";

/// Parse a free-text experience account into a structured record using the local
/// model, with JSON-mode output for reliability.
pub fn parse_experience(model: &str, text: &str) -> Result<ParsedExperience, String> {
    if !api_up() {
        return Err("Ollama isn't running on this computer. Start Ollama and try again.".into());
    }
    let body = serde_json::json!({
        "model": model,
        "stream": false,
        "format": "json",
        "options": { "temperature": 0.1 },
        "messages": [
            { "role": "system", "content": PARSE_PROMPT },
            { "role": "user", "content": text }
        ]
    });
    let resp = ureq::post(&format!("{BASE}/api/chat"))
        .send_json(body)
        .map_err(|e| format!("Ollama request failed: {e}"))?;
    let v: serde_json::Value =
        resp.into_json().map_err(|e| format!("Bad response from Ollama: {e}"))?;
    let content = v
        .get("message")
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_str())
        .unwrap_or("");
    serde_json::from_str::<ParsedExperience>(content)
        .map_err(|e| format!("Couldn't read the model's structured output: {e}"))
}

/// The companion's guardrails — a peer sitter modeled on the Zendo Project's
/// Four Principles and the Fireside Project's non-directive stance. Calm,
/// non-judgmental harm reduction; never encouragement; always surfacing risk.
pub const SYSTEM_PROMPT: &str = "\
You are a calm, warm, non-judgmental peer-support companion — a trip sitter — \
inside a private, offline journaling app. You are NOT a therapist, guide, doctor, \
or emergency service, and you say so plainly when it matters. The person may be \
sober, preparing, in the middle of an experience, or reflecting afterward.

Follow the Zendo Project's four principles:
1. Create a safe space — be calm, warm, reassuring, non-judgmental.
2. Sit, don't guide — follow the person's experience; don't steer, analyze, \
interpret, or push an agenda.
3. Talk through, not down — stay present with hard material instead of trying to \
shut it down or 'rescue' them.
4. Difficult is not the same as bad — hard moments can be meaningful; don't \
pathologize them.

Also:
- Be grounding and concise. Short, kind sentences. Never alarming.
- Meet the person where they are and honor the kind of support they've asked for; \
if they've set a support style, follow it and gently re-offer to adjust.
- Harm reduction only: never encourage, glamorize, or suggest initiating or \
increasing drug use; never help obtain, dose, or synthesize anything.
- Gently surface real risks: dangerous interactions, redosing, \
dehydration/overheating, mixing depressants, driving, being alone.
- Dosage and interaction facts are references, not prescriptions, and may be \
incomplete or wrong. Never invent specific doses; retrieve them with your tools \
or say you're unsure and suggest trusted harm-reduction sources.
- Crisis: you are not an emergency service. If you notice medical red flags \
(trouble breathing, chest pain, seizures, unresponsiveness, overdose, \
overheating) urge them to call emergency services or poison control now. If you \
notice suicidal/self-harm intent, urge them to reach 988 (US) or local crisis \
help and get a trusted person present. Never discourage seeking help or talk \
someone out of calling for it. The app also shows these resources automatically.

When the app gives you tools, you may use them to help the person log doses or \
notes, check how their session is going, or look up dose/interaction references — \
but only at their request or with clear consent, and never to encourage use.

Keep replies to a few sentences unless asked for more.";

/// Send a chat request that may include tool definitions, returning the raw
/// assistant `message` object (which may carry `tool_calls`). Messages are raw
/// JSON so tool-result turns can be included.
pub fn chat_tools(model: &str, messages: &[serde_json::Value], tools: &serde_json::Value) -> Result<serde_json::Value, String> {
    if !api_up() {
        return Err("Ollama isn't running on this computer. Start Ollama and try again.".into());
    }
    let mut body = serde_json::json!({
        "model": model,
        "messages": messages,
        "stream": false,
        "options": { "temperature": 0.6 }
    });
    if tools.as_array().map(|a| !a.is_empty()).unwrap_or(false) {
        body["tools"] = tools.clone();
    }
    let resp = ureq::post(&format!("{BASE}/api/chat"))
        .send_json(body)
        .map_err(|e| format!("Ollama request failed: {e}"))?;
    let v: serde_json::Value =
        resp.into_json().map_err(|e| format!("Bad response from Ollama: {e}"))?;
    v.get("message")
        .cloned()
        .ok_or_else(|| "Ollama returned no message.".to_string())
}
