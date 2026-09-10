// SPDX-License-Identifier: GPL-3.0-or-later

use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::PathBuf;
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

static LOG: Mutex<Option<File>> = Mutex::new(None);

const IO_ATTEMPTS: u32 = 8;

fn io_transient(err: &io::Error) -> bool {
    matches!(
        err.kind(),
        io::ErrorKind::PermissionDenied | io::ErrorKind::AlreadyExists
    ) || matches!(err.raw_os_error(), Some(5) | Some(32)) // ACCESS_DENIED / SHARING_VIOLATION
}

/// 8 attempts; backoff 50ms, 100ms, 200ms, … on lock/permission errors.
pub fn retry_io<T>(mut op: impl FnMut() -> std::io::Result<T>) -> std::io::Result<T> {
    let mut delay = Duration::from_millis(50);
    let mut attempt = 0u32;
    loop {
        match op() {
            Ok(v) => return Ok(v),
            Err(e) => {
                attempt += 1;
                if attempt >= IO_ATTEMPTS || !io_transient(&e) {
                    return Err(e);
                }
                thread::sleep(delay);
                delay = delay.saturating_mul(2);
            }
        }
    }
}

pub fn path() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("."));
    let dir = base.join("epg.monster-studio").join("logs");
    let _ = std::fs::create_dir_all(&dir);
    dir.join("ghoul.log")
}

pub fn init() {
    let p = path();
    let file = retry_io(|| OpenOptions::new().create(true).append(true).open(&p)).or_else(|_| {
        let fallback = std::env::current_exe()
            .ok()
            .and_then(|e| e.parent().map(|d| d.join("ghoul.log")))
            .unwrap_or_else(|| PathBuf::from("ghoul.log"));
        retry_io(|| OpenOptions::new().create(true).append(true).open(fallback.clone()))
    });
    if let Ok(f) = file {
        if let Ok(mut g) = LOG.lock() {
            *g = Some(f);
        }
    }
    line("info", &format!("log file {}", path().display()));
    std::panic::set_hook(Box::new(|info| {
        line("panic", &info.to_string());
    }));
}

pub fn line(level: &str, msg: &str) {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let row = format!("{ts} [{level}] {msg}\n");
    eprint!("{row}");
    if let Ok(mut g) = LOG.lock() {
        if let Some(f) = g.as_mut() {
            let _ = retry_io(|| {
                f.write_all(row.as_bytes())?;
                f.flush()
            });
        }
    }
}

pub fn redact_url(url: &str) -> String {
    let t = url.trim();
    if t.starts_with("file:") {
        let last = t.rsplit(['/', '\\']).find(|s| !s.is_empty()).unwrap_or("");
        return format!("file:…/{last}");
    }
    if let Some(scheme_end) = t.find("://") {
        let rest = &t[scheme_end + 3..];
        let host = rest.split(['/', '?', '#']).next().unwrap_or("");
        let last = t.rsplit('/').next().unwrap_or("");
        return format!("{}://{host}/…/{last}", &t[..scheme_end]);
    }
    "…".into()
}
