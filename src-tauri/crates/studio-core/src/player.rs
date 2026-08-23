// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::BTreeMap;
use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;

use crate::info::USER_AGENT;
use crate::logo::PLAYER_UA;
use crate::settings::{AppSettings, PlayerEngine};
use crate::tools::{default_mpv_path, default_vlc_path};

pub fn play(
    url: &str,
    settings: &AppSettings,
    headers: Option<&BTreeMap<String, String>>,
    app_root: &Path,
) -> Result<(), String> {
    if url.trim().is_empty() {
        return Err("URL is empty.".into());
    }
    match effective_player(settings, app_root) {
        PlayerEngine::Vlc => play_vlc(url, settings, headers, app_root),
        PlayerEngine::Mpv => play_mpv(url, settings, headers, app_root),
    }
}

pub fn effective_player(settings: &AppSettings, app_root: &Path) -> PlayerEngine {
    if settings.default_player == PlayerEngine::Vlc {
        let vlc = resolve_vlc(settings);
        if Path::new(&vlc).is_file() {
            return PlayerEngine::Vlc;
        }
    }
    let mpv = resolve_mpv(settings, app_root);
    if Path::new(&mpv).is_file() {
        return PlayerEngine::Mpv;
    }
    let vlc = resolve_vlc(settings);
    if Path::new(&vlc).is_file() {
        return PlayerEngine::Vlc;
    }
    settings.default_player
}

fn stream_user_agent(headers: Option<&BTreeMap<String, String>>) -> String {
    if let Some(h) = headers {
        for (k, v) in h {
            if k.eq_ignore_ascii_case("user-agent")
                && !v.trim().is_empty()
                && !v.trim().eq_ignore_ascii_case(USER_AGENT)
            {
                return v.clone();
            }
        }
    }
    PLAYER_UA.to_string()
}

fn resolve_mpv(settings: &AppSettings, app_root: &Path) -> String {
    if !settings.mpv_path.trim().is_empty() && Path::new(&settings.mpv_path).is_file() {
        return settings.mpv_path.clone();
    }
    default_mpv_path(app_root).to_string_lossy().into_owned()
}

fn resolve_vlc(settings: &AppSettings) -> String {
    if !settings.vlc_path.trim().is_empty() && Path::new(&settings.vlc_path).is_file() {
        return settings.vlc_path.clone();
    }
    default_vlc_path().to_string_lossy().into_owned()
}

fn start_detached(path: &str, args: &[String]) -> Result<(), String> {
    let mut cmd = Command::new(path);
    cmd.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if let Some(dir) = Path::new(path).parent() {
        if dir.is_dir() {
            cmd.current_dir(dir);
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW so mpv does not attach to this console and fail to
        // show a video window.
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        cmd.creation_flags(CREATE_NO_WINDOW | CREATE_NEW_PROCESS_GROUP);
    }
    cmd.spawn().map_err(|e| e.to_string())?;
    Ok(())
}

fn mpv_args(url: &str, headers: Option<&BTreeMap<String, String>>) -> Vec<String> {
    // --ytdl=no: IPTV http(s) URLs must not go through youtube-dl. Without it,
    // this mpv build logs "youtube-dl failed" then "Failed to recognize file
    // format" and exits â€” VLC plays the same URL.
    let mut args = vec![
        "--force-window=yes".to_string(),
        "--keep-open=yes".to_string(),
        "--no-terminal".to_string(),
        "--ytdl=no".to_string(),
    ];
    if let Some(h) = headers {
        if !h.is_empty() {
            let lines = h
                .iter()
                .map(|(k, v)| format!("{k}: {v}"))
                .collect::<Vec<_>>()
                .join("\r\n");
            args.push(format!("--http-header-fields={lines}"));
        }
    }
    args.push(url.to_string());
    args
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProbeVerdict {
    Ok,
    Dead(String),
}

/// Classify the first bytes of an HTTP response so we can tell a dead
/// playlist/HTML error page from a live MPEG-TS / HLS body.
pub fn classify_probe_body(status: u16, body: &[u8]) -> ProbeVerdict {
    if status == 404 {
        return ProbeVerdict::Dead("This stream looks dead (HTTP 404).".into());
    }
    if status == 401 || status == 403 {
        return ProbeVerdict::Dead(format!(
            "This stream was refused (HTTP {status}). The URL may be expired or need different headers."
        ));
    }
    if !(200..400).contains(&status) {
        return ProbeVerdict::Dead(format!("This stream looks dead (HTTP {status})."));
    }
    if body.is_empty() {
        return ProbeVerdict::Dead(
            "This stream URL returned no data â€” it is probably dead.".into(),
        );
    }
    let start: Vec<u8> = body
        .iter()
        .copied()
        .skip_while(|b| b.is_ascii_whitespace())
        .take(64)
        .collect();
    let head = String::from_utf8_lossy(&start).to_ascii_lowercase();
    if head.starts_with("<!doctype")
        || head.starts_with("<html")
        || head.starts_with("<head")
        || head.starts_with("<body")
    {
        return ProbeVerdict::Dead(
            "This stream URL returned a web page, not video â€” it is probably dead.".into(),
        );
    }
    ProbeVerdict::Ok
}

pub fn probe_stream(url: &str, headers: Option<&BTreeMap<String, String>>) -> Result<(), String> {
    let trimmed = url.trim();
    let lower = trimmed.to_ascii_lowercase();
    if !lower.starts_with("http://") && !lower.starts_with("https://") {
        return Ok(());
    }
    let mut req = ureq::get(trimmed).timeout(Duration::from_secs(5));
    if let Some(h) = headers {
        for (k, v) in h {
            if k.eq_ignore_ascii_case("user-agent") {
                continue;
            }
            req = req.set(k, v);
        }
    }
    req = req.set("User-Agent", &stream_user_agent(headers));
    match req.call() {
        Ok(resp) => {
            let status = resp.status();
            let mut buf = [0u8; 2048];
            let n = resp.into_reader().read(&mut buf).unwrap_or(0);
            match classify_probe_body(status, &buf[..n]) {
                ProbeVerdict::Ok => Ok(()),
                ProbeVerdict::Dead(msg) => Err(msg),
            }
        }
        Err(ureq::Error::Status(code, _)) => Err(format!("This stream looks dead (HTTP {code}).")),
        Err(_) => Err("Could not reach this stream. It may be dead.".into()),
    }
}

fn play_mpv(
    url: &str,
    settings: &AppSettings,
    headers: Option<&BTreeMap<String, String>>,
    app_root: &Path,
) -> Result<(), String> {
    let path = resolve_mpv(settings, app_root);
    if !Path::new(&path).is_file() {
        return Err(format!(
            "mpv not found at '{path}'. Configure path in Settings."
        ));
    }
    // Wrapper: refuse to launch mpv on a dead/HTML error URL so the toast
    // can say why instead of a flash-and-exit player window.
    probe_stream(url, headers)?;
    start_detached(&path, &mpv_args(url, headers))
}

fn play_vlc(
    url: &str,
    settings: &AppSettings,
    headers: Option<&BTreeMap<String, String>>,
    _app_root: &Path,
) -> Result<(), String> {
    let path = resolve_vlc(settings);
    if !Path::new(&path).is_file() {
        return Err(format!(
            "VLC not found at '{path}'. Configure path in Settings."
        ));
    }
    let mut args = vec![url.to_string()];
    if let Some(ua) = headers.and_then(|h| {
        h.iter()
            .find(|(k, _)| k.eq_ignore_ascii_case("User-Agent"))
            .map(|(_, v)| v.clone())
    }) {
        args.push(format!(":http-user-agent={ua}"));
    }
    start_detached(&path, &args)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_url_is_rejected() {
        let s = AppSettings::default();
        let err = play("  ", &s, None, Path::new(".")).unwrap_err();
        assert!(err.contains("empty"));
    }

    #[test]
    fn resolve_mpv_uses_bundled_default_when_settings_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("tools").join("mpv");
        std::fs::create_dir_all(&dir).unwrap();
        let mpv = dir.join(crate::tools::tool_file_name("mpv"));
        std::fs::write(&mpv, b"x").unwrap();
        let s = AppSettings::default();
        assert_eq!(
            resolve_mpv(&s, tmp.path()),
            mpv.to_string_lossy().into_owned()
        );
        assert_eq!(effective_player(&s, tmp.path()), PlayerEngine::Mpv);
    }

    #[test]
    fn resolve_mpv_prefers_existing_settings_path() {
        let tmp = tempfile::tempdir().unwrap();
        let custom = tmp.path().join(crate::tools::tool_file_name("custom-mpv"));
        std::fs::write(&custom, b"x").unwrap();
        let mut s = AppSettings::default();
        s.mpv_path = custom.to_string_lossy().into_owned();
        assert_eq!(
            resolve_mpv(&s, tmp.path()),
            custom.to_string_lossy().into_owned()
        );
    }

    #[test]
    fn mpv_args_skip_ytdl_and_keep_url_last() {
        let url = "http://example.com/live.ts?a=1&b=2";
        let args = mpv_args(url, None);
        assert!(args.iter().any(|a| a == "--ytdl=no"));
        assert!(args.iter().any(|a| a == "--force-window=yes"));
        assert_eq!(args.last().map(String::as_str), Some(url));
    }

    #[test]
    fn probe_classifies_html_and_http_errors_as_dead() {
        assert!(matches!(
            classify_probe_body(404, b""),
            ProbeVerdict::Dead(m) if m.contains("404")
        ));
        assert!(matches!(
            classify_probe_body(200, b"<!DOCTYPE html><html>offline</html>"),
            ProbeVerdict::Dead(m) if m.to_ascii_lowercase().contains("web page")
        ));
        assert!(matches!(
            classify_probe_body(200, b""),
            ProbeVerdict::Dead(_)
        ));
    }

    #[test]
    fn probe_accepts_hls_and_mpegts() {
        assert_eq!(
            classify_probe_body(200, b"#EXTM3U\n#EXTINF:-1,\nhttp://x/1.ts\n"),
            ProbeVerdict::Ok
        );
        assert_eq!(classify_probe_body(200, &[0x47, 0x40, 0x00, 0x10]), ProbeVerdict::Ok);
    }

    #[test]
    fn probe_skips_non_http() {
        assert!(probe_stream("C:\\videos\\clip.ts", None).is_ok());
    }

    #[test]
    fn stream_user_agent_ignores_app_identity() {
        let mut h = BTreeMap::new();
        h.insert("User-Agent".into(), USER_AGENT.into());
        assert_eq!(stream_user_agent(Some(&h)), PLAYER_UA);
        h.insert("User-Agent".into(), "Custom/1".into());
        assert_eq!(stream_user_agent(Some(&h)), "Custom/1");
        assert_eq!(stream_user_agent(None), PLAYER_UA);
    }
}
