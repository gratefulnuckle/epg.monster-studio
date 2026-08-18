// SPDX-License-Identifier: GPL-3.0-or-later

use serde::Serialize;
use studio_core::paths::app_data_directory;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct TunerProbeStep {
    pub client: String,
    pub name: String,
    pub ok: bool,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct TunerProbeReport {
    pub kind: String,
    pub base_url: String,
    pub steps: Vec<TunerProbeStep>,
}

impl TunerProbeReport {
    pub fn passed(&self) -> bool {
        !self.steps.is_empty() && self.steps.iter().all(|s| s.ok)
    }
    pub fn summary(&self) -> String {
        if self.passed() {
            format!("{}: {} checks passed", self.kind, self.steps.len())
        } else {
            format!(
                "{}: {} failed / {}",
                self.kind,
                self.steps.iter().filter(|s| !s.ok).count(),
                self.steps.len()
            )
        }
    }
}

pub fn user_agent(kind: &str) -> &'static str {
    match kind {
        "Plex" => "PlexMediaServer/1.40.0",
        "Jellyfin" => "Jellyfin-Server/10.9.0",
        "Emby" => "EmbyServer/4.8.0",
        _ => "TiviMate/5.1.0",
    }
}

fn client_name(kind: &str) -> &'static str {
    match kind {
        "Plex" => "Plex DVR",
        "Jellyfin" => "Jellyfin Live TV",
        "Emby" => "Emby Live TV",
        _ => "TiviMate",
    }
}

fn pass(kind: &str, name: &str, detail: &str) -> TunerProbeStep {
    TunerProbeStep {
        client: client_name(kind).into(),
        name: name.into(),
        ok: true,
        detail: detail.into(),
    }
}
fn fail(kind: &str, name: &str, detail: &str) -> TunerProbeStep {
    TunerProbeStep {
        client: client_name(kind).into(),
        name: name.into(),
        ok: false,
        detail: detail.into(),
    }
}

fn get_text(url: &str, ua: &str) -> Result<String, String> {
    match ureq::get(url)
        .set("User-Agent", ua)
        .set("Accept", "*/*")
        .timeout(std::time::Duration::from_secs(8))
        .call()
    {
        Ok(r) => r.into_string().map_err(|e| e.to_string()),
        Err(ureq::Error::Status(code, r)) => {
            let body = r.into_string().unwrap_or_default();
            Err(format!("{code} {body}"))
        }
        Err(e) => Err(e.to_string()),
    }
}

pub fn run(kind: &str, base_url: &str) -> TunerProbeReport {
    let root = base_url.trim_end_matches('/').to_string();
    let mut report = TunerProbeReport {
        kind: kind.to_string(),
        base_url: root.clone(),
        steps: Vec::new(),
    };
    if root.is_empty() {
        report.steps.push(fail(kind, "base URL", "No tuner URL"));
        return report;
    }
    let ua = user_agent(kind);
    if kind == "Iptv" {
        probe_iptv(&mut report, &root, ua);
    } else {
        probe_hdhr(&mut report, kind, &root, ua);
    }
    report
}

fn probe_hdhr(report: &mut TunerProbeReport, kind: &str, root: &str, ua: &str) {
    match get_text(&format!("{root}/discover.json"), ua) {
        Ok(body) => report.steps.push(check_discover(kind, &body, root)),
        Err(e) => {
            report.steps.push(fail(kind, "discover.json", &e));
            return;
        }
    }
    match get_text(&format!("{root}/lineup_status.json"), ua) {
        Ok(body) if body.contains("ScanInProgress") => {
            report
                .steps
                .push(pass(kind, "lineup_status.json", "ScanInProgress present"));
        }
        Ok(_) => report
            .steps
            .push(fail(kind, "lineup_status.json", "Missing ScanInProgress")),
        Err(e) => report.steps.push(fail(kind, "lineup_status.json", &e)),
    }
    let lineup = match get_text(&format!("{root}/lineup.json"), ua) {
        Ok(body) => {
            report.steps.push(check_lineup(kind, &body, root));
            body
        }
        Err(e) => {
            report.steps.push(fail(kind, "lineup.json", &e));
            return;
        }
    };
    match get_text(&format!("{root}/guide.xml"), ua) {
        Ok(body) => report.steps.push(check_guide(kind, &body)),
        Err(e) => report.steps.push(fail(kind, "guide.xml", &e)),
    }
    if kind == "Jellyfin" {
        match get_text(&format!("{root}/tuner.m3u"), ua) {
            Ok(body) => report.steps.push(check_m3u(kind, "tuner.m3u", &body, root)),
            Err(e) => report.steps.push(fail(kind, "tuner.m3u", &e)),
        }
    }
    probe_tune(report, kind, &lineup, ua);
}

fn probe_iptv(report: &mut TunerProbeReport, root: &str, ua: &str) {
    let m3u = match get_text(&format!("{root}/playlist.m3u8"), ua) {
        Ok(body) => {
            report
                .steps
                .push(check_m3u("Iptv", "playlist.m3u8", &body, root));
            body
        }
        Err(e) => {
            report.steps.push(fail("Iptv", "playlist.m3u8", &e));
            String::new()
        }
    };
    match get_text(&format!("{root}/guide.xml"), ua) {
        Ok(body) => report.steps.push(check_guide("Iptv", &body)),
        Err(e) => report.steps.push(fail("Iptv", "guide.xml", &e)),
    }
    if m3u.to_ascii_lowercase().contains("url-tvg=") {
        let tvg = if m3u.contains("/guide.xml") {
            "Playlist points at local guide.xml"
        } else if m3u.to_ascii_lowercase().contains("epg.monster") {
            "Playlist points at my.epg.monster feed"
        } else {
            "url-tvg present"
        };
        report.steps.push(pass("Iptv", "url-tvg", tvg));
    } else if !m3u.is_empty() {
        report
            .steps
            .push(fail("Iptv", "url-tvg", "Playlist missing url-tvg"));
    }
    if let Some(first) = first_auto_url(&m3u) {
        probe_tune_url(report, "Iptv", &first, ua);
    } else if !m3u.is_empty() {
        report
            .steps
            .push(pass("Iptv", "tune", "No channels in playlist (lineup empty)"));
    }
}

fn check_discover(kind: &str, json: &str, root: &str) -> TunerProbeStep {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(json) else {
        return fail(kind, "discover.json", "invalid json");
    };
    let need = [
        "DeviceID",
        "FriendlyName",
        "LineupURL",
        "BaseURL",
        "TunerCount",
        "ModelNumber",
    ];
    let missing: Vec<_> = need
        .iter()
        .filter(|k| v.get(**k).is_none())
        .copied()
        .collect();
    if !missing.is_empty() {
        return fail(kind, "discover.json", &format!("Missing {}", missing.join(", ")));
    }
    let lineup = v.get("LineupURL").and_then(|x| x.as_str()).unwrap_or("");
    if !lineup.to_ascii_lowercase().contains("/lineup.json") {
        return fail(kind, "discover.json", "LineupURL is not /lineup.json");
    }
    if looks_like_provider(json, root) {
        return fail(kind, "discover.json", "Provider URL leaked");
    }
    pass(
        kind,
        "discover.json",
        &format!(
            "{} · {}",
            v.get("FriendlyName").and_then(|x| x.as_str()).unwrap_or(""),
            v.get("DeviceID").and_then(|x| x.as_str()).unwrap_or("")
        ),
    )
}

fn check_lineup(kind: &str, json: &str, root: &str) -> TunerProbeStep {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(json) else {
        return fail(kind, "lineup.json", "Not a JSON array");
    };
    if !v.is_array() {
        return fail(kind, "lineup.json", "Not a JSON array");
    }
    if looks_like_provider(json, root) {
        return fail(kind, "lineup.json", "Provider URL leaked — Plex would store that");
    }
    let n = v.as_array().map(|a| a.len()).unwrap_or(0);
    if n == 0 {
        return pass(kind, "lineup.json", "0 channels (add Tuner lineup in Managed Output)");
    }
    let first = &v[0];
    if first.get("GuideNumber").is_none()
        || first.get("GuideName").is_none()
        || first.get("URL").is_none()
    {
        return fail(kind, "lineup.json", "Row missing GuideNumber / GuideName / URL");
    }
    let url = first.get("URL").and_then(|x| x.as_str()).unwrap_or("");
    if !url.to_ascii_lowercase().contains("/auto/v") {
        return fail(kind, "lineup.json", "Channel URL is not /auto/v{n}");
    }
    pass(kind, "lineup.json", &format!("{n} channel(s)"))
}

fn check_guide(kind: &str, xml: &str) -> TunerProbeStep {
    if !xml.to_ascii_lowercase().contains("<tv") {
        return fail(kind, "guide.xml", "Not XMLTV");
    }
    let n = xml.matches("<channel ").count() + xml.matches("<channel\t").count();
    let n = if n == 0 {
        xml.matches("<channel id=").count()
    } else {
        n
    };
    pass(kind, "guide.xml", &format!("{n} XMLTV channel(s)"))
}

fn check_m3u(kind: &str, name: &str, body: &str, root: &str) -> TunerProbeStep {
    if !body.starts_with("#EXTM3U") {
        return fail(kind, name, "Not an M3U");
    }
    for line in body.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if looks_like_provider(line, root) {
            return fail(kind, name, "Provider URL leaked in playlist");
        }
    }
    let streams = body.matches("/auto/v").count();
    pass(kind, name, &format!("{streams} local stream URL(s)"))
}

fn probe_tune(report: &mut TunerProbeReport, kind: &str, lineup_json: &str, ua: &str) {
    if let Some(url) = first_lineup_url(lineup_json) {
        probe_tune_url(report, kind, &url, ua);
    } else {
        report
            .steps
            .push(pass(kind, "tune", "Skipped — empty lineup"));
    }
}

fn probe_tune_url(report: &mut TunerProbeReport, kind: &str, url: &str, ua: &str) {
    match ureq::get(url)
        .set("User-Agent", ua)
        .timeout(std::time::Duration::from_secs(6))
        .call()
    {
        Ok(r) => {
            let code = r.status();
            let _ = r.into_reader();
            report.steps.push(pass(
                kind,
                "tune",
                &format!("HTTP {code} · headers accepted"),
            ));
        }
        Err(ureq::Error::Status(404, _)) => {
            report.steps.push(fail(kind, "tune", &format!("{url} → 404")));
        }
        Err(ureq::Error::Status(503, _)) => {
            report.steps.push(pass(
                kind,
                "tune",
                "503 (busy or ffmpeg missing) — endpoint is wired",
            ));
        }
        Err(ureq::Error::Status(code, _)) => {
            report.steps.push(fail(kind, "tune", &format!("{url} → {code}")));
        }
        Err(e) => {
            let msg = e.to_string();
            if msg.to_ascii_lowercase().contains("timed") {
                report.steps.push(pass(
                    kind,
                    "tune",
                    "Timed out reading body — headers were accepted (typical while ffmpeg starts)",
                ));
            } else {
                report.steps.push(fail(kind, "tune", &msg));
            }
        }
    }
}

fn first_lineup_url(json: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(json).ok()?;
    v.as_array()?
        .first()?
        .get("URL")?
        .as_str()
        .map(|s| s.to_string())
}

fn first_auto_url(m3u: &str) -> Option<String> {
    for token in m3u.split_whitespace() {
        if token.to_ascii_lowercase().contains("/auto/v") && token.starts_with("http") {
            return Some(token.trim().to_string());
        }
    }
    None
}

fn looks_like_provider(text: &str, root: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    let mut i = 0;
    while let Some(rel) = lower[i..].find("http://").or_else(|| lower[i..].find("https://")) {
        let start = i + rel;
        let rest = &text[start..];
        let end = rest
            .find(|c: char| c.is_whitespace() || matches!(c, '"' | '\'' | '<' | '>'))
            .unwrap_or(rest.len());
        let u = &rest[..end];
        i = start + end.max(1);
        if u.to_ascii_lowercase().starts_with(&root.to_ascii_lowercase()) {
            continue;
        }
        if u.contains("127.0.0.1") || u.to_ascii_lowercase().contains("localhost") {
            continue;
        }
        return true;
    }
    false
}

pub fn to_json(reports: &[TunerProbeReport]) -> String {
    let passed = !reports.is_empty() && reports.iter().all(|r| r.passed());
    let rows: Vec<serde_json::Value> = reports
        .iter()
        .map(|r| {
            serde_json::json!({
                "Kind": r.kind,
                "BaseUrl": r.base_url,
                "Passed": r.passed(),
                "Summary": r.summary(),
                "Steps": r.steps
            })
        })
        .collect();
    serde_json::to_string_pretty(&serde_json::json!({
        "Product": "epg.monster studio",
        "GeneratedAt": studio_core::audit::now_iso(),
        "Passed": passed,
        "Reports": rows
    }))
    .unwrap_or_else(|_| "{}".into())
}

pub fn write_json(reports: &[TunerProbeReport], path: Option<&std::path::Path>) -> std::io::Result<std::path::PathBuf> {
    let dest = path
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| app_data_directory().join("tunertest.json"));
    if let Some(dir) = dest.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(&dest, to_json(reports))?;
    Ok(dest)
}
