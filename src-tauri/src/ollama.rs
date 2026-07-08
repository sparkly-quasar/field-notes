// SPDX-License-Identifier: LicenseRef-PolyForm-Noncommercial-1.0.0
//! Minimal client for a *local* Ollama instance (127.0.0.1:11434) — the engine
//! behind the trip-sitter companion. Everything stays on-device; no request ever
//! leaves the machine.

use serde::{Deserialize, Serialize};
use std::net::TcpStream;
use std::time::Duration;

const BASE: &str = "http://127.0.0.1:11434";

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

/// Send a chat completion and return the assistant's reply (non-streaming).
pub fn chat(model: &str, messages: &[ChatMsg]) -> Result<String, String> {
    if !api_up() {
        return Err("Ollama isn't running on this computer. Start Ollama and try again.".into());
    }
    let body = serde_json::json!({
        "model": model,
        "messages": messages,
        "stream": false,
        "options": { "temperature": 0.6 }
    });
    let resp = ureq::post(&format!("{BASE}/api/chat"))
        .send_json(body)
        .map_err(|e| format!("Ollama request failed: {e}"))?;
    let v: serde_json::Value =
        resp.into_json().map_err(|e| format!("Bad response from Ollama: {e}"))?;
    Ok(v.get("message")
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_str())
        .unwrap_or("")
        .to_string())
}

/// The companion's guardrails. Calm, non-judgmental harm reduction — never
/// encouragement, always surfacing risk.
pub const SYSTEM_PROMPT: &str = "\
You are a calm, warm, non-judgmental harm-reduction companion inside a private, \
offline journaling app. The person may be sober, preparing, in the middle of an \
experience, or reflecting afterward.

Your role:
- Be grounding, reassuring, and concise. Short, kind sentences. Never alarming.
- You practice harm reduction. You do NOT encourage, glamorize, or suggest \
initiating or increasing any drug use, and you never help obtain or synthesize \
anything.
- Proactively surface real safety risks: dangerous interactions, redosing, \
dehydration/overheating, mixing depressants, driving, being alone.
- You are NOT a medical professional and must say so when it matters. For \
anything worrying — trouble breathing, chest pain, seizures, unresponsiveness, \
severe distress — tell them to contact emergency services or poison control now.
- Dosage and interaction details are references, not prescriptions, and may be \
incomplete or wrong. Never invent specific doses; if unsure, say so and suggest \
checking trusted harm-reduction sources.
- If the app provides current session context, use it, but never scold.

Keep replies to a few sentences unless asked for more.";
