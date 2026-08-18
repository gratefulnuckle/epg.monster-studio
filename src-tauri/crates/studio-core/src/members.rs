// SPDX-License-Identifier: GPL-3.0-or-later

use serde::{Deserialize, Serialize};

use crate::curation::{self, CurationBuildResult};
use crate::info::VERSION;
use crate::models::ManagedChannel;

pub const DEFAULT_BASE: &str = "https://epg.monster";
pub const DEFAULT_MAX_CHANNELS: i32 = 2500;
pub const DEFAULT_MAX_BODY_BYTES: i32 = 3_145_728;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemberPingResult {
    pub ok: bool,
    pub status_code: i32,
    pub message: String,
    pub email: Option<String>,
    pub username: Option<String>,
    pub feed_url: Option<String>,
    pub feed_url_gz: Option<String>,
    pub slug: Option<String>,
    pub channel_count: Option<i32>,
    pub build_status: Option<String>,
    pub max_channels: Option<i32>,
    pub max_body_bytes: Option<i32>,
    pub received: Option<i32>,
    pub unique: Option<i32>,
    pub matched: Option<i32>,
    pub skipped_missing_tvg_id: Option<i32>,
    pub duplicates_collapsed: Option<i32>,
    pub rebuild_queued: bool,
    pub job_id: Option<String>,
    pub unknown_count: Option<i32>,
    pub unknown_tvg_ids: Vec<String>,
}

pub fn normalize_base(raw: &str) -> String {
    let mut s = raw.trim().trim_end_matches('/').to_string();
    if s.is_empty() {
        return DEFAULT_BASE.into();
    }
    if !s.to_ascii_lowercase().starts_with("http") {
        s = format!("https://{s}");
    }
    s.trim_end_matches('/').to_string()
}

pub fn user_agent(version: Option<&str>) -> String {
    let v = version.map(str::trim).filter(|s| !s.is_empty()).unwrap_or(VERSION);
    format!("epg.monster-studio/{v}")
}

pub fn format_publish_report(built: &CurationBuildResult, result: &MemberPingResult) -> String {
    let matched = result.matched.unwrap_or(0);
    let unknown_n = result.unknown_count.unwrap_or_else(|| {
        match (result.unique, result.matched) {
            (Some(u), Some(m)) => (u - m).max(0),
            _ => result.unknown_tvg_ids.len() as i32,
        }
    });
    let feed = result.feed_url.clone().unwrap_or_default();
    let mut lines = vec![
        if result.ok {
            if feed.is_empty() {
                format!("Uploaded {matched} channel(s)")
            } else {
                format!("Uploaded {matched} channel(s) · {feed}")
            }
        } else {
            result.message.clone()
        },
        format!("{unknown_n} unknown tvg-id(s)"),
        format!(
            "{} curated rows · {} unique tvg-ids sent · cap {}",
            result.received.unwrap_or(built.included),
            result.unique.unwrap_or(built.included),
            built.cap
        ),
        format!(
            "{} empty tvg-id · {} duplicate tvg-ids",
            result
                .skipped_missing_tvg_id
                .unwrap_or(built.skipped_no_tvg_id),
            result
                .duplicates_collapsed
                .unwrap_or(built.skipped_duplicate)
        ),
    ];
    if let Some(gz) = result.feed_url_gz.as_deref().filter(|s| !s.is_empty()) {
        lines.push(format!("gzip: {gz}"));
    }
    if !result.unknown_tvg_ids.is_empty() {
        let sample: Vec<_> = result.unknown_tvg_ids.iter().take(30).cloned().collect();
        lines.push(format!("Unknown sample: {}", sample.join(", ")));
    }
    if built.over_cap > 0 {
        lines.push(format!(
            "Trimmed {} unique id(s) to stay under cap {}.",
            built.over_cap, built.cap
        ));
    }
    if let Some(st) = result.build_status.as_deref().filter(|s| !s.is_empty()) {
        lines.push(format!("Build: {st}"));
    }
    lines.join("\n")
}

fn json_str(v: &serde_json::Value, key: &str) -> Option<String> {
    v.get(key)
        .and_then(|x| x.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

fn json_i32(v: &serde_json::Value, key: &str) -> Option<i32> {
    v.get(key).and_then(|x| {
        x.as_i64()
            .map(|n| n as i32)
            .or_else(|| x.as_str().and_then(|s| s.parse().ok()))
    })
}

fn fail(code: i32, verb: &str, body: &str) -> MemberPingResult {
    let msg = match code {
        401 => format!("{verb} failed — access key rejected (401)."),
        403 => format!("{verb} failed — account not verified or not active (403)."),
        404 => format!(
            "{verb} failed — HTTP 404 ({} path not found).",
            verb.to_ascii_lowercase()
        ),
        413 => format!("{verb} failed — body too large (413)."),
        422 => format!("{verb} failed — schema or over cap (422). {}", truncate(body)),
        429 => format!("{verb} failed — rate limited (429)."),
        500 => format!("{verb} failed — server error (500). {}", truncate(body)),
        _ => format!("{verb} failed — HTTP {code}. {}", truncate(body)),
    };
    MemberPingResult {
        status_code: code,
        message: msg,
        ..MemberPingResult::default()
    }
}

fn truncate(s: &str) -> String {
    let t = s.trim();
    if t.len() <= 240 {
        t.to_string()
    } else {
        format!("{}…", &t[..240])
    }
}

fn send(
    method: &str,
    url: &str,
    key: &str,
    version: Option<&str>,
    body: Option<&str>,
    timeout_sec: u64,
) -> Result<(u16, String), String> {
    let ua = user_agent(version);
    let mut req = match method {
        "PUT" => ureq::put(url),
        "POST" => ureq::post(url),
        _ => ureq::get(url),
    };
    req = req
        .set("Authorization", &format!("Bearer {key}"))
        .set("X-EPG-Member-Key", key)
        .set("User-Agent", &ua)
        .timeout(std::time::Duration::from_secs(timeout_sec));
    let resp = if let Some(json) = body {
        req.set("Content-Type", "application/json").send_string(json)
    } else {
        req.call()
    };
    match resp {
        Ok(r) => {
            let code = r.status();
            let text = r.into_string().unwrap_or_default();
            Ok((code, text))
        }
        Err(ureq::Error::Status(code, r)) => {
            let text = r.into_string().unwrap_or_default();
            Ok((code, text))
        }
        Err(e) => Err(e.to_string()),
    }
}

pub fn ping(api_base: &str, access_key: &str, studio_version: Option<&str>) -> MemberPingResult {
    let key = access_key.trim();
    if key.is_empty() {
        return MemberPingResult {
            message: "Paste an access key from my.epg.monster → Keys (starts with epgm_).".into(),
            ..MemberPingResult::default()
        };
    }
    let url = format!("{}/api/member/v1/ping", normalize_base(api_base));
    match send("GET", &url, key, studio_version, None, 15) {
        Ok((code, body)) if (200..300).contains(&code) => parse_ping(&body, code as i32),
        Ok((code, body)) => fail(code as i32, "Ping", &body),
        Err(e) => MemberPingResult {
            message: e,
            ..MemberPingResult::default()
        },
    }
}

fn parse_ping(body: &str, code: i32) -> MemberPingResult {
    let Ok(root) = serde_json::from_str::<serde_json::Value>(body) else {
        return MemberPingResult {
            ok: true,
            status_code: code,
            message: "Key accepted (unparsed body)".into(),
            ..MemberPingResult::default()
        };
    };
    let feed = root.get("feed");
    let limits = root.get("limits");
    let ok = root.get("ok").and_then(|v| v.as_bool()).unwrap_or(true);
    let email = json_str(&root, "email");
    let feed_url = feed.and_then(|f| json_str(f, "feedUrl").or_else(|| json_str(f, "feedUrlXml")));
    let mut msg = if ok {
        match &email {
            Some(e) => format!("Key valid · {e}"),
            None => "Key valid".into(),
        }
    } else {
        "Ping returned ok=false".into()
    };
    if let Some(u) = &feed_url {
        msg.push_str(" · ");
        msg.push_str(u);
    }
    MemberPingResult {
        ok,
        status_code: code,
        email,
        username: json_str(&root, "username").or_else(|| json_str(&root, "memberUsername")),
        feed_url,
        feed_url_gz: feed.and_then(|f| json_str(f, "feedUrlGz")),
        slug: feed.and_then(|f| json_str(f, "slug")),
        channel_count: feed.and_then(|f| json_i32(f, "channelCount")),
        build_status: feed.and_then(|f| json_str(f, "buildStatus")),
        max_channels: limits
            .and_then(|l| json_i32(l, "maxChannels"))
            .or(Some(DEFAULT_MAX_CHANNELS)),
        max_body_bytes: limits
            .and_then(|l| json_i32(l, "maxBodyBytes"))
            .or(Some(DEFAULT_MAX_BODY_BYTES)),
        message: msg,
        ..MemberPingResult::default()
    }
}

fn parse_put(body: &str, code: i32, sent: i32) -> MemberPingResult {
    let Ok(root) = serde_json::from_str::<serde_json::Value>(body) else {
        return MemberPingResult {
            ok: true,
            status_code: code,
            message: format!("Uploaded {sent} channel(s) (unparsed body)"),
            ..MemberPingResult::default()
        };
    };
    let report = root.get("report").cloned().unwrap_or(serde_json::Value::Null);
    let unknown = try_unknown(&root);
    let matched = json_i32(&root, "channelCount").or_else(|| json_i32(&report, "matched"));
    let unique = json_i32(&root, "requestedCount").or_else(|| json_i32(&report, "unique"));
    let received = json_i32(&report, "received").or(Some(sent));
    let feed_url = json_str(&root, "feedUrl").or_else(|| json_str(&root, "feedUrlXml"));
    let unknown_count = json_i32(&report, "unknownCount").unwrap_or(unknown.len() as i32);
    let extra = if unknown_count > 0 {
        format!(" · {unknown_count} unknown tvg-id(s)")
    } else {
        String::new()
    };
    let shown = json_i32(&report, "matched").or(matched).unwrap_or(sent);
    let mut message = format!("Uploaded {shown} channel(s)");
    if let Some(u) = &feed_url {
        message.push_str(" · ");
        message.push_str(u);
    }
    message.push_str(&extra);
    MemberPingResult {
        ok: true,
        status_code: code,
        feed_url,
        feed_url_gz: json_str(&root, "feedUrlGz"),
        slug: json_str(&root, "slug"),
        channel_count: matched,
        build_status: json_str(&root, "buildStatus"),
        received,
        unique,
        matched: json_i32(&report, "matched").or(matched),
        skipped_missing_tvg_id: json_i32(&report, "skippedMissingTvgId"),
        duplicates_collapsed: json_i32(&report, "duplicatesCollapsed"),
        rebuild_queued: root
            .get("rebuildQueued")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        job_id: json_str(&root, "jobId"),
        unknown_count: Some(unknown_count),
        unknown_tvg_ids: unknown,
        message,
        ..MemberPingResult::default()
    }
}

fn try_unknown(root: &serde_json::Value) -> Vec<String> {
    let keys = ["unknownTvgIds", "unknown", "missingTvgIds"];
    for k in keys {
        if let Some(arr) = root.get(k).and_then(|v| v.as_array()) {
            return arr
                .iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect();
        }
        if let Some(arr) = root
            .get("report")
            .and_then(|r| r.get(k))
            .and_then(|v| v.as_array())
        {
            return arr
                .iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect();
        }
    }
    Vec::new()
}

pub fn put_channels(
    api_base: &str,
    access_key: &str,
    document: &curation::CurationDocument,
    max_channels: Option<i32>,
    max_body_bytes: Option<i32>,
    studio_version: Option<&str>,
) -> MemberPingResult {
    let key = access_key.trim();
    if key.is_empty() {
        return MemberPingResult {
            message: "No access key in Settings. Open Settings → my.epg.monster, paste the key, Test, then Save.".into(),
            ..MemberPingResult::default()
        };
    }
    let cap = max_channels.filter(|n| *n > 0).unwrap_or(DEFAULT_MAX_CHANNELS);
    let mut to_send = document.clone();
    if to_send.channels.len() as i32 > cap {
        to_send.channels.truncate(cap as usize);
    }
    let json = curation::to_json(&to_send);
    let bytes = json.len();
    let body_cap = max_body_bytes.filter(|n| *n > 0).unwrap_or(DEFAULT_MAX_BODY_BYTES);
    if bytes as i32 > body_cap {
        return MemberPingResult {
            status_code: 413,
            message: format!(
                "Upload too large ({bytes} bytes, limit {body_cap}). Remove channels or logos and try again."
            ),
            ..MemberPingResult::default()
        };
    }
    let url = format!("{}/api/member/v1/feed/channels", normalize_base(api_base));
    match send("PUT", &url, key, studio_version, Some(&json), 90) {
        Ok((code, body)) if (200..300).contains(&code) => {
            parse_put(&body, code as i32, to_send.channels.len() as i32)
        }
        Ok((code, body)) => fail(code as i32, "Upload", &body),
        Err(e) => MemberPingResult {
            message: e,
            ..MemberPingResult::default()
        },
    }
}

pub fn publish_lineup(
    api_base: &str,
    access_key: &str,
    channels: &[ManagedChannel],
    studio_version: &str,
) -> (CurationBuildResult, MemberPingResult) {
    let pinged = ping(api_base, access_key, Some(studio_version));
    let cap = pinged.max_channels.filter(|n| *n > 0).unwrap_or(DEFAULT_MAX_CHANNELS);
    let built = curation::build(channels, studio_version, None, Some(cap));
    if built.included == 0 {
        return (
            built,
            MemberPingResult {
                message: "No channels with a tvg-id to upload.".into(),
                ..MemberPingResult::default()
            },
        );
    }
    let mut put = put_channels(
        api_base,
        access_key,
        &built.document,
        Some(cap),
        pinged.max_body_bytes.or(Some(DEFAULT_MAX_BODY_BYTES)),
        Some(studio_version),
    );
    if put.feed_url.is_none() {
        put.feed_url = pinged.feed_url;
    }
    if put.feed_url_gz.is_none() {
        put.feed_url_gz = pinged.feed_url_gz;
    }
    put.max_channels = Some(cap);
    put.max_body_bytes = pinged.max_body_bytes;
    if put.ok && put.rebuild_queued {
        let job = poll_job(api_base, access_key, Some(studio_version), 12);
        put.ok = put.ok && (job.ok || job.status_code == 404);
        if job.build_status.is_some() {
            put.build_status = job.build_status;
        }
    }
    (built, put)
}

fn poll_job(api_base: &str, access_key: &str, version: Option<&str>, attempts: i32) -> MemberPingResult {
    let key = access_key.trim();
    if key.is_empty() {
        return MemberPingResult {
            message: "No access key.".into(),
            ..MemberPingResult::default()
        };
    }
    let url = format!("{}/api/member/v1/feed/jobs/latest", normalize_base(api_base));
    let mut last = "queued".to_string();
    for _ in 0..attempts.max(1) {
        match send("GET", &url, key, version, None, 15) {
            Ok((404, _)) => {
                return MemberPingResult {
                    ok: true,
                    status_code: 404,
                    build_status: Some("none".into()),
                    message: "No build job yet.".into(),
                    ..MemberPingResult::default()
                };
            }
            Ok((code, body)) if !(200..300).contains(&code) => {
                return fail(code as i32, "Job poll", &body);
            }
            Ok((code, body)) => {
                let status = serde_json::from_str::<serde_json::Value>(&body)
                    .ok()
                    .and_then(|v| json_str(&v, "status").or_else(|| json_str(&v, "buildStatus")))
                    .unwrap_or_else(|| "unknown".into());
                last = status.clone();
                if matches!(status.as_str(), "done" | "ready" | "complete" | "completed") {
                    return MemberPingResult {
                        ok: true,
                        status_code: code as i32,
                        build_status: Some(status),
                        message: "Personal EPG ready.".into(),
                        ..MemberPingResult::default()
                    };
                }
                if matches!(status.as_str(), "failed" | "error") {
                    return MemberPingResult {
                        status_code: code as i32,
                        build_status: Some(status),
                        message: "Personal EPG build failed.".into(),
                        ..MemberPingResult::default()
                    };
                }
            }
            Err(e) => last = e,
        }
        std::thread::sleep(std::time::Duration::from_secs(2));
    }
    MemberPingResult {
        ok: true,
        build_status: Some(last.clone()),
        message: format!("Build still running ({last}). Check my.epg.monster."),
        ..MemberPingResult::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_base_defaults_and_strips_slash() {
        assert_eq!(normalize_base(""), "https://epg.monster");
        assert_eq!(normalize_base("https://epg.monster/"), "https://epg.monster");
        assert_eq!(normalize_base("my.epg.monster"), "https://my.epg.monster");
    }

    #[test]
    fn format_publish_report_uses_unknown_count_not_sample_length() {
        let built = curation::build(&[], VERSION, None, None);
        let result = MemberPingResult {
            ok: true,
            matched: Some(1192),
            received: Some(2035),
            unique: Some(1803),
            unknown_count: Some(611),
            unknown_tvg_ids: (1..=30).map(|i| format!("id.{i}")).collect(),
            feed_url: Some("https://my.epg.monster/example".into()),
            skipped_missing_tvg_id: Some(0),
            duplicates_collapsed: Some(232),
            ..MemberPingResult::default()
        };
        let text = format_publish_report(&built, &result);
        assert!(text.contains("611 unknown tvg-id(s)"));
        assert!(!text.contains("30 unknown tvg-id(s)"));
    }
}
