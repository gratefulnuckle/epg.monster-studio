// SPDX-License-Identifier: GPL-3.0-or-later

//! GStreamer engine as a child process. Studio never links `gstreamer-rs`, so
//! an operator who skips the install tick can still launch the app.
//!
//! Line protocol on stdin/stdout:
//!   in:  `open <url>` / `ua <s>` / `header <Key> <Value>` / `play` / `pause <0|1>`
//!        / `mute <0|1>` / `volume <0..1>` / `rect <x> <y> <w> <h>` / `quit`
//!   out: `playing` / `paused` / `buffering <0-100>` / `error <msg>` / `eos`

use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;

use crate::engine::{Engine, EngineId, PlayRequest, PlayState};
use crate::paths;

pub struct GstSidecar {
    app_root: PathBuf,
    child: Option<Child>,
    stdin: Option<ChildStdin>,
    state: Arc<Mutex<PlayState>>,
}

impl GstSidecar {
    pub fn new(app_root: PathBuf) -> Self {
        Self {
            app_root,
            child: None,
            stdin: None,
            state: Arc::new(Mutex::new(PlayState::Stopped)),
        }
    }

    fn write_line(&mut self, line: &str) {
        if let Some(stdin) = self.stdin.as_mut() {
            let _ = writeln!(stdin, "{line}");
            let _ = stdin.flush();
        }
    }

    fn set_state(&self, s: PlayState) {
        if let Ok(mut g) = self.state.lock() {
            *g = s;
        }
    }
}

impl Drop for GstSidecar {
    fn drop(&mut self) {
        self.stop();
    }
}

impl Engine for GstSidecar {
    fn id(&self) -> EngineId {
        EngineId::Gstreamer
    }

    fn start(&mut self, req: &PlayRequest) -> Result<(), String> {
        self.stop();
        let exe = paths::ghoul_gst_exe(&self.app_root);
        if !exe.is_file() {
            self.set_state(PlayState::Error("GStreamer not installed".into()));
            return Err("GStreamer not installed".into());
        }
        if paths::find_gst_root(None, &self.app_root).is_none() {
            self.set_state(PlayState::Error("GStreamer not installed".into()));
            return Err("GStreamer not installed".into());
        }
        let mut cmd = Command::new(&exe);
        cmd.arg("--hwnd")
            .arg(req.pane.to_string())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        if let Some(dir) = exe.parent() {
            cmd.current_dir(dir);
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }
        let mut child = cmd
            .spawn()
            .map_err(|_| "GStreamer not installed".to_string())?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "GStreamer failed to start".to_string())?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| "GStreamer failed to start".to_string())?;
        let slot = Arc::clone(&self.state);
        let _ = thread::Builder::new()
            .name("ghoul-gst-out".into())
            .spawn(move || read_sidecar(stdout, slot));
        self.stdin = Some(stdin);
        self.child = Some(child);
        self.write_line(&format!("ua {}", req.ua));
        for (k, v) in req.extra_headers() {
            self.write_line(&format!("header {k} {v}"));
        }
        self.write_line(&format!("open {}", req.url));
        self.write_line("play");
        self.set_state(PlayState::Playing);
        Ok(())
    }

    fn stop(&mut self) {
        self.write_line("quit");
        self.stdin = None;
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        self.set_state(PlayState::Stopped);
    }

    fn pause(&mut self, paused: bool) {
        self.write_line(&format!("pause {}", if paused { 1 } else { 0 }));
        self.set_state(if paused {
            PlayState::Paused
        } else {
            PlayState::Playing
        });
    }

    fn mute(&mut self, muted: bool) {
        self.write_line(&format!("mute {}", if muted { 1 } else { 0 }));
    }

    fn volume(&mut self, vol: f64) {
        self.write_line(&format!("volume {}", vol.clamp(0.0, 1.0)));
    }

    fn status(&self) -> PlayState {
        self.state
            .lock()
            .ok()
            .map(|g| g.clone())
            .unwrap_or(PlayState::Stopped)
    }
}

fn read_sidecar(stdout: std::process::ChildStdout, slot: Arc<Mutex<PlayState>>) {
    let reader = BufReader::new(stdout);
    for line in reader.lines().map_while(Result::ok) {
        let line = line.trim();
        let next = if line.eq_ignore_ascii_case("playing") {
            PlayState::Playing
        } else if line.eq_ignore_ascii_case("paused") {
            PlayState::Paused
        } else if line.eq_ignore_ascii_case("eos") {
            PlayState::Stopped
        } else if let Some(rest) = line.strip_prefix("buffering ") {
            let n = rest.trim().parse::<u8>().unwrap_or(0);
            PlayState::Buffering(n.min(100))
        } else if let Some(rest) = line.strip_prefix("error ") {
            let msg = rest.trim();
            if msg.is_empty() {
                PlayState::Error("Stream failed".into())
            } else {
                PlayState::Error(msg.to_string())
            }
        } else {
            continue;
        };
        if let Ok(mut g) = slot.lock() {
            *g = next;
        }
    }
}

impl GstSidecar {
    pub fn set_rect(&mut self, x: i32, y: i32, w: i32, h: i32) {
        self.write_line(&format!("rect {x} {y} {w} {h}"));
    }
}
