// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::BTreeMap;

use regex::Regex;

use crate::models::ChannelEntry;

/// Port of C# `M3uParser.Parse`.
pub fn parse_m3u(content: &str, source_id: &str) -> Vec<ChannelEntry> {
    let mut list = Vec::new();
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
            list.push(parse_extinf(&extinf, trimmed, source_id, entry_line));
        } else {
            list.push(ChannelEntry {
                source_id: source_id.to_string(),
                name: trimmed.to_string(),
                url: trimmed.to_string(),
                group_title: "Ungrouped".into(),
                line_no,
                ..ChannelEntry::default()
            });
        }
    }
    list
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

    let re = Regex::new(r#"(?P<key>[\w-]+)\s*=\s*"(?P<value>[^"]*)""#).expect("attr regex");
    let mut attrs = BTreeMap::<String, String>::new();
    for cap in re.captures_iter(&attr_region) {
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
        name_raw.to_string()
    };

    ChannelEntry {
        source_id: source_id.to_string(),
        name,
        group_title: group.unwrap_or_else(|| "Ungrouped".into()),
        tvg_id,
        tvg_name,
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
    fn bare_url_without_extinf() {
        let m3u = "#EXTM3U\nhttp://example.com/raw\n";
        let channels = parse_m3u(m3u, "");
        assert_eq!(channels.len(), 1);
        assert_eq!(channels[0].url, "http://example.com/raw");
        assert_eq!(channels[0].group_title, "Ungrouped");
    }
}
