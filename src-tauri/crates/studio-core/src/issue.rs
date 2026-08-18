// SPDX-License-Identifier: GPL-3.0-or-later

use regex::Regex;
use sha2::{Digest, Sha256};
use std::sync::OnceLock;

/// Builds a scrubbed epg.monster.issue v1 payload. Never includes keys or stream URLs.
pub fn redact(text: &str) -> String {
    if text.is_empty() {
        return text.to_string();
    }
    static KEY: OnceLock<Regex> = OnceLock::new();
    static URL: OnceLock<Regex> = OnceLock::new();
    let key = KEY.get_or_init(|| Regex::new(r"(?i)epgm_[A-Za-z0-9_\-]+").expect("key"));
    let url = URL.get_or_init(|| Regex::new(r#"(?i)https?://[^\s"'<>]+"#).expect("url"));
    let mut s = key.replace_all(text, "epgm_***").into_owned();
    s = url
        .replace_all(&s, |caps: &regex::Captures| {
            let full = &caps[0];
            let after = full.splitn(2, "://").nth(1).unwrap_or("");
            let host = after
                .split(['/', '?', '#'])
                .next()
                .unwrap_or("")
                .split(':')
                .next()
                .unwrap_or("")
                .to_ascii_lowercase();
            let allowed = [
                "epg.monster",
                "github.com",
                "microsoft.com",
                "windows.com",
                "nuget.org",
            ];
            if allowed.iter().any(|a| host == *a || host.ends_with(&format!(".{a}"))) {
                full.to_string()
            } else {
                let scheme = if full.to_ascii_lowercase().starts_with("https://") {
                    "https://"
                } else {
                    "http://"
                };
                format!("{scheme}[redacted]")
            }
        })
        .into_owned();
    if let Some(home) = dirs::home_dir() {
        let home_s = home.to_string_lossy();
        if !home_s.is_empty() {
            s = replace_ignore_case(&s, &home_s, "%USERPROFILE%");
        }
    }
    s
}

fn replace_ignore_case(hay: &str, needle: &str, with: &str) -> String {
    let lower = hay.to_ascii_lowercase();
    let n = needle.to_ascii_lowercase();
    let mut out = String::new();
    let mut i = 0;
    while let Some(pos) = lower[i..].find(&n) {
        out.push_str(&hay[i..i + pos]);
        out.push_str(with);
        i += pos + needle.len();
    }
    out.push_str(&hay[i..]);
    out
}

pub fn build(
    kind: &str,
    title: &str,
    summary: Option<&str>,
    details: Option<&str>,
    studio_version: &str,
    feed_slug: Option<&str>,
    channel_count: Option<i32>,
    notes: Option<&str>,
    member_username: Option<&str>,
) -> serde_json::Value {
    let clean_title = redact(if title.trim().is_empty() { "Crash" } else { title });
    let clean_details = redact(details.unwrap_or(""));
    let clean_summary = redact(summary.unwrap_or(""));
    let clean_notes = combine_notes(notes, &clean_summary);
    let (err_type, err_message) = split_error(&clean_title, &clean_summary, &clean_details);
    let created = time::OffsetDateTime::now_utc();
    let created_at = format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}Z",
        created.year(),
        created.month() as u8,
        created.day(),
        created.hour(),
        created.minute(),
        created.second(),
        created.millisecond()
    );
    serde_json::json!({
        "schema": "epg.monster.issue",
        "version": 1,
        "kind": if kind.trim().is_empty() { "crash" } else { kind },
        "createdAt": created_at,
        "title": clean_title,
        "notes": clean_notes,
        "client": {
            "name": "epg.monster Studio",
            "version": studio_version,
            "os": std::env::consts::OS,
            "osVersion": std::env::consts::OS,
            "arch": if cfg!(target_pointer_width = "64") { "x64" } else { "x86" },
            "runtime": "tauri/rust"
        },
        "user": {
            "memberUsername": member_username.map(str::trim).filter(|s| !s.is_empty()),
            "feedSlug": feed_slug
        },
        "error": {
            "type": err_type,
            "message": err_message,
            "stackTrace": truncate(&clean_details, 8000),
            "fingerprint": fingerprint(&err_type, &clean_details)
        },
        "context": {
            "screen": "crash",
            "channelCount": channel_count
        },
        "environment": {
            "locale": std::env::var("LANG").unwrap_or_default(),
            "timezone": "local"
        }
    })
}

fn combine_notes(user_notes: Option<&str>, clean_summary: &str) -> Option<String> {
    let note = redact(user_notes.unwrap_or("")).trim().to_string();
    if note.is_empty() {
        return if clean_summary.trim().is_empty() {
            None
        } else {
            Some(clean_summary.to_string())
        };
    }
    if clean_summary.trim().is_empty() || note.contains(clean_summary) {
        Some(note)
    } else {
        Some(format!("{note}\n\n{clean_summary}"))
    }
}

fn split_error(title: &str, summary: &str, details: &str) -> (String, String) {
    let first = details
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or(title)
        .to_string();
    if let Some(idx) = first.find(':') {
        if first[..idx].to_ascii_lowercase().contains("exception") {
            return (first[..idx].trim().to_string(), first[idx + 1..].trim().to_string());
        }
    }
    (
        title.to_string(),
        if summary.trim().is_empty() {
            first
        } else {
            summary.to_string()
        },
    )
}

fn fingerprint(err_type: &str, details: &str) -> String {
    let line = details
        .lines()
        .map(str::trim)
        .find(|l| l.contains(" at ") || l.contains("Exception"))
        .unwrap_or(err_type);
    let raw = format!("{err_type}|{line}");
    let hash = Sha256::digest(raw.as_bytes());
    hash[..8].iter().map(|b| format!("{b:02x}")).collect()
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    let mut end = max.min(s.len());
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &s[..end])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_keys_and_provider_urls() {
        let s = redact("key=epgm_secretTOKEN stream=http://provider.example/live.ts");
        assert!(!s.contains("epgm_secretTOKEN"));
        assert!(s.contains("epgm_***"));
        assert!(s.contains("[redacted]"));
        assert!(!s.contains("provider.example"));
    }

    #[test]
    fn build_matches_handshake_schema() {
        let payload = build(
            "crash",
            "NullReferenceException: boom",
            Some("export failed"),
            Some("System.NullReferenceException: boom\n   at EpgMonsterStudio.Pages.ManagedOutputPage.Export\nkey=epgm_secretTOKEN http://provider.example/live.ts"),
            crate::VERSION,
            Some("example"),
            Some(2035),
            Some("exporting channels.json"),
            Some("example-user"),
        );
        let json = serde_json::to_string(&payload).unwrap();
        assert_eq!(payload["schema"], "epg.monster.issue");
        assert_eq!(payload["version"], 1);
        assert_eq!(payload["kind"], "crash");
        assert_eq!(payload["client"]["name"], "epg.monster Studio");
        assert_eq!(payload["user"]["memberUsername"], "example-user");
        assert_eq!(payload["user"]["feedSlug"], "example");
        assert_eq!(payload["context"]["channelCount"], 2035);
        assert_eq!(payload["context"]["screen"], "crash");
        assert!(payload["notes"].as_str().unwrap().contains("exporting channels.json"));
        assert!(!payload["error"]["fingerprint"].as_str().unwrap().is_empty());
        assert!(!json.contains("epgm_secretTOKEN"));
        assert!(!json.contains("provider.example"));
    }

    #[test]
    fn truncate_does_not_panic_on_utf8_boundary() {
        let s = "é".repeat(5000);
        let t = truncate(&s, 8000);
        assert!(t.ends_with('…'));
        assert!(t.is_char_boundary(t.len()));
    }
}
