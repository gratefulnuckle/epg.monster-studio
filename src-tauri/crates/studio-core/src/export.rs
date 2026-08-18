// SPDX-License-Identifier: GPL-3.0-or-later

use crate::models::ManagedChannel;

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
}
