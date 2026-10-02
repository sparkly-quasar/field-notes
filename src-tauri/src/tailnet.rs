// SPDX-License-Identifier: LicenseRef-PolyForm-Noncommercial-1.0.0
//! The Tailscale that ships inside Field Notes.
//!
//! `fieldnotes-tailnet` (Go, in `tailnet/`) is Tailscale's own `tsnet` library
//! wrapped in a small program. It joins the user's tailnet as its own device,
//! named `field-notes`, and answers HTTPS there by forwarding to the portal on
//! loopback. So a new user installs nothing on the computer: they sign in once
//! in the browser, and install Tailscale only on the phone.
//!
//! It changes how the portal is *reached*, not what it allows. The portal still
//! binds 127.0.0.1 and still checks every request's token, the lock and the
//! allowlist (`portal.rs`, rules 1–4). The helper:
//! - listens only on the tailnet (tsnet listeners can't be reached any other way),
//! - forwards only to 127.0.0.1 (it refuses any other target),
//! - never uses Funnel, so nothing is published to the internet (a test below
//!   reads its source to keep it that way),
//! - sends no logs to Tailscale (`TS_NO_LOGS_NO_SUPPORT`).
//!
//! Its state (this device's keys) lives in `tailnet/` beside the journal, so a
//! restart keeps it signed in, and "Erase all data" removes it.
//!
//! Protocol: one JSON object per line on the helper's stdout ([`HelperStatus`]);
//! `logout` on its stdin signs out. Closing stdin stops it, so it can't outlive
//! the app.

use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// The sidecar's file name. Tauri bundles it next to the app's own executable.
#[cfg(windows)]
const HELPER: &str = "fieldnotes-tailnet.exe";
#[cfg(not(windows))]
const HELPER: &str = "fieldnotes-tailnet";

/// The name this computer gets on the tailnet. Tailscale adds `-1`, `-2`… if the
/// account already has one (a second computer running Field Notes).
const HOSTNAME: &str = "field-notes";

/// What the helper last reported. Field names match `Status` in `tailnet/main.go`.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct HelperStatus {
    /// `Off` (not running), `Starting`, `NeedsLogin`, `NeedsMachineAuth`,
    /// `Running`, `Stopped`, or `Error`.
    pub state: String,
    #[serde(default)]
    pub auth_url: Option<String>,
    #[serde(default)]
    pub login: Option<String>,
    #[serde(default)]
    pub dns_name: Option<String>,
    #[serde(default)]
    pub https: bool,
    #[serde(default)]
    pub serving: bool,
    #[serde(default)]
    pub error: Option<String>,
}

impl HelperStatus {
    fn off() -> Self {
        HelperStatus { state: "Off".into(), ..Default::default() }
    }
    pub fn signed_in(&self) -> bool {
        self.state == "Running"
    }
}

struct Helper {
    child: Child,
    stdin: Option<ChildStdin>,
    target: u16,
}

pub struct Tailnet {
    /// Where this device's Tailscale state lives: `<app data>/tailnet`.
    dir: PathBuf,
    helper: Mutex<Option<Helper>>,
    status: Arc<Mutex<HelperStatus>>,
}

impl Tailnet {
    pub fn new(app_data: &Path) -> Self {
        Tailnet {
            dir: app_data.join("tailnet"),
            helper: Mutex::new(None),
            status: Arc::new(Mutex::new(HelperStatus::off())),
        }
    }

    /// Where the bundled helper is, if this build has one.
    pub fn helper_path() -> Option<PathBuf> {
        let exe = std::env::current_exe().ok()?;
        let dir = exe.parent()?;
        let p = dir.join(HELPER);
        if p.is_file() {
            return Some(p);
        }
        // Test binaries run from target/<profile>/deps; Tauri puts the sidecar
        // one level up, next to the app.
        #[cfg(test)]
        {
            let p = dir.parent()?.join(HELPER);
            if p.is_file() {
                return Some(p);
            }
        }
        None
    }

    pub fn status(&self) -> HelperStatus {
        let mut h = self.helper.lock().unwrap();
        // Notice a helper that exited on its own (crash, or after `logout`).
        if let Some(helper) = h.as_mut() {
            if let Ok(Some(_)) = helper.child.try_wait() {
                *h = None;
                let mut s = self.status.lock().unwrap();
                if s.state != "Error" {
                    *s = HelperStatus::off();
                }
                return s.clone();
            }
        }
        if h.is_none() {
            let s = self.status.lock().unwrap();
            // Keep a final error visible; otherwise it's simply off.
            return if s.state == "Error" { s.clone() } else { HelperStatus::off() };
        }
        self.status.lock().unwrap().clone()
    }

    pub fn running(&self) -> bool {
        self.status().state != "Off" && self.helper.lock().unwrap().is_some()
    }

    /// Start the helper, forwarding to the portal on `port`. Already running for
    /// that port: nothing to do. Running for another port (the portal restarted
    /// elsewhere in its range): restart it, keeping the same tailnet identity.
    pub fn start(&self, port: u16) -> Result<(), String> {
        {
            let h = self.helper.lock().unwrap();
            if h.as_ref().is_some_and(|h| h.target == port) {
                drop(h);
                if self.running() {
                    return Ok(());
                }
            }
        }
        self.stop();

        let bin = Self::helper_path().ok_or(
            "This copy of Field Notes doesn't include its built-in Tailscale. \
             Reinstall it, or use the Tailscale app on this computer instead.",
        )?;
        std::fs::create_dir_all(&self.dir).map_err(|e| format!("Couldn't create the Tailscale folder: {e}"))?;

        let mut cmd = Command::new(bin);
        cmd.arg("--dir")
            .arg(&self.dir)
            .arg("--hostname")
            .arg(HOSTNAME)
            .arg("--target")
            .arg(format!("127.0.0.1:{port}"))
            // Belt and braces: the helper sets this itself before starting.
            .env("TS_NO_LOGS_NO_SUPPORT", "true")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }
        let mut child = cmd.spawn().map_err(|e| format!("Couldn't start the built-in Tailscale: {e}"))?;

        *self.status.lock().unwrap() = HelperStatus { state: "Starting".into(), ..Default::default() };
        let stdout = child.stdout.take().ok_or("Couldn't read from the built-in Tailscale.")?;
        let status = self.status.clone();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if let Some(s) = parse_line(&line) {
                    *status.lock().unwrap() = s;
                }
            }
        });

        let stdin = child.stdin.take();
        *self.helper.lock().unwrap() = Some(Helper { child, stdin, target: port });
        Ok(())
    }

    /// Stop the helper. The tailnet identity stays on disk, so starting again
    /// comes back as the same device, already signed in.
    pub fn stop(&self) {
        let Some(mut h) = self.helper.lock().unwrap().take() else { return };
        drop(h.stdin.take()); // EOF: the helper shuts down cleanly
        wait_or_kill(&mut h.child, Duration::from_secs(5));
        *self.status.lock().unwrap() = HelperStatus::off();
    }

    /// Sign this computer out of Tailscale and forget it: the device is removed
    /// from the user's tailnet and its keys are deleted here.
    pub fn sign_out(&self) -> Result<(), String> {
        let taken = self.helper.lock().unwrap().take();
        if let Some(mut h) = taken {
            if let Some(stdin) = h.stdin.as_mut() {
                let _ = stdin.write_all(b"logout\n");
                let _ = stdin.flush();
            }
            wait_or_kill(&mut h.child, Duration::from_secs(20));
        }
        *self.status.lock().unwrap() = HelperStatus::off();
        self.forget_files()
    }

    /// Stop, and delete this device's Tailscale state. For "Erase all data".
    pub fn forget(&self) {
        self.stop();
        let _ = self.forget_files();
    }

    fn forget_files(&self) -> Result<(), String> {
        match std::fs::remove_dir_all(&self.dir) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(format!("Couldn't remove the Tailscale folder: {e}")),
        }
    }
}

impl Drop for Tailnet {
    fn drop(&mut self) {
        self.stop();
    }
}

fn wait_or_kill(child: &mut Child, grace: Duration) {
    let until = Instant::now() + grace;
    while Instant::now() < until {
        if let Ok(Some(_)) = child.try_wait() {
            return;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    let _ = child.kill();
    let _ = child.wait();
}

fn parse_line(line: &str) -> Option<HelperStatus> {
    serde_json::from_str(line.trim()).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_helpers_status_lines() {
        let s = parse_line(r#"{"state":"NeedsLogin","auth_url":"https://login.tailscale.com/a/abc","https":false,"serving":false}"#).unwrap();
        assert_eq!(s.state, "NeedsLogin");
        assert_eq!(s.auth_url.as_deref(), Some("https://login.tailscale.com/a/abc"));
        assert!(!s.signed_in());

        let s = parse_line(r#"{"state":"Running","login":"you@example.com","dns_name":"field-notes.tail1.ts.net","https":true,"serving":true}"#).unwrap();
        assert!(s.signed_in() && s.https && s.serving);
        assert_eq!(s.login.as_deref(), Some("you@example.com"));

        assert!(parse_line("not json").is_none());
    }

    /// Rule 1's guarantee now rests partly on the helper: it must never publish to
    /// the internet, and never answer anywhere but the tailnet.
    #[test]
    fn the_helper_never_uses_funnel_or_an_os_socket() {
        let src = include_str!("../../tailnet/main.go");
        let code: String = src
            .lines()
            .filter(|l| !l.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(!code.contains("Funnel"), "the helper must never use Tailscale Funnel");
        assert!(!code.contains("net.Listen("), "the helper must listen only through tsnet");
        assert!(code.contains("SetNoLogsNoSupport()"), "the helper must not upload logs");
        assert!(code.contains(r#"host != "127.0.0.1""#), "the helper must forward only to loopback");
    }

    /// Drives the real bundled helper through this module: it starts, reports
    /// over the pipe, stops when told, and signing out deletes its keys. It can't
    /// get as far as signed in without a Tailscale account, so it stops at
    /// "waiting for sign-in" (or "starting", where the network is slow).
    #[test]
    fn drives_the_real_helper() {
        if Tailnet::helper_path().is_none() {
            eprintln!("skipped: run scripts/build-tailnet.sh and build first");
            return;
        }
        let dir = std::env::temp_dir().join(format!("fn-tailnet-live-{}", std::process::id()));
        let t = Tailnet::new(&dir);
        t.start(8787).unwrap();
        assert!(t.running());

        let until = Instant::now() + Duration::from_secs(20);
        let mut seen = t.status();
        while seen.state == "Starting" && Instant::now() < until {
            std::thread::sleep(Duration::from_millis(200));
            seen = t.status();
        }
        eprintln!("helper reported: {}", seen.state);
        assert!(["Starting", "NeedsLogin"].contains(&seen.state.as_str()), "unexpected: {seen:?}");
        assert!(!seen.serving);
        assert!(dir.join("tailnet").is_dir());

        // Same port: already running, nothing restarts.
        t.start(8787).unwrap();
        assert!(t.running());

        t.stop();
        assert_eq!(t.status().state, "Off");
        assert!(dir.join("tailnet").is_dir(), "stopping keeps the device's keys");

        t.start(8788).unwrap();
        t.sign_out().unwrap();
        assert_eq!(t.status().state, "Off");
        assert!(!dir.join("tailnet").exists(), "signing out deletes the keys");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_never_started_helper_reports_off() {
        let dir = std::env::temp_dir().join(format!("fn-tailnet-{}", std::process::id()));
        let t = Tailnet::new(&dir);
        assert_eq!(t.status().state, "Off");
        assert!(!t.running());
        t.forget(); // removing a folder that isn't there is fine
    }
}
