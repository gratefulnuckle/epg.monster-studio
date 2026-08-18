// SPDX-License-Identifier: GPL-3.0-or-later

use std::fs;
use std::io::{BufRead, Write};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::paths::{app_data_directory, crashes_directory, current_log_path};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CrashReport {
    pub title: String,
    pub summary: String,
    pub details: String,
    pub report_path: String,
    pub log_path: String,
    pub when: String,
    pub kind: String,
}

fn session_lock_path() -> PathBuf {
    app_data_directory().join("session.lock")
}
fn pending_crash_path() -> PathBuf {
    app_data_directory().join("pending-crash.txt")
}

pub fn write_session_lock(state: &str) {
    let body = format!(
        "pid={}\nstarted={}\nstate={}\nexe={}\nbase={}\n",
        std::process::id(),
        crate::audit::now_iso(),
        state,
        std::env::current_exe()
            .map(|p| p.display().to_string())
            .unwrap_or_default(),
        std::env::current_dir()
            .map(|p| p.display().to_string())
            .unwrap_or_default()
    );
    let _ = fs::write(session_lock_path(), body);
}

pub fn mark_clean_exit() {
    let _ = fs::remove_file(session_lock_path());
}

pub fn mark_tray_state() {
    write_session_lock("tray");
    append_log(
        "Info",
        "CrashGuard",
        "Main window hidden to tray (process still running)",
    );
}

pub fn write_crash_report(kind: &str, title: &str, summary: &str, details: &str, exception_type: &str) -> CrashReport {
    let dir = crashes_directory();
    let now = crate::audit::now_iso();
    let digits: String = now.chars().filter(|c| c.is_ascii_digit()).take(14).collect();
    let stamp = if digits.len() >= 14 {
        format!("{}-{}", &digits[..8], &digits[8..14])
    } else {
        digits
    };
    let path = dir.join(format!("crash-{stamp}.txt"));
    let log = current_log_path();
    let exe = std::env::current_exe()
        .map(|p| p.display().to_string())
        .unwrap_or_default();
    let base = std::env::current_dir()
        .map(|p| p.display().to_string())
        .unwrap_or_default();
    let body = format!(
        "epg.monster studio crash report\n\
========================\n\
When:        {now}\n\
Kind:        {kind}\n\
Title:       {title}\n\
Type:        {exception_type}\n\
Summary:     {summary}\n\
PID:         {}\n\
OS:          {}\n\
64-bit:      {}\n\
Runtime:     tauri/rust\n\
BaseDir:     {base}\n\
Exe:         {exe}\n\
Log:         {}\n\n\
Details / stack\n\
---------------\n\
{details}\n\n\
Recent notes\n\
------------\n\
If Kind=native: this was likely an access violation (segfault) in native code\n\
(WinUI, graphics, SQLite native, ffmpeg child, etc.). Check minidump-*.dmp in this folder.\n\
If Kind=managed: the stack above is the managed .NET exception path.\n\
If Kind=unclean: the process vanished without an exception report.\n",
        std::process::id(),
        std::env::consts::OS,
        cfg!(target_pointer_width = "64"),
        log.display()
    );
    let _ = fs::write(&path, body);
    let _ = fs::write(pending_crash_path(), path.to_string_lossy().as_bytes());
    append_log("Fatal", "CrashGuard", &format!("Crash report written: {}", path.display()));
    CrashReport {
        title: title.into(),
        summary: summary.into(),
        details: details.into(),
        report_path: path.to_string_lossy().into_owned(),
        log_path: log.to_string_lossy().into_owned(),
        when: now,
        kind: kind.into(),
    }
}

pub fn consume_pending_crash() -> Option<CrashReport> {
    let pending = pending_crash_path();
    if pending.is_file() {
        let path = fs::read_to_string(&pending).unwrap_or_default();
        let path = path.trim().to_string();
        let _ = fs::remove_file(&pending);
        let _ = fs::remove_file(session_lock_path());
        write_session_lock("running");
        if PathBuf::from(&path).is_file() {
            return Some(parse_report_file(&path));
        }
    }

    let lock = session_lock_path();
    if lock.is_file() {
        let text = fs::read_to_string(&lock).unwrap_or_default();
        let prior = parse_pid(&text);
        let mut unclean = true;
        if prior == Some(std::process::id()) {
            unclean = false;
        } else if let Some(pid) = prior {
            if process_alive(pid) {
                unclean = false;
            }
        }
        let _ = fs::remove_file(&lock);
        if unclean {
            let report = latest_session_crash(&text).unwrap_or_else(|| {
                let log_tail = read_log_tail(&current_log_path(), 40);
                write_crash_report(
                    "unclean",
                    "Unexpected shutdown",
                    &prior
                        .map(|p| format!("Previous session (pid {p}) ended without a clean exit — no managed exception was logged."))
                        .unwrap_or_else(|| {
                            "The previous epg.monster studio session did not exit cleanly (possible crash or force-close).".into()
                        }),
                    &format!(
                        "No managed stack / native SEH report was captured.\n\
Common causes:\n\
  • Window closed to tray, then process killed (Task Manager / rebuild publish)\n\
  • Native hard crash (access violation) that killed the process before handlers ran\n\
  • Auto-audit / ffmpeg child activity around the exit\n\n\
Session lock:\n{text}\n\n\
Recent log tail ({}):\n{log_tail}\n",
                        current_log_path().display()
                    ),
                    "UncleanExit",
                )
            });
            write_session_lock("running");
            return Some(report);
        }
    }
    write_session_lock("running");
    None
}

fn parse_pid(lock: &str) -> Option<u32> {
    lock.lines()
        .find(|l| l.to_ascii_lowercase().starts_with("pid="))
        .and_then(|l| l[4..].trim().parse().ok())
}

fn parse_started(lock: &str) -> Option<std::time::SystemTime> {
    let raw = lock
        .lines()
        .find(|l| l.to_ascii_lowercase().starts_with("started="))?
        [8..]
        .trim();
    time::OffsetDateTime::parse(raw, &time::format_description::well_known::Rfc3339)
        .ok()
        .map(|dt| std::time::SystemTime::from(dt))
}

fn latest_session_crash(lock: &str) -> Option<CrashReport> {
    let session_start = parse_started(lock).unwrap_or_else(|| {
        std::time::SystemTime::now()
            .checked_sub(std::time::Duration::from_secs(2 * 3600))
            .unwrap_or(std::time::UNIX_EPOCH)
    });
    let cutoff = session_start
        .checked_sub(std::time::Duration::from_secs(5 * 60))
        .unwrap_or(std::time::UNIX_EPOCH);
    let dir = crashes_directory();
    let mut latest: Option<(std::time::SystemTime, PathBuf)> = None;
    let rd = fs::read_dir(&dir).ok()?;
    for e in rd.flatten() {
        let p = e.path();
        let name = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
        if !name.starts_with("crash-") || !name.ends_with(".txt") {
            continue;
        }
        let meta = e.metadata().ok()?;
        let mtime = meta.modified().ok()?;
        if mtime < cutoff {
            continue;
        }
        if latest.as_ref().map(|(t, _)| mtime > *t).unwrap_or(true) {
            latest = Some((mtime, p));
        }
    }
    latest.map(|(_, p)| parse_report_file(&p.to_string_lossy()))
}

fn process_alive(pid: u32) -> bool {
    #[cfg(windows)]
    {
        std::process::Command::new("tasklist")
            .args(["/FI", &format!("PID eq {pid}"), "/NH"])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).contains(&pid.to_string()))
            .unwrap_or(false)
    }
    #[cfg(not(windows))]
    {
        PathBuf::from(format!("/proc/{pid}")).exists()
    }
}

fn parse_report_file(path: &str) -> CrashReport {
    let text = fs::read_to_string(path).unwrap_or_else(|_| "(could not read report file)".into());
    let grab = |key: &str| {
        text.lines()
            .find(|l| l.to_ascii_lowercase().starts_with(&key.to_ascii_lowercase()))
            .map(|l| l[key.len()..].trim().to_string())
            .unwrap_or_default()
    };
    let title = grab("Title:");
    let summary = grab("Summary:");
    let kind = grab("Kind:");
    CrashReport {
        title: if title.is_empty() {
            "Previous crash".into()
        } else {
            title
        },
        summary: if summary.is_empty() {
            "See crash report for details.".into()
        } else {
            summary
        },
        details: text,
        report_path: path.into(),
        log_path: current_log_path().to_string_lossy().into_owned(),
        when: fs::metadata(path)
            .and_then(|m| m.modified())
            .ok()
            .map(|t| {
                let dt = time::OffsetDateTime::from(t);
                format!(
                    "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
                    dt.year(),
                    dt.month() as u8,
                    dt.day(),
                    dt.hour(),
                    dt.minute(),
                    dt.second()
                )
            })
            .unwrap_or_else(crate::audit::now_iso),
        kind: if kind.is_empty() {
            "unknown".into()
        } else {
            kind
        },
    }
}

fn read_log_tail(path: &std::path::Path, lines: usize) -> String {
    let Ok(file) = fs::File::open(path) else {
        return String::new();
    };
    let all: Vec<String> = std::io::BufReader::new(file)
        .lines()
        .filter_map(|l| l.ok())
        .collect();
    let start = all.len().saturating_sub(lines);
    all[start..].join("\n")
}

pub fn append_log(level: &str, source: &str, message: &str) {
    let path = current_log_path();
    if let Ok(mut f) = fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(
            f,
            "{} [{level}] [{source}] {message}",
            crate::audit::now_iso()
        );
    }
}
