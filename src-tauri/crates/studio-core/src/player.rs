// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;

use crate::settings::{AppSettings, PlayerEngine};
use crate::tools::{detect_bundled, tool_file_name};

pub fn play(
    url: &str,
    settings: &AppSettings,
    headers: Option<&BTreeMap<String, String>>,
    app_root: &Path,
) -> Result<(), String> {
    if url.trim().is_empty() {
        return Err("URL is empty.".into());
    }
    match settings.default_player {
        PlayerEngine::Vlc => play_vlc(url, settings, headers, app_root),
        PlayerEngine::Mpv => play_mpv(url, settings, headers, app_root),
    }
}

fn resolve_mpv(settings: &AppSettings, app_root: &Path) -> String {
    if !settings.mpv_path.trim().is_empty() && Path::new(&settings.mpv_path).exists() {
        return settings.mpv_path.clone();
    }
    detect_bundled(app_root)
        .into_iter()
        .find(|(n, _)| n == "mpv")
        .map(|(_, p)| p.to_string_lossy().into_owned())
        .unwrap_or_else(|| {
            if cfg!(windows) {
                tool_file_name("mpv")
            } else {
                "mpv".into()
            }
        })
}

fn resolve_vlc(settings: &AppSettings) -> String {
    if !settings.vlc_path.trim().is_empty() && Path::new(&settings.vlc_path).exists() {
        return settings.vlc_path.clone();
    }
    if cfg!(windows) {
        let pf = r"C:\Program Files\VideoLAN\VLC\vlc.exe";
        if Path::new(pf).exists() {
            return pf.into();
        }
    }
    settings.vlc_path.clone()
}

fn play_mpv(
    url: &str,
    settings: &AppSettings,
    headers: Option<&BTreeMap<String, String>>,
    app_root: &Path,
) -> Result<(), String> {
    let path = resolve_mpv(settings, app_root);
    if !Path::new(&path).exists() {
        return Err(format!(
            "mpv not found at '{path}'. Configure path in Settings."
        ));
    }
    let mut args = vec!["--force-window=yes".to_string(), "--keep-open=yes".to_string()];
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
    Command::new(&path)
        .args(&args)
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn play_vlc(
    url: &str,
    settings: &AppSettings,
    headers: Option<&BTreeMap<String, String>>,
    _app_root: &Path,
) -> Result<(), String> {
    let path = resolve_vlc(settings);
    if !Path::new(&path).exists() {
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
    Command::new(&path)
        .args(&args)
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
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
}
