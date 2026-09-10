// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::epg::tvg_lookup_ids;
use crate::hdhr::xmltv_time;
use crate::models::{EpgProgramme, ManagedChannel};

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Playlist tvg-ids plus catalog aliases so `list_programmes` can find rows.
pub fn programme_lookup_ids(channels: &[ManagedChannel]) -> Vec<String> {
    let mut ids = Vec::new();
    let mut seen = HashSet::new();
    for ch in channels {
        let Some(id) = ch.tvg_id.as_deref() else {
            continue;
        };
        for alias in tvg_lookup_ids(id) {
            let key = alias.to_ascii_lowercase();
            if seen.insert(key) {
                ids.push(alias);
            }
        }
    }
    ids
}

pub fn write_guide_xmltv(
    path: &Path,
    channels: &[ManagedChannel],
    programmes: &[EpgProgramme],
) -> Result<(), String> {
    std::fs::write(path, export_guide_xmltv(channels, programmes)).map_err(|e| e.to_string())
}

/// XMLTV for a curated playlist: `channel` / `programme@channel` use the playlist tvg-id.
pub fn export_guide_xmltv(channels: &[ManagedChannel], programmes: &[EpgProgramme]) -> String {
    let mut by_tvg: HashMap<String, Vec<&EpgProgramme>> = HashMap::new();
    for p in programmes {
        let key = p.tvg_id.trim().to_ascii_lowercase();
        if key.is_empty() {
            continue;
        }
        by_tvg.entry(key).or_default().push(p);
    }
    let mut sb = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<tv generator-info-name=\"epg.monster studio\">\n");
    for ch in channels {
        if ch.hidden {
            continue;
        }
        let id = ch
            .tvg_id
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or("");
        if id.is_empty() {
            continue;
        }
        let name = if ch.name.trim().is_empty() {
            id
        } else {
            ch.name.trim()
        };
        sb.push_str("  <channel id=\"");
        sb.push_str(&xml_escape(id));
        sb.push_str("\">\n    <display-name>");
        sb.push_str(&xml_escape(name));
        sb.push_str("</display-name>\n  </channel>\n");
    }
    for ch in channels {
        if ch.hidden {
            continue;
        }
        let id = ch
            .tvg_id
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or("");
        if id.is_empty() {
            continue;
        }
        let mut seen: HashSet<(String, String)> = HashSet::new();
        for alias in tvg_lookup_ids(id) {
            let Some(list) = by_tvg.get(&alias.to_ascii_lowercase()) else {
                continue;
            };
            for p in list {
                if !seen.insert((p.start_utc.clone(), p.title.clone())) {
                    continue;
                }
                sb.push_str("  <programme start=\"");
                sb.push_str(&xmltv_time(&p.start_utc));
                sb.push_str("\" stop=\"");
                sb.push_str(&xmltv_time(&p.stop_utc));
                sb.push_str("\" channel=\"");
                sb.push_str(&xml_escape(id));
                sb.push_str("\">\n    <title>");
                sb.push_str(&xml_escape(&p.title));
                sb.push_str("</title>\n  </programme>\n");
            }
        }
    }
    sb.push_str("</tv>\n");
    sb
}

fn escape_attr(s: &str) -> String {
    s.replace('"', "'")
}

fn write_extinf(out: &mut String, ch: &ManagedChannel, display: &str) {
    let mut attrs = Vec::new();
    if let Some(id) = ch.tvg_id.as_deref().filter(|s| !s.is_empty()) {
        attrs.push(format!("tvg-id=\"{}\"", escape_attr(id)));
    }
    if !ch.name.is_empty() {
        attrs.push(format!("tvg-name=\"{}\"", escape_attr(&ch.name)));
    }
    if let Some(logo) = ch.tvg_logo.as_deref().filter(|s| !s.is_empty()) {
        attrs.push(format!("tvg-logo=\"{}\"", escape_attr(logo)));
    }
    if ch.tvg_shift_hours.abs() > 0.0001 {
        attrs.push(format!("tvg-shift=\"{}\"", ch.tvg_shift_hours));
    }
    attrs.push(format!("group-title=\"{}\"", escape_attr(&ch.group_title)));
    out.push_str("#EXTINF:-1 ");
    out.push_str(&attrs.join(" "));
    out.push(',');
    out.push_str(display);
    out.push('\n');
}

fn visible_variant(ch: &ManagedChannel) -> Option<&crate::models::StreamVariant> {
    ch.variants
        .iter()
        .find(|v| v.visibility == "visible" && !v.url.trim().is_empty())
}

/// Refuse to mount G-houl when the curated lineup cannot play.
pub fn ghoul_mount_gate(channels: &[ManagedChannel]) -> Result<(), String> {
    if channels.is_empty() {
        return Err("Load a curated playlist in Playlist Editor first.".into());
    }
    if channels
        .iter()
        .filter(|ch| !ch.hidden)
        .all(|ch| visible_variant(ch).is_none())
    {
        return Err("Curated channels have no playable stream URLs.".into());
    }
    Ok(())
}

/// Visible-only M3U snapshot for G-houl, with per-row UA and extra headers as
/// `#EXTVLCOPT:` / `#EXTHTTP:` tags so both hosts share one parse path.
pub fn export_ghoul_snapshot(
    channels: &[ManagedChannel],
    headers_by_variant: &HashMap<String, Vec<(String, String)>>,
) -> String {
    let mut sb = String::from("#EXTM3U\n");
    let mut list = channels.to_vec();
    list.sort_by(|a, b| {
        a.sort_order
            .cmp(&b.sort_order)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    for ch in list {
        if ch.hidden {
            continue;
        }
        let Some(v) = visible_variant(&ch) else {
            continue;
        };
        write_extinf(&mut sb, &ch, &ch.name);
        let headers = headers_by_variant.get(&v.id).cloned().unwrap_or_default();
        let mut ua = String::new();
        let mut extra: Vec<(String, String)> = Vec::new();
        for (k, val) in headers {
            if k.eq_ignore_ascii_case("user-agent") {
                ua = val;
            } else if !val.trim().is_empty() {
                extra.push((k, val));
            }
        }
        if !ua.trim().is_empty() {
            sb.push_str("#EXTVLCOPT:http-user-agent=");
            sb.push_str(ua.trim());
            sb.push('\n');
        }
        for (k, val) in &extra {
            if k.eq_ignore_ascii_case("referer") || k.eq_ignore_ascii_case("referrer") {
                sb.push_str("#EXTVLCOPT:http-referrer=");
                sb.push_str(val.trim());
                sb.push('\n');
            }
        }
        let others: Vec<(String, String)> = extra
            .into_iter()
            .filter(|(k, _)| {
                !k.eq_ignore_ascii_case("referer") && !k.eq_ignore_ascii_case("referrer")
            })
            .collect();
        if !others.is_empty() {
            sb.push_str("#EXTHTTP:{");
            for (i, (k, val)) in others.iter().enumerate() {
                if i > 0 {
                    sb.push(',');
                }
                sb.push('"');
                sb.push_str(&json_escape(k));
                sb.push_str("\":\"");
                sb.push_str(&json_escape(val));
                sb.push('"');
            }
            sb.push_str("}\n");
        }
        sb.push_str(&v.url);
        sb.push('\n');
    }
    sb
}

fn json_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

pub fn export_visible_only(channels: &[ManagedChannel]) -> String {
    let mut sb = String::from("#EXTM3U\n");
    let mut list = channels.to_vec();
    list.sort_by(|a, b| {
        a.sort_order
            .cmp(&b.sort_order)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    for ch in list {
        if ch.hidden {
            continue;
        }
        let url = ch
            .variants
            .iter()
            .find(|v| v.visibility == "visible")
            .or_else(|| ch.variants.first())
            .map(|v| v.url.as_str())
            .filter(|u| !u.is_empty());
        let Some(url) = url else { continue };
        write_extinf(&mut sb, &ch, &ch.name);
        sb.push_str(url);
        sb.push('\n');
    }
    sb
}

pub fn export_all(channels: &[ManagedChannel]) -> String {
    let mut sb = String::from("#EXTM3U\n");
    let mut list = channels.to_vec();
    list.sort_by(|a, b| {
        a.sort_order
            .cmp(&b.sort_order)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    for ch in list {
        let mut variants = ch.variants.clone();
        variants.sort_by(|a, b| {
            let av = if a.visibility == "visible" { 0 } else { 1 };
            let bv = if b.visibility == "visible" { 0 } else { 1 };
            av.cmp(&bv).then(a.priority.cmp(&b.priority))
        });
        let mut backup = 0;
        for v in variants.iter().filter(|v| !v.url.is_empty()) {
            let display = if v.visibility == "visible" {
                ch.name.clone()
            } else {
                backup += 1;
                let tag = v
                    .label
                    .as_deref()
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| format!("backup {backup}"));
                format!("{} ({tag})", ch.name)
            };
            write_extinf(&mut sb, &ch, &display);
            sb.push_str(&v.url);
            sb.push('\n');
        }
    }
    sb
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{ManagedChannel, StreamVariant};

    fn ch() -> ManagedChannel {
        ManagedChannel {
            id: "1".into(),
            name: "CNN".into(),
            group_title: "NEWS".into(),
            tvg_id: Some("CNN.us".into()),
            tvg_logo: None,
            notes: None,
            sort_order: 1,
            tvg_shift_hours: 0.0,
            in_tuner: false,
            hidden: false,
            tuner_number: None,
            variants: vec![
                StreamVariant {
                    id: "v1".into(),
                    managed_channel_id: "1".into(),
                    url: "http://vis".into(),
                    label: Some("A".into()),
                    source_entry_id: None,
                    origin_name: None,
                    origin_tvg_id: None,
                    visibility: "visible".into(),
                    priority: 0,
                    last_audit_ok: None,
                    last_audit_at: None,
                },
                StreamVariant {
                    id: "v2".into(),
                    managed_channel_id: "1".into(),
                    url: "http://bak".into(),
                    label: Some("B".into()),
                    source_entry_id: None,
                    origin_name: None,
                    origin_tvg_id: None,
                    visibility: "hidden_backup".into(),
                    priority: 1,
                    last_audit_ok: None,
                    last_audit_at: None,
                },
            ],
            has_epg_match: false,
        }
    }

    #[test]
    fn ghoul_snapshot_visible_only_with_ua_and_headers() {
        let row = ch();
        let mut headers = HashMap::new();
        headers.insert(
            "v1".into(),
            vec![
                ("User-Agent".into(), "Foo/1".into()),
                ("Referer".into(), "http://ref".into()),
                ("X-Token".into(), "abc".into()),
            ],
        );
        let s = export_ghoul_snapshot(&[row], &headers);
        assert!(s.contains("http://vis"));
        assert!(!s.contains("http://bak"));
        assert!(s.contains("#EXTVLCOPT:http-user-agent=Foo/1"));
        assert!(s.contains("#EXTVLCOPT:http-referrer=http://ref"));
        assert!(s.contains("#EXTHTTP:"));
        assert!(s.contains("X-Token"));
        assert!(s.contains("abc"));
    }

    #[test]
    fn ghoul_gate_empty_and_url_less() {
        assert_eq!(
            ghoul_mount_gate(&[]).unwrap_err(),
            "Load a curated playlist in Playlist Editor first."
        );
        let mut row = ch();
        row.variants[0].url.clear();
        row.variants[1].url.clear();
        assert_eq!(
            ghoul_mount_gate(&[row]).unwrap_err(),
            "Curated channels have no playable stream URLs."
        );
        assert!(ghoul_mount_gate(&[ch()]).is_ok());
    }

    #[test]
    fn visible_export_omits_hidden_channels() {
        let mut row = ch();
        row.hidden = true;
        let hidden = export_visible_only(&[row.clone()]);
        assert!(!hidden.contains("http://vis"));
        assert!(!hidden.contains("CNN"));
        row.hidden = false;
        let shown = export_visible_only(&[row]);
        assert!(shown.contains("http://vis"));
        assert!(shown.contains("CNN"));
    }

    #[test]
    fn visible_export_omits_backups() {
        let s = export_visible_only(&[ch()]);
        assert!(s.contains("http://vis"));
        assert!(!s.contains("http://bak"));
        assert!(s.contains("tvg-id=\"CNN.us\""));
        assert!(s.contains("group-title=\"NEWS\""));
    }

    #[test]
    fn all_export_includes_labeled_backup() {
        let s = export_all(&[ch()]);
        assert!(s.contains("http://vis"));
        assert!(s.contains("http://bak"));
        assert!(s.contains("CNN (B)"));
    }

    #[test]
    fn programme_lookup_ids_includes_aliases() {
        let mut row = ch();
        row.tvg_id = Some("KAUT-DT.us_locals1.us (src05)".into());
        let ids = programme_lookup_ids(&[row]);
        assert!(ids.iter().any(|s| s.eq_ignore_ascii_case("KAUT-DT.us")));
        assert!(ids.iter().any(|s| s.contains("src05")));
    }

    #[test]
    fn write_guide_xmltv_writes_sliced_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("iptv.xml");
        let mut row = ch();
        row.tvg_id = Some("KAUT-DT.us_locals1.us (src05)".into());
        let programmes = [crate::models::EpgProgramme {
            tvg_id: "KAUT-DT.us".into(),
            title: "Local News".into(),
            description: None,
            start_utc: "2026-08-19T20:00:00Z".into(),
            stop_utc: "2026-08-19T21:00:00Z".into(),
        }];
        write_guide_xmltv(&path, &[row], &programmes).unwrap();
        let xml = std::fs::read_to_string(&path).unwrap();
        assert!(xml.contains("Local News"));
        assert!(xml.contains("channel=\"KAUT-DT.us_locals1.us (src05)\""));
        assert!(xml.contains("generator-info-name=\"epg.monster studio\""));
    }

    #[test]
    fn guide_xmltv_uses_playlist_tvg_id() {
        let mut row = ch();
        row.tvg_id = Some("KAUT-DT.us_locals1.us (src05)".into());
        let programmes = [crate::models::EpgProgramme {
            tvg_id: "KAUT-DT.us".into(),
            title: "Local News".into(),
            description: None,
            start_utc: "2026-08-19T20:00:00Z".into(),
            stop_utc: "2026-08-19T21:00:00Z".into(),
        }];
        let xml = export_guide_xmltv(&[row], &programmes);
        assert!(xml.contains("channel=\"KAUT-DT.us_locals1.us (src05)\""));
        assert!(xml.contains("Local News"));
        assert!(xml.contains("channel id=\"KAUT-DT.us_locals1.us (src05)\""));
    }
}
