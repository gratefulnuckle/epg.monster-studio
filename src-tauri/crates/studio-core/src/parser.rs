// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::BTreeMap;
use std::sync::OnceLock;

use regex::Regex;

use crate::models::ChannelEntry;

fn attr_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r#"(?P<key>[\w-]+)\s*=\s*"(?P<value>[^"]*)""#).expect("attr regex"))
}

/// Parse an M3U/M3U8 playlist into channel rows.
pub fn parse_m3u(content: &str, source_id: &str) -> Vec<ChannelEntry> {
    let mut list = Vec::new();
    for_each_m3u_channel(content, source_id, |ch| list.push(ch));
    list
}

/// Stream entries so large playlists can be inserted in batches.
pub fn for_each_m3u_channel(content: &str, source_id: &str, mut visit: impl FnMut(ChannelEntry)) {
    let mut pending: Option<(String, i32)> = None;
    let mut line_no = 0i32;

    for raw in content.lines() {
        line_no += 1;
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.len() >= 7 && trimmed[..7].eq_ignore_ascii_case("#EXTINF") {
            pending = Some((trimmed.to_string(), line_no));
            continue;
        }
        if trimmed.starts_with('#') {
            continue;
        }
        if let Some((extinf, entry_line)) = pending.take() {
            visit(parse_extinf(&extinf, trimmed, source_id, entry_line));
        } else {
            visit(ChannelEntry {
                source_id: source_id.to_string(),
                name: trimmed.to_string(),
                url: trimmed.to_string(),
                group_title: "Ungrouped".into(),
                line_no,
                ..ChannelEntry::default()
            });
        }
    }
}

fn parse_extinf(extinf_line: &str, url: &str, source_id: &str, line_no: i32) -> ChannelEntry {
    let comma = extinf_line.rfind(',');
    let (attr_region_raw, name_raw) = match comma {
        Some(i) => (&extinf_line[..i], extinf_line[i + 1..].trim()),
        None => (extinf_line, "Unknown"),
    };

    let mut attr_region = if let Some(colon) = attr_region_raw.find(':') {
        attr_region_raw[colon + 1..].trim().to_string()
    } else {
        attr_region_raw.trim().to_string()
    };

    if let Some(space) = attr_region.find(' ') {
        if attr_region[..space].parse::<f64>().is_ok() {
            attr_region = attr_region[space..].trim().to_string();
        }
    } else if attr_region.parse::<f64>().is_ok() {
        attr_region.clear();
    }

    let mut attrs = BTreeMap::<String, String>::new();
    for cap in attr_re().captures_iter(&attr_region) {
        attrs.insert(
            cap.name("key").unwrap().as_str().to_string(),
            cap.name("value").unwrap().as_str().to_string(),
        );
    }

    let get = |keys: &[&str]| {
        attrs
            .iter()
            .find(|(k, _)| keys.iter().any(|want| k.eq_ignore_ascii_case(want)))
            .map(|(_, v)| v.clone())
    };

    let group = get(&["group-title"]).filter(|s| !s.trim().is_empty());
    let tvg_id = get(&["tvg-id"]).filter(|s| !s.trim().is_empty());
    let tvg_name = get(&["tvg-name"]).filter(|s| !s.trim().is_empty());
    let tvg_logo = get(&["tvg-logo"]).filter(|s| !s.trim().is_empty());
    let shift_raw = get(&["tvg-shift", "timeshift", "tvg_shift"]);
    let tvg_shift_hours = shift_raw
        .as_deref()
        .map(|s| s.trim().parse::<f64>().unwrap_or(0.0))
        .unwrap_or(0.0);

    let skip = [
        "group-title",
        "tvg-id",
        "tvg-name",
        "tvg-logo",
        "tvg-shift",
        "timeshift",
        "tvg_shift",
    ];
    let extra: BTreeMap<_, _> = attrs
        .into_iter()
        .filter(|(k, _)| {
            !skip
                .iter()
                .any(|s| s.eq_ignore_ascii_case(k))
        })
        .collect();

    let name = if name_raw.is_empty() {
        url.to_string()
    } else {
        crate::epg::clean_epg_token(name_raw)
    };

    ChannelEntry {
        source_id: source_id.to_string(),
        name,
        group_title: group
            .map(|g| crate::epg::clean_epg_token(&g))
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "Ungrouped".into()),
        tvg_id: tvg_id.map(|s| crate::epg::clean_epg_token(&s)).filter(|s| !s.is_empty()),
        tvg_name: tvg_name.map(|s| crate::epg::clean_epg_token(&s)).filter(|s| !s.is_empty()),
        tvg_logo,
        tvg_shift_hours,
        url: url.trim().to_string(),
        attrs_json: serde_json::to_string(&extra).unwrap_or_else(|_| "{}".into()),
        line_no,
        ..ChannelEntry::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_extinf_with_attrs() {
        let m3u = r#"#EXTM3U
#EXTINF:-1 tvg-id="ESPN.us" tvg-logo="http://logo/espn.png" group-title="Sports",ESPN USA
http://example.com/espn.ts
#EXTINF:-1 group-title="News",CNN
http://example.com/cnn.m3u8
"#;
        let channels = parse_m3u(m3u, "src1");
        assert_eq!(channels.len(), 2);
        assert_eq!(channels[0].name, "ESPN USA");
        assert_eq!(channels[0].group_title, "Sports");
        assert_eq!(channels[0].tvg_id.as_deref(), Some("ESPN.us"));
        assert_eq!(channels[0].tvg_logo.as_deref(), Some("http://logo/espn.png"));
        assert_eq!(channels[0].url, "http://example.com/espn.ts");
        assert_eq!(channels[0].source_id, "src1");
        assert_eq!(channels[1].name, "CNN");
        assert_eq!(channels[1].group_title, "News");
    }

    #[test]
    fn missing_group_becomes_ungrouped() {
        let m3u = r#"#EXTM3U
#EXTINF:-1 tvg-id="x",Solo Channel
http://example.com/solo
"#;
        let channels = parse_m3u(m3u, "");
        assert_eq!(channels.len(), 1);
        assert_eq!(channels[0].group_title, "Ungrouped");
        assert_eq!(channels[0].tvg_id.as_deref(), Some("x"));
    }

    #[test]
    fn html_entities_in_name_and_group() {
        let m3u = r#"#EXTM3U
#EXTINF:-1 tvg-id="A&amp;E.us" group-title="US Locals &amp; Regional",Crime &amp; Investigation
http://example.com/ae
"#;
        let channels = parse_m3u(m3u, "src");
        assert_eq!(channels.len(), 1);
        assert_eq!(channels[0].name, "Crime & Investigation");
        assert_eq!(channels[0].group_title, "US Locals & Regional");
        assert_eq!(channels[0].tvg_id.as_deref(), Some("A&E.us"));
    }

    #[test]
    fn bare_url_without_extinf() {
        let m3u = "#EXTM3U\nhttp://example.com/raw\n";
        let channels = parse_m3u(m3u, "");
        assert_eq!(channels.len(), 1);
        assert_eq!(channels[0].url, "http://example.com/raw");
        assert_eq!(channels[0].group_title, "Ungrouped");
    }

    #[test]
    fn parse_thousands_of_extinf_rows() {
        let mut m3u = String::from("#EXTM3U\n");
        for i in 0..4_000 {
            m3u.push_str(&format!(
                "#EXTINF:-1 tvg-id=\"id{i}\" group-title=\"G{g}\",Ch {i}\nhttp://example.com/{i}\n",
                g = i % 40
            ));
        }
        let started = std::time::Instant::now();
        let channels = parse_m3u(&m3u, "src-big");
        assert_eq!(channels.len(), 4_000);
        assert_eq!(channels[0].tvg_id.as_deref(), Some("id0"));
        assert_eq!(channels[3999].name, "Ch 3999");
        assert!(
            started.elapsed() < std::time::Duration::from_secs(2),
            "parse_m3u too slow: {:?}",
            started.elapsed()
        );
    }
}
