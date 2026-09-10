// SPDX-License-Identifier: GPL-3.0-or-later

//! Bundled `mpv.exe` driven over `--input-ipc-server` JSON IPC, `--wid` into
//! the video pane.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use crate::engine::{Engine, EngineId, PlayRequest, PlayState};
use crate::paths;

pub struct MpvIpc {
    app_root: PathBuf,
    child: Option<Child>,
    ipc: Option<std::fs::File>,
    ipc_path: Option<PathBuf>,
    state: PlayState,
}

impl MpvIpc {
    pub fn new(app_root: PathBuf) -> Self {
        Self {
            app_root,
            child: None,
            ipc: None,
            ipc_path: None,
            state: PlayState::Stopped,
        }
    }

    fn send(&mut self, cmd: serde_json::Value) {
        if let Some(f) = self.ipc.as_mut() {
            let line = format!("{cmd}\n");
            let _ = f.write_all(line.as_bytes());
            let _ = f.flush();
        }
    }
}

impl Drop for MpvIpc {
    fn drop(&mut self) {
        self.stop();
    }
}

impl Engine for MpvIpc {
    fn id(&self) -> EngineId {
        EngineId::MpvIpc
    }

    fn start(&mut self, req: &PlayRequest) -> Result<(), String> {
        self.stop();
        let exe = paths::mpv_exe(&self.app_root);
        if !exe.is_file() {
            self.state = PlayState::Error("mpv.exe not found".into());
            return Err("mpv.exe not found".into());
        }
        let (ipc_arg, ipc_path) = ipc_endpoint();
        let mut args = vec![
            format!("--input-ipc-server={ipc_arg}"),
            "--force-window=yes".into(),
            "--keep-open=yes".into(),
            "--no-terminal".into(),
            "--ytdl=no".into(),
            format!("--user-agent={}", req.ua),
        ];
        if req.pane != 0 {
            args.push(format!("--wid={}", req.pane));
        }
        let fields = req.http_header_fields();
        if !fields.is_empty() {
            args.push(format!("--http-header-fields={fields}"));
        }
        args.push(req.url.clone());

        let mut cmd = Command::new(&exe);
        cmd.args(&args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        if let Some(dir) = exe.parent() {
            cmd.current_dir(dir);
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
            cmd.creation_flags(CREATE_NO_WINDOW | CREATE_NEW_PROCESS_GROUP);
        }
        let child = cmd.spawn().map_err(|_| "mpv.exe not found".to_string())?;
        match wait_open_ipc(&ipc_path) {
            Ok(file) => {
                self.ipc = Some(file);
                self.ipc_path = Some(ipc_path);
                self.child = Some(child);
                self.state = PlayState::Playing;
                Ok(())
            }
            Err(_) => {
                let mut child = child;
                let _ = child.kill();
                let _ = child.wait();
                cleanup_ipc(&ipc_path);
                self.state = PlayState::Error("mpv failed to start".into());
                Err("mpv failed to start".into())
            }
        }
    }

    fn stop(&mut self) {
        self.send(serde_json::json!({"command": ["quit"]}));
        self.ipc = None;
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        if let Some(p) = self.ipc_path.take() {
            cleanup_ipc(&p);
        }
        self.state = PlayState::Stopped;
    }

    fn pause(&mut self, paused: bool) {
        self.send(serde_json::json!({"command": ["set_property", "pause", paused]}));
        if matches!(self.state, PlayState::Playing | PlayState::Paused) {
            self.state = if paused {
                PlayState::Paused
            } else {
                PlayState::Playing
            };
        }
    }

    fn mute(&mut self, muted: bool) {
        self.send(serde_json::json!({"command": ["set_property", "mute", muted]}));
    }

    fn volume(&mut self, vol: f64) {
        let v = (vol.clamp(0.0, 1.0) * 100.0).round();
        self.send(serde_json::json!({"command": ["set_property", "volume", v]}));
    }

    fn status(&self) -> PlayState {
        self.state.clone()
    }
}

fn ipc_endpoint() -> (String, PathBuf) {
    let token = format!(
        "ghoul-mpv-{}-{}",
        std::process::id(),
        unix_millis()
    );
    #[cfg(windows)]
    {
        let path = PathBuf::from(format!(r"\\.\pipe\{token}"));
        (format!(r"\\.\pipe\{token}"), path)
    }
    #[cfg(not(windows))]
    {
        let path = std::env::temp_dir().join(format!("{token}.sock"));
        (path.to_string_lossy().into_owned(), path)
    }
}

fn unix_millis() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

fn wait_open_ipc(path: &Path) -> Result<std::fs::File, String> {
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut last = "mpv.exe not found".to_string();
    while Instant::now() < deadline {
        match OpenOptions::new().read(true).write(true).open(path) {
            Ok(f) => return Ok(f),
            Err(e) => last = e.to_string(),
        }
        thread::sleep(Duration::from_millis(40));
    }
    let _ = last;
    Err("mpv failed to start".into())
}

fn cleanup_ipc(path: &Path) {
    #[cfg(not(windows))]
    {
        let _ = std::fs::remove_file(path);
    }
    let _ = path;
}
