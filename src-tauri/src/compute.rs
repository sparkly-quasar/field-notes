// SPDX-License-Identifier: LicenseRef-PolyForm-Noncommercial-1.0.0
//! A small watcher that answers one question before someone leans on the
//! Companion: does this machine actually have the memory to run the local model,
//! or is it about to swap, crawl, or fail?
//!
//! Two signals, combined into one plain verdict:
//! 1. **Static preflight — RAM.** Total and available memory (via `sysinfo`),
//!    weighed against the chosen model's resident footprint. Local LLMs are
//!    memory-bound; this is the gate that actually decides whether it runs.
//! 2. **Live footprint.** When Ollama already has the model loaded, `/api/ps`
//!    reports its resident size and how much sits on the GPU vs spilled to CPU.
//!    A model spilling to CPU is the practical face of "not enough compute": it
//!    runs, but slowly. Real numbers beat the estimate whenever we have them.
//!
//! Everything here is best-effort and read-only. It never blocks the Companion —
//! a machine that can't be measured gets an honest `Unknown`, not a refusal.

use serde::Serialize;

const BASE: &str = "http://127.0.0.1:11434";
const GB: f64 = 1024.0 * 1024.0 * 1024.0;

/// How much headroom over the model's own footprint we want free before calling
/// a machine comfortable: the OS, the app, and the browser engine all need room
/// too. A model that exactly fills RAM is a swap-storm waiting to happen.
const HEADROOM_GB: f64 = 1.5;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Verdict {
    /// Plenty of room — run without a second thought.
    Ample,
    /// It'll run, but it's close: expect slowness, or free some memory first.
    Tight,
    /// Not enough memory for this model on this machine — steer to a smaller one.
    Insufficient,
    /// Couldn't measure the machine (rare). Don't block on it.
    Unknown,
}

/// The model's live footprint, present only when Ollama has it resident.
#[derive(Debug, Clone, Serialize)]
pub struct LoadedModel {
    /// Total resident size right now.
    pub resident_gb: f64,
    /// Portion held in GPU/accelerator memory.
    pub gpu_gb: f64,
    /// Portion held in ordinary RAM. Large while a GPU is present means the model
    /// spilled off the accelerator — the slow path.
    pub cpu_gb: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ComputeStatus {
    pub verdict: Verdict,
    pub total_ram_gb: f64,
    pub available_ram_gb: f64,
    /// The footprint we measured against: the live resident size if the model is
    /// loaded, otherwise an estimate from its size class.
    pub required_gb: f64,
    pub cpu_cores: usize,
    pub arch: String,
    pub os: String,
    pub loaded: Option<LoadedModel>,
    /// One plain sentence for the UI to show as-is.
    pub message: String,
}

/// When the machine can't be read at all — memory query returned nothing.
pub fn unknown() -> ComputeStatus {
    ComputeStatus {
        verdict: Verdict::Unknown,
        total_ram_gb: 0.0,
        available_ram_gb: 0.0,
        required_gb: 0.0,
        cpu_cores: 0,
        arch: std::env::consts::ARCH.to_string(),
        os: std::env::consts::OS.to_string(),
        loaded: None,
        message: "Couldn't read this machine's memory, so I can't tell whether the model \
                  will run comfortably. If replies are very slow, try a smaller model."
            .into(),
    }
}

/// Estimated resident footprint (GB) for a model tag, when it isn't loaded and we
/// can't read the real size. Keyed on the parameter-count hint in the tag; these
/// track a 4-bit-quantized model's weights-plus-context, which is what Ollama
/// actually holds — not the download size. Falls back to the app's 8B default tier.
fn estimated_gb(model: &str) -> f64 {
    let m = model.to_lowercase();
    // Most specific / largest hints first so ":1b" doesn't shadow ":13b".
    for (needle, gb) in [
        (":70b", 42.0),
        (":34b", 22.0),
        (":32b", 20.0),
        (":30b", 20.0),
        (":14b", 10.0),
        (":13b", 10.0),
        (":8b", 6.0),
        (":7b", 6.0),
        (":4b", 4.0),
        (":3b", 3.0),
        (":2b", 2.5),
        (":1b", 1.5),
        (":0.5b", 1.0),
    ] {
        if m.contains(needle) || m.contains(&needle.replace(':', "-")) {
            return gb;
        }
    }
    6.0
}

/// The model's live footprint from Ollama's `/api/ps`, if it's loaded. Best-effort:
/// any error (Ollama down, model not resident, unexpected shape) yields `None`.
fn loaded_model(model: &str) -> Option<LoadedModel> {
    let resp = ureq::get(&format!("{BASE}/api/ps"))
        .timeout(std::time::Duration::from_millis(600))
        .call()
        .ok()?;
    let v: serde_json::Value = resp.into_json().ok()?;
    let models = v.get("models")?.as_array()?;
    // Match the exact tag, tolerating Ollama's habit of defaulting ":latest".
    let want = model.to_lowercase();
    let entry = models.iter().find(|m| {
        m.get("name")
            .and_then(|n| n.as_str())
            .map(|n| {
                let n = n.to_lowercase();
                n == want || n.trim_end_matches(":latest") == want.trim_end_matches(":latest")
            })
            .unwrap_or(false)
    })?;
    let size = entry.get("size").and_then(|s| s.as_f64()).unwrap_or(0.0);
    let vram = entry.get("size_vram").and_then(|s| s.as_f64()).unwrap_or(0.0);
    let cpu = (size - vram).max(0.0);
    Some(LoadedModel {
        resident_gb: round1(size / GB),
        gpu_gb: round1(vram / GB),
        cpu_gb: round1(cpu / GB),
    })
}

/// Read the machine and judge whether `model` can run comfortably on it.
pub fn status(model: &str) -> ComputeStatus {
    use sysinfo::System;
    let mut sys = System::new();
    sys.refresh_memory();
    let total = sys.total_memory() as f64 / GB;
    if total <= 0.0 {
        return unknown();
    }
    let available = sys.available_memory() as f64 / GB;
    let cpu_cores = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(0);

    let loaded = loaded_model(model);
    // A loaded model's real resident size beats any estimate.
    let required = loaded.as_ref().map(|l| l.resident_gb).unwrap_or_else(|| estimated_gb(model));

    let verdict = decide(total, available, required, loaded.as_ref());
    let message = phrasing(verdict, total, available, required, loaded.as_ref());

    ComputeStatus {
        verdict,
        total_ram_gb: round1(total),
        available_ram_gb: round1(available),
        required_gb: round1(required),
        cpu_cores,
        arch: std::env::consts::ARCH.to_string(),
        os: std::env::consts::OS.to_string(),
        loaded,
        message,
    }
}

fn decide(total: f64, available: f64, required: f64, loaded: Option<&LoadedModel>) -> Verdict {
    // If it's already resident, the machine is by definition holding it. The only
    // question left is whether it spilled off the GPU onto the CPU (the slow path).
    if let Some(l) = loaded {
        // Spill is only meaningful when some of it *did* fit on a GPU; an all-CPU
        // load on a GPU-less machine (Apple unified memory reports vram=0 too) is
        // the expected, fine case, judged by RAM below instead.
        if l.gpu_gb > 0.0 && l.cpu_gb > 0.5 {
            return Verdict::Tight;
        }
        // Loaded and not spilling — but if free RAM is now razor-thin, still tight.
        return if available < HEADROOM_GB { Verdict::Tight } else { Verdict::Ample };
    }
    if total < required {
        return Verdict::Insufficient;
    }
    if available < required + HEADROOM_GB {
        return Verdict::Tight;
    }
    Verdict::Ample
}

fn phrasing(v: Verdict, total: f64, available: f64, required: f64, loaded: Option<&LoadedModel>) -> String {
    let t = round1(total);
    let a = round1(available);
    let r = round1(required);
    match v {
        Verdict::Ample if loaded.is_some() => format!(
            "The model is loaded and running comfortably ({r} GB resident, {a} GB free)."
        ),
        Verdict::Ample => format!(
            "Your machine has {t} GB of memory ({a} GB free now) — plenty for a model this size (~{r} GB)."
        ),
        Verdict::Tight if loaded.as_ref().map(|l| l.gpu_gb > 0.0 && l.cpu_gb > 0.5).unwrap_or(false) => {
            let l = loaded.unwrap();
            format!(
                "The model is running but part of it ({:.1} GB) spilled onto the CPU, so replies \
                 will be slow. Closing other apps or using a smaller model would help.",
                l.cpu_gb
            )
        }
        Verdict::Tight => format!(
            "This is close: the model needs about {r} GB and only {a} GB is free right now (of {t} GB). \
             It should run, but closing other apps first will keep it responsive."
        ),
        Verdict::Insufficient => format!(
            "This machine has {t} GB of memory, and a model this size needs about {r} GB to run — \
             it won't fit. Pick a smaller model (e.g. a 3B) in setup."
        ),
        Verdict::Unknown => unknown().message,
    }
}

fn round1(x: f64) -> f64 {
    (x * 10.0).round() / 10.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_plausible_machine() {
        // This test runs on a real machine; total memory should be a sane number
        // of GB (bytes → GB), catching a units regression (KB vs bytes) loudly.
        let s = status("qwen3:8b");
        assert!(s.total_ram_gb > 0.5, "total looks wrong: {}", s.total_ram_gb);
        assert!(s.total_ram_gb < 8192.0, "total looks wrong: {}", s.total_ram_gb);
        assert!(!s.message.is_empty());
    }

    #[test]
    fn estimates_scale_with_size() {
        assert!(estimated_gb("llama3.2:3b") < estimated_gb("qwen3:8b"));
        assert!(estimated_gb("qwen3:8b") < estimated_gb("qwen3:32b"));
        // Size hints work with a dash too, and unknown tags fall back to the 8B tier.
        assert_eq!(estimated_gb("some-1b-thing"), 1.5);
        assert_eq!(estimated_gb("mystery-model"), 6.0);
    }

    #[test]
    fn a_loaded_model_that_fits_is_ample() {
        let fits = LoadedModel { resident_gb: 6.0, gpu_gb: 6.0, cpu_gb: 0.0 };
        assert_eq!(decide(16.0, 8.0, 6.0, Some(&fits)), Verdict::Ample);
    }

    #[test]
    fn a_loaded_model_spilling_to_cpu_is_tight() {
        let spill = LoadedModel { resident_gb: 6.0, gpu_gb: 4.0, cpu_gb: 2.0 };
        assert_eq!(decide(16.0, 8.0, 6.0, Some(&spill)), Verdict::Tight);
    }

    #[test]
    fn too_little_total_ram_is_insufficient() {
        assert_eq!(decide(4.0, 3.0, 6.0, None), Verdict::Insufficient);
    }

    #[test]
    fn enough_total_but_little_free_is_tight() {
        assert_eq!(decide(16.0, 5.0, 6.0, None), Verdict::Tight);
        assert_eq!(decide(16.0, 12.0, 6.0, None), Verdict::Ample);
    }
}
