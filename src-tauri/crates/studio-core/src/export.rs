// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::{HashMap, HashSet};

use crate::epg::tvg_lookup_ids;
use crate::hdhr::xmltv_time;
use crate::models::{EpgProgramme, ManagedChannel};

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
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

pub fn export_visible_only(channels: &[ManagedChannel]) -> String {
    let mut sb = String::from("#EXTM3U\n");
    let mut list = channels.to_vec();
    list.sort_by(|a, b| {
        a.sort_order
            .cmp(&b.sort_order)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    for ch in list {
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
