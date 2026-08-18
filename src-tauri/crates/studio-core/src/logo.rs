// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};

use crate::models::ManagedChannel;
use crate::paths::app_data_directory;

pub const PLAYER_UA: &str = "VLC/3.0.20 LibVLC/3.0.20";

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogoCheck {
    pub issue: Option<String>,
    pub reason: String,
}

impl LogoCheck {
    pub fn ok() -> Self {
        Self {
            issue: None,
            reason: String::new(),
        }
    }
    pub fn is_ok(&self) -> bool {
        self.issue.as_deref().unwrap_or("").is_empty()
    }
}

fn reject(reason: &str) -> LogoCheck {
    LogoCheck {
        issue: Some("player-reject".into()),
        reason: reason.into(),
    }
}

/// Host + AbsolutePath equivalent of System.Uri (no userinfo, query, or fragment).
fn host_and_path(url: &str) -> (String, String) {
    let rest = url
        .split_once("://")
        .map(|(_, r)| r)
        .unwrap_or(url);
    let rest = rest.split(['?', '#']).next().unwrap_or(rest);
    let rest = if let Some(at) = rest.rfind('@') {
        &rest[at + 1..]
    } else {
        rest
    };
    let (host, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, ""),
    };
    let host = host.split(':').next().unwrap_or(host).to_ascii_lowercase();
    (host, path.to_string())
}

pub fn classify_url(url: &str) -> LogoCheck {
    let raw = url.trim();
    if raw.is_empty() {
        return LogoCheck {
            issue: Some("missing".into()),
            reason: "No logo URL.".into(),
        };
    }
    let lower = raw.to_ascii_lowercase();
    if !lower.starts_with("http://") && !lower.starts_with("https://") {
        return LogoCheck {
            issue: Some("invalid".into()),
            reason: "Logo URL must be http(s).".into(),
        };
    }
    let (host, path) = host_and_path(raw);
    let path_l = path.to_ascii_lowercase();

    if host.contains("wikimedia.org") || host.contains("wikipedia.org") {
        return reject(
            "Wikimedia/Wikipedia block TiviMate, Plex, and tuners (browser-only). Use a direct PNG on a CDN or tv-logos.",
        );
    }
    if host.contains("google.") && (path_l.contains("imgres") || path_l.contains("search")) {
        return reject(
            "Google image pages are HTML, not a file. Right-click the image → Copy image address.",
        );
    }
    if host.contains("bing.com") && path_l.contains("images") {
        return reject("Bing image pages are HTML. Use the direct image file URL.");
    }
    if host == "github.com" && path_l.contains("/blob/") {
        return reject("GitHub blob pages are HTML. Use raw.githubusercontent.com /…png.");
    }
    if path_l.ends_with(".svg") || path_l.ends_with(".svgz") {
        return reject("SVG is ignored by most IPTV players and Plex. Use PNG or JPEG.");
    }
    if path_l.ends_with(".webp") {
        return reject("WebP is hit-or-miss in TiviMate/Plex/older tuners. Prefer PNG or JPEG.");
    }
    if lower.starts_with("data:") {
        return reject("data: URLs are not fetched by players.");
    }
    LogoCheck::ok()
}

pub fn is_player_safe_image(b: &[u8]) -> bool {
    if b.len() < 4 {
        return false;
    }
    if b[0] == 0x89 && b[1] == 0x50 && b[2] == 0x4E && b[3] == 0x47 {
        return true;
    }
    if b[0] == 0xFF && b[1] == 0xD8 && b[2] == 0xFF {
        return true;
    }
    if b[0] == b'G' && b[1] == b'I' && b[2] == b'F' {
        return true;
    }
    false
}

pub fn probe_url(url: &str) -> LogoCheck {
    let classified = classify_url(url);
    if !classified.is_ok() {
        return classified;
    }
    match ureq::get(url)
        .set("User-Agent", PLAYER_UA)
        .set("Accept", "image/png,image/jpeg,image/gif,*/*")
        .timeout(std::time::Duration::from_secs(15))
        .call()
    {
        Ok(resp) => {
            let status = resp.status();
            if !(200..300).contains(&status) {
                return reject(&format!(
                    "Players got HTTP {status}. TiviMate/Plex/tuners use a simple GET (this probe uses {PLAYER_UA})."
                ));
            }
            let media = resp.content_type().to_ascii_lowercase();
            if media.contains("svg") {
                return reject("Server sent SVG. Most players skip SVG.");
            }
            if media.contains("html") || media.contains("json") {
                return reject("URL returned a page, not an image file.");
            }
            let mut buf = [0u8; 16];
            let n = resp.into_reader().read(&mut buf).unwrap_or(0);
            if !is_player_safe_image(&buf[..n]) {
                return reject("Bytes are not PNG/JPEG/GIF. Players will skip WebP, SVG, ICO, and HTML.");
            }
            LogoCheck::ok()
        }
        Err(e) => reject(&format!("Fetch failed the way a player would: {e}")),
    }
}

pub fn issue_label(issue: Option<&str>) -> &'static str {
    match issue {
        Some("invalid") => "Invalid URL",
        Some("broken") => "Won't load",
        Some("player-reject") => "Players reject",
        Some("missing") => "Missing",
        _ => "OK",
    }
}

/// Path.GetInvalidFileNameChars + `/` `\` `:` (LogoSaver.Sanitize).
pub fn sanitize(raw: &str) -> String {
    let s = raw.trim().to_lowercase();
    let t: String = s
        .chars()
        .map(|c| {
            if c.is_control()
                || matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|')
            {
                '_'
            } else {
                c
            }
        })
        .collect();
    let t = t.trim_matches(['.', ' ']);
    if t.is_empty() {
        "_".into()
    } else {
        t.to_string()
    }
}

pub fn default_logo_dir() -> PathBuf {
    app_data_directory().join("logo")
}

pub fn dest_path(root: &Path, group: &str, tvg_id: &str) -> PathBuf {
    root.join(sanitize(group)).join(format!("{}.png", sanitize(tvg_id)))
}

pub fn tracker_path(root: &Path) -> PathBuf {
    root.join("logo-save-tracker.json")
}

pub fn hosted_path(tvg_id: &str) -> String {
    format!("/logos/{}.png", sanitize(tvg_id))
}

pub fn hosted_url(tuner_base: &str, tvg_id: &str) -> String {
    format!("{}{}", tuner_base.trim_end_matches('/'), hosted_path(tvg_id))
}

/// C# LogoSaver.PlaylistLogo — local tuner logos replace tvg-logo when asked.
pub fn playlist_logo(ch: &ManagedChannel, tuner_base: &str, use_local: bool) -> Option<String> {
    if !use_local || ch.tvg_id.as_deref().unwrap_or("").trim().is_empty() {
        return ch
            .tvg_logo
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string);
    }
    Some(hosted_url(tuner_base, ch.tvg_id.as_deref().unwrap()))
}

pub fn try_resolve_hosted(root: &Path, request_path: &str) -> Option<PathBuf> {
    let mut rel = request_path.trim();
    if rel.len() >= 7 && rel[..7].eq_ignore_ascii_case("/logos/") {
        rel = &rel[7..];
    }
    let rel = url_unescape(rel).trim_start_matches(['/', '\\']).to_string();
    let stem = if rel.to_ascii_lowercase().ends_with(".png") {
        &rel[..rel.len() - 4]
    } else {
        &rel
    };
    let name = if stem.contains('/') || stem.contains('\\') {
        std::path::Path::new(stem)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or(stem)
    } else {
        stem
    };
    let found = find_by_tvg_id(root, name)?;
    let root_full = std::fs::canonicalize(root).ok()?;
    let found_full = std::fs::canonicalize(&found).ok()?;
    if found_full.starts_with(&root_full) {
        Some(found)
    } else {
        None
    }
}

fn url_unescape(s: &str) -> String {
    let mut out = String::new();
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Ok(v) = u8::from_str_radix(std::str::from_utf8(&b[i + 1..i + 3]).unwrap_or(""), 16) {
                out.push(v as char);
                i += 3;
                continue;
            }
        }
        out.push(b[i] as char);
        i += 1;
    }
    out
}

pub fn find_by_tvg_id(root: &Path, tvg_id: &str) -> Option<PathBuf> {
    if tvg_id.trim().is_empty() {
        return None;
    }
    let name = format!("{}.png", sanitize(tvg_id));
    let flat = root.join(&name);
    if flat.is_file() {
        return std::fs::canonicalize(&flat).ok().or(Some(flat));
    }
    if !root.is_dir() {
        return None;
    }
    let entries = std::fs::read_dir(root).ok()?;
    for ent in entries.flatten() {
        let nested = ent.path().join(&name);
        if nested.is_file() {
            return std::fs::canonicalize(&nested).ok().or(Some(nested));
        }
    }
    None
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogoIssue {
    pub managed_channel_id: String,
    pub channel_name: String,
    pub group_title: String,
    pub tvg_id: Option<String>,
    pub current_logo: Option<String>,
    pub issue: String,
    pub reason: String,
}

pub fn classify_channel(ch: &ManagedChannel) -> LogoIssue {
    let check = classify_url(ch.tvg_logo.as_deref().unwrap_or(""));
    LogoIssue {
        managed_channel_id: ch.id.clone(),
        channel_name: ch.name.clone(),
        group_title: if ch.group_title.trim().is_empty() {
            "Ungrouped".into()
        } else {
            ch.group_title.clone()
        },
        tvg_id: ch.tvg_id.clone(),
        current_logo: ch.tvg_logo.clone(),
        issue: check.issue.unwrap_or_default(),
        reason: check.reason,
    }
}

pub fn sort_issues(rows: &mut [LogoIssue]) {
    rows.sort_by(|a, b| {
        a.group_title
            .to_ascii_lowercase()
            .cmp(&b.group_title.to_ascii_lowercase())
            .then(
                a.channel_name
                    .to_ascii_lowercase()
                    .cmp(&b.channel_name.to_ascii_lowercase()),
            )
    });
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogoSaveItem {
    pub channel_id: String,
    pub name: String,
    pub group: String,
    pub tvg_id: String,
    pub url: String,
    pub dest_path: String,
    pub status: String,
    pub error: Option<String>,
}

pub fn load_tracker(root: &Path) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let path = tracker_path(root);
    let Ok(text) = std::fs::read_to_string(path) else {
        return map;
    };
    let Ok(doc) = serde_json::from_str::<serde_json::Value>(&text) else {
        return map;
    };
    let Some(items) = doc.get("items").and_then(|v| v.as_object()) else {
        return map;
    };
    for (k, v) in items {
        let st = v
            .get("status")
            .and_then(|s| s.as_str())
            .or_else(|| v.as_str())
            .unwrap_or("");
        if !st.is_empty() {
            map.insert(k.to_ascii_lowercase(), st.to_string());
        }
    }
    map
}

pub fn save_tracker(root: &Path, items: &[LogoSaveItem]) {
    let _ = std::fs::create_dir_all(root);
    let mut map = serde_json::Map::new();
    for i in items {
        map.insert(
            i.tvg_id.clone(),
            serde_json::json!({
                "status": i.status,
                "path": i.dest_path,
                "error": i.error,
            }),
        );
    }
    let payload = serde_json::json!({
        "updatedAt": now_iso(),
        "items": map,
    });
    let _ = std::fs::write(
        tracker_path(root),
        serde_json::to_string_pretty(&payload).unwrap_or_default(),
    );
}

fn now_iso() -> String {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_default()
}

pub fn plan_save(channels: &[ManagedChannel], root: &Path) -> Vec<LogoSaveItem> {
    let tracker = load_tracker(root);
    let mut items = Vec::new();
    for ch in channels {
        let url = ch.tvg_logo.as_deref().unwrap_or("").trim();
        let tvg = ch.tvg_id.as_deref().unwrap_or("").trim();
        if url.is_empty() || tvg.is_empty() {
            continue;
        }
        let check = classify_url(url);
        let dest = dest_path(root, &ch.group_title, tvg);
        let ok = check.is_ok();
        let (mut status, mut err) = if ok {
            ("pending".into(), None)
        } else {
            ("skip".into(), Some(check.reason))
        };
        if dest.exists() {
            status = "saved".into();
            err = None;
        } else if let Some(prev) = tracker.get(&tvg.to_ascii_lowercase()) {
            if prev != "saved" {
                status = if prev == "skip" && ok {
                    "pending".into()
                } else {
                    prev.clone()
                };
            }
        }
        items.push(LogoSaveItem {
            channel_id: ch.id.clone(),
            name: ch.name.clone(),
            group: if ch.group_title.trim().is_empty() {
                "ungrouped".into()
            } else {
                ch.group_title.clone()
            },
            tvg_id: tvg.to_string(),
            url: url.to_string(),
            dest_path: dest.to_string_lossy().into_owned(),
            status,
            error: err,
        });
    }
    items
}

pub fn save_one(item: &mut LogoSaveItem, ffmpeg_path: &str) {
    if item.status == "skip" {
        return;
    }
    let classified = classify_url(&item.url);
    if !classified.is_ok() {
        item.status = "failed".into();
        item.error = Some(classified.reason);
        return;
    }
    if let Some(dir) = Path::new(&item.dest_path).parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let tmp = std::env::temp_dir().join(format!("epgm-logo-{}.bin", uuid::Uuid::new_v4().simple()));
    match ureq::get(&item.url)
        .set("User-Agent", PLAYER_UA)
        .set("Accept", "*/*")
        .timeout(std::time::Duration::from_secs(20))
        .call()
    {
        Ok(resp) => {
            let status = resp.status();
            if !(200..300).contains(&status) {
                item.status = "failed".into();
                item.error = Some(format!("HTTP {status}"));
                let _ = std::fs::remove_file(&tmp);
                return;
            }
            let mut bytes = Vec::new();
            if resp.into_reader().read_to_end(&mut bytes).is_err() || !is_player_safe_image(&bytes) {
                item.status = "failed".into();
                item.error = Some("Response is not PNG/JPEG/GIF".into());
                let _ = std::fs::remove_file(&tmp);
                return;
            }
            if std::fs::write(&tmp, &bytes).is_err() {
                item.status = "failed".into();
                item.error = Some("temp write failed".into());
                return;
            }
            let png_magic = bytes.len() >= 4 && bytes[0] == 0x89 && bytes[1] == 0x50;
            if png_magic {
                if std::fs::copy(&tmp, &item.dest_path).is_ok() {
                    item.status = "saved".into();
                    item.error = None;
                } else {
                    item.status = "failed".into();
                    item.error = Some("write failed".into());
                }
            } else if convert_png(ffmpeg_path, &tmp, Path::new(&item.dest_path)) {
                item.status = "saved".into();
                item.error = None;
            } else {
                item.status = "failed".into();
                item.error = Some("ffmpeg could not convert to PNG".into());
            }
            let _ = std::fs::remove_file(&tmp);
        }
        Err(e) => {
            let _ = std::fs::remove_file(&tmp);
            item.status = "failed".into();
            item.error = Some(e.to_string());
        }
    }
}

fn convert_png(ffmpeg: &str, src: &Path, dest: &Path) -> bool {
    if ffmpeg.is_empty() || !Path::new(ffmpeg).is_file() {
        return false;
    }
    std::process::Command::new(ffmpeg)
        .args([
            "-y",
            "-hide_banner",
            "-loglevel",
            "error",
            "-i",
        ])
        .arg(src)
        .args(["-frames:v", "1"])
        .arg(dest)
        .status()
        .map(|s| s.success() && dest.is_file())
        .unwrap_or(false)
}

/// SearchQueryFor + GoogleImagesTransparentUrl / DuckDuckGoImagesUrl / TvLogosGithubSearchUrl.
pub fn search_urls(name: &str) -> (String, String, String) {
    let q = if name.trim().is_empty() {
        "channel logo".into()
    } else {
        format!("{} logo", name.trim())
    };
    let google = crate::epg::google_images_transparent_url(&format!("{q} transparent png"));
    let ddg = duckduckgo_images_url(&q);
    let tv_q = if name.trim().is_empty() {
        "logo"
    } else {
        name.trim()
    };
    (google, ddg, tv_logos_github_search_url(tv_q))
}

pub fn duckduckgo_images_url(query: &str) -> String {
    let q = if query.trim().is_empty() {
        "logo"
    } else {
        query.trim()
    };
    format!(
        "https://duckduckgo.com/?iax=images&ia=images&q={}",
        crate::epg::urlencoding_minimal(&format!("{q} transparent logo png"))
    )
}

pub fn tv_logos_github_search_url(query: &str) -> String {
    let q = if query.trim().is_empty() {
        "logo"
    } else {
        query.trim()
    };
    format!(
        "https://github.com/search?type=code&q={}",
        crate::epg::urlencoding_minimal(&format!("repo:tv-logo/tv-logos {q}"))
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_and_invalid() {
        assert_eq!(classify_url("").issue.as_deref(), Some("missing"));
        assert_eq!(classify_url("ftp://x").issue.as_deref(), Some("invalid"));
    }

    #[test]
    fn wikimedia_is_player_reject_not_broken() {
        let c = classify_url("https://upload.wikimedia.org/wikipedia/commons/c/cf/Brazzers_logo.png");
        assert_eq!(c.issue.as_deref(), Some("player-reject"));
        assert!(c.reason.to_ascii_lowercase().contains("wikimedia"));
    }

    #[test]
    fn svg_and_search_pages() {
        assert_eq!(
            classify_url("https://example.com/logo.svg").issue.as_deref(),
            Some("player-reject")
        );
        assert_eq!(
            classify_url("https://www.google.com/imgres?imgurl=x").issue.as_deref(),
            Some("player-reject")
        );
        assert_eq!(
            classify_url("https://github.com/user/repo/blob/main/cnn.png")
                .issue
                .as_deref(),
            Some("player-reject")
        );
    }

    #[test]
    fn svg_query_still_rejected() {
        assert_eq!(
            classify_url("https://cdn.example/logo.svg?cache=1").issue.as_deref(),
            Some("player-reject")
        );
    }

    #[test]
    fn svg_in_directory_name_is_allowed() {
        assert!(classify_url("https://cdn.example/svg-icons/logo.png").is_ok());
    }

    #[test]
    fn webp_rejected() {
        assert_eq!(
            classify_url("https://cdn.example/logo.webp").issue.as_deref(),
            Some("player-reject")
        );
    }

    #[test]
    fn allows_direct_https_png() {
        let c = classify_url(
            "https://raw.githubusercontent.com/tv-logo/tv-logos/main/countries/united-states/cnn-us.png",
        );
        assert!(c.issue.is_none());
    }

    #[test]
    fn png_jpeg_gif_magic() {
        assert!(is_player_safe_image(&[0x89, 0x50, 0x4E, 0x47, 0, 0, 0, 0]));
        assert!(is_player_safe_image(&[0xFF, 0xD8, 0xFF, 0xE0]));
        assert!(is_player_safe_image(&[b'G', b'I', b'F', b'8']));
        assert!(!is_player_safe_image(&[
            b'R', b'I', b'F', b'F', 0, 0, 0, 0, b'W', b'E', b'B', b'P'
        ]));
        assert!(!is_player_safe_image(b"<svg xmlns"));
    }

    #[test]
    fn dest_is_group_tvgid_png() {
        let p = dest_path(Path::new("/logo"), "NEWS", "CNN.us");
        assert!(p.ends_with(Path::new("news").join("cnn.us.png")));
    }

    #[test]
    fn tracker_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let items = [LogoSaveItem {
            channel_id: "1".into(),
            name: "CNN".into(),
            group: "NEWS".into(),
            tvg_id: "CNN.us".into(),
            url: "https://cdn.example/cnn.png".into(),
            dest_path: dest_path(dir.path(), "NEWS", "CNN.us")
                .to_string_lossy()
                .into_owned(),
            status: "failed".into(),
            error: Some("HTTP 404".into()),
        }];
        save_tracker(dir.path(), &items);
        let map = load_tracker(dir.path());
        assert_eq!(map.get("cnn.us").map(String::as_str), Some("failed"));
    }

    #[test]
    fn search_urls_are_name_only() {
        let (g, d, t) = search_urls("CNN");
        assert!(g.contains("CNN%20logo%20transparent%20png"));
        assert!(d.contains("CNN%20logo%20transparent%20logo%20png"));
        assert!(t.contains("tv-logo"));
        assert!(t.contains("CNN"));
    }
}
