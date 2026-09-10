// SPDX-License-Identifier: GPL-3.0-or-later

//! Line protocol on stdin/stdout. Never prints stream URLs.

#![windows_subsystem = "windows"]

mod gstplay;
mod log;
mod runtime;

use std::collections::BTreeMap;
use std::io::{self, BufRead, Write};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use gstplay::{Note, Player};

fn main() {
    log::init();
    let (hwnd, gst_dir) = parse_args();
    if let Some(root) = runtime::find_gst_root(gst_dir.as_deref()) {
        runtime::apply_gst_root(&root);
    } else {
        emit("error GStreamer not installed");
        return;
    }

    let (tx, rx) = mpsc::channel::<String>();
    let _ = thread::Builder::new()
        .name("ghoul-gst-in".into())
        .spawn(move || {
            for line in io::stdin().lock().lines() {
                match line {
                    Ok(l) => {
                        if tx.send(l).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        });

    let mut ua = String::new();
    let mut headers: BTreeMap<String, String> = BTreeMap::new();
    let mut player: Option<Player> = None;

    loop {
        if let Some(p) = player.as_ref() {
            for n in p.poll() {
                match n {
                    Note::Playing => emit("playing"),
                    Note::Paused => emit("paused"),
                    Note::Buffering(p) => emit(&format!("buffering {p}")),
                    Note::Error(msg) => emit(&format!("error {msg}")),
                    Note::Eos => emit("eos"),
                }
            }
        }
        match rx.recv_timeout(Duration::from_millis(16)) {
            Ok(line) => {
                let line = line.trim().to_string();
                if line.is_empty() {
                    continue;
                }
                if line.eq_ignore_ascii_case("quit") {
                    player = None;
                    break;
                }
                if let Some(rest) = strip_ci(&line, "ua ") {
                    ua = rest.to_string();
                    continue;
                }
                if let Some(rest) = strip_ci(&line, "header ") {
                    if let Some((k, v)) = rest.split_once(' ') {
                        headers.insert(k.trim().to_string(), v.trim().to_string());
                    }
                    continue;
                }
                if let Some(url) = strip_ci(&line, "open ") {
                    match Player::new(url.trim(), &ua, &headers, hwnd) {
                        Ok(p) => {
                            if let Err(e) = p.play() {
                                emit(&format!("error {e}"));
                            }
                            player = Some(p);
                        }
                        Err(e) => emit(&format!("error {e}")),
                    }
                    continue;
                }
                if line.eq_ignore_ascii_case("play") {
                    if let Some(p) = player.as_ref() {
                        if let Err(e) = p.play() {
                            emit(&format!("error {e}"));
                        }
                    }
                    continue;
                }
                if let Some(rest) = strip_ci(&line, "pause ") {
                    if let Some(p) = player.as_ref() {
                        if rest.trim() == "1" {
                            p.pause();
                        } else if let Err(e) = p.play() {
                            emit(&format!("error {e}"));
                        }
                    }
                    continue;
                }
                if let Some(rest) = strip_ci(&line, "mute ") {
                    if let Some(p) = player.as_ref() {
                        p.set_mute(rest.trim() == "1");
                    }
                    continue;
                }
                if let Some(rest) = strip_ci(&line, "volume ") {
                    if let Some(p) = player.as_ref() {
                        if let Ok(v) = rest.trim().parse::<f64>() {
                            p.set_volume(v);
                        }
                    }
                    continue;
                }
                if let Some(rest) = strip_ci(&line, "rect ") {
                    if let Some(p) = player.as_ref() {
                        let mut it = rest.split_whitespace();
                        let x: i32 = it.next().and_then(|s| s.parse().ok()).unwrap_or(0);
                        let y: i32 = it.next().and_then(|s| s.parse().ok()).unwrap_or(0);
                        let w: i32 = it.next().and_then(|s| s.parse().ok()).unwrap_or(0);
                        let h: i32 = it.next().and_then(|s| s.parse().ok()).unwrap_or(0);
                        p.set_render_rect(x, y, w, h);
                        p.fill_window();
                    }
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
}

fn parse_args() -> (usize, Option<String>) {
    let mut hwnd = 0usize;
    let mut gst_dir = None;
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--hwnd" => hwnd = it.next().and_then(|s| s.parse().ok()).unwrap_or(0),
            "--gst-dir" => gst_dir = it.next(),
            _ => {}
        }
    }
    (hwnd, gst_dir)
}

fn strip_ci<'a>(line: &'a str, prefix: &str) -> Option<&'a str> {
    if line.len() >= prefix.len() && line[..prefix.len()].eq_ignore_ascii_case(prefix) {
        Some(&line[prefix.len()..])
    } else {
        None
    }
}

fn emit(line: &str) {
    let mut out = io::stdout().lock();
    let _ = writeln!(out, "{line}");
    let _ = out.flush();
}
