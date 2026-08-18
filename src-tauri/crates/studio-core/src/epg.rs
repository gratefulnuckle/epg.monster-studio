// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::{HashMap, HashSet};
use std::io::{Cursor, Read};
use std::path::Path;

use flate2::read::GzDecoder;
use quick_xml::events::Event;
use quick_xml::Reader;
use time::format_description::well_known::Rfc3339;
use time::{Duration, OffsetDateTime, PrimitiveDateTime};

use crate::models::{CatalogEntry, EpgAuditRow, ManagedChannel};
use crate::settings::AppSettings;
use crate::USER_AGENT;

pub const DEFAULT_XML_URL: &str = "https://epg.monster/epg.xml";

pub fn is_epgshare_url(url: &str) -> bool {
    let u = url.to_ascii_lowercase();
    u.contains("epgshare01") || u.contains("epgshare.online")
}

pub fn resolve_xml_urls(settings: &AppSettings) -> Vec<String> {
    let raw: Vec<String> = if let Some(list) = &settings.epg_xml_urls {
        if !list.is_empty() {
            list.clone()
        } else if !settings.epg_xml_url.trim().is_empty() {
            vec![settings.epg_xml_url.clone()]
        } else {
            vec![]
        }
    } else if !settings.epg_xml_url.trim().is_empty() {
        vec![settings.epg_xml_url.clone()]
    } else {
        vec![]
    };
    let mut out: Vec<String> = raw
        .into_iter()
        .map(|u| u.trim().to_string())
        .filter(|u| !u.is_empty() && !is_epgshare_url(u))
        .collect();
    out.sort();
    out.dedup();
    if out.is_empty() {
        out.push(DEFAULT_XML_URL.into());
    }
    out
}

pub fn clean_epg_token(value: &str) -> String {
    let mut s = value.trim().to_string();
    for _ in 0..3 {
        let decoded = html_decode(&s);
        if decoded == s {
            break;
        }
        s = decoded;
    }
    s = s
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&nbsp;", " ");
    let collapsed: String = s.split_whitespace().collect::<Vec<_>>().join(" ");
    collapsed.trim().to_string()
}

fn html_decode(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ")
}

pub fn looks_like_gzip(bytes: &[u8]) -> bool {
    bytes.len() >= 2 && bytes[0] == 0x1f && bytes[1] == 0x8b
}

pub fn materialize_xmltv(bytes: &[u8], dest: &Path) -> std::io::Result<()> {
    if let Some(dir) = dest.parent() {
        std::fs::create_dir_all(dir)?;
    }
    if looks_like_gzip(bytes) {
        let mut dec = GzDecoder::new(Cursor::new(bytes));
        let mut out = Vec::new();
        dec.read_to_end(&mut out)?;
        std::fs::write(dest, out)
    } else {
        std::fs::write(dest, bytes)
    }
}

pub fn parse_xmltv_channels(xml: &str, source_label: &str) -> Vec<CatalogEntry> {
    let source = if source_label.trim().is_empty() {
        "xmltv"
    } else {
        source_label.trim()
    };
    let mut list = Vec::new();
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) if e.name().as_ref() == b"channel" => {
                let id = e
                    .attributes()
                    .flatten()
                    .find(|a| a.key.as_ref() == b"id")
                    .and_then(|a| String::from_utf8(a.value.to_vec()).ok())
                    .unwrap_or_default();
                let id = clean_epg_token(&id);
                let mut name = String::new();
                let mut logo = None;
                let mut inner = Vec::new();
                loop {
                    match reader.read_event_into(&mut inner) {
                        Ok(Event::Start(ie)) if ie.name().as_ref() == b"display-name" && name.is_empty() => {
                            if let Ok(Event::Text(t)) = reader.read_event_into(&mut inner) {
                                name = clean_epg_token(&t.unescape().unwrap_or_default());
                            }
                        }
                        Ok(Event::Empty(ie)) | Ok(Event::Start(ie)) if ie.name().as_ref() == b"icon" => {
                            if logo.is_none() {
                                logo = ie
                                    .attributes()
                                    .flatten()
                                    .find(|a| a.key.as_ref() == b"src")
                                    .and_then(|a| String::from_utf8(a.value.to_vec()).ok())
                                    .map(|s| s.trim().to_string())
                                    .filter(|s| !s.is_empty());
                            }
                        }
                        Ok(Event::End(ie)) if ie.name().as_ref() == b"channel" => break,
                        Ok(Event::Eof) => break,
                        Err(_) => break,
                        _ => {}
                    }
                    inner.clear();
                }
                if !id.is_empty() {
                    list.push(CatalogEntry {
                        tvg_id: id.clone(),
                        name: if name.is_empty() { id } else { name },
                        logo,
                        section: source.to_string(),
                    });
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    list
}

pub fn index_programmes_from_xml(xml: &str) -> Vec<(String, String, String, String)> {
    parse_xmltv_programmes(xml, OffsetDateTime::now_utc())
}

pub fn parse_xmltv_programmes(xml: &str, now: OffsetDateTime) -> Vec<(String, String, String, String)> {
    let window_start = now - Duration::hours(8);
    let window_end = now + Duration::hours(16);
    let mut list = Vec::new();
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) if e.name().as_ref() == b"programme" => {
                let mut channel = String::new();
                let mut start_raw = String::new();
                let mut stop_raw = String::new();
                for a in e.attributes().flatten() {
                    match a.key.as_ref() {
                        b"channel" => channel = String::from_utf8_lossy(&a.value).into_owned(),
                        b"start" => start_raw = String::from_utf8_lossy(&a.value).into_owned(),
                        b"stop" => stop_raw = String::from_utf8_lossy(&a.value).into_owned(),
                        _ => {}
                    }
                }
                channel = clean_epg_token(&channel);
                let mut title = "(no title)".to_string();
                let mut inner = Vec::new();
                loop {
                    match reader.read_event_into(&mut inner) {
                        Ok(Event::Start(ie)) if ie.name().as_ref() == b"title" => {
                            if let Ok(Event::Text(t)) = reader.read_event_into(&mut inner) {
                                title = t.unescape().unwrap_or_default().trim().to_string();
                            }
                        }
                        Ok(Event::End(ie)) if ie.name().as_ref() == b"programme" => break,
                        Ok(Event::Eof) => break,
                        Err(_) => break,
                        _ => {}
                    }
                    inner.clear();
                }
                if channel.is_empty() {
                    continue;
                }
                let Some(start) = try_parse_xmltv_time(&start_raw) else { continue };
                let Some(stop) = try_parse_xmltv_time(&stop_raw) else { continue };
                if stop <= window_start || start >= window_end {
                    continue;
                }
                list.push((
                    channel,
                    title,
                    start.format(&Rfc3339).unwrap_or_default(),
                    stop.format(&Rfc3339).unwrap_or_default(),
                ));
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    list
}

pub fn try_parse_xmltv_time(raw: &str) -> Option<OffsetDateTime> {
    let raw = raw.trim();
    if raw.len() < 14 {
        return None;
    }
    let mut parts = raw.split(|c: char| c.is_whitespace());
    let ts = parts.next()?;
    if ts.len() < 14 {
        return None;
    }
    let fmt = time::format_description::parse_borrowed::<2>("[year][month][day][hour][minute][second]").ok()?;
    let local = PrimitiveDateTime::parse(&ts[..14], &fmt).ok()?;
    let mut offset = time::UtcOffset::UTC;
    if let Some(off) = parts.next() {
        let off = off.replace(':', "");
        if off.len() >= 5 && (off.starts_with('+') || off.starts_with('-')) {
            let sign = if off.starts_with('-') { -1 } else { 1 };
            let hh: i8 = off[1..3].parse().ok()?;
            let mm: i8 = off[3..5].parse().ok()?;
            offset = time::UtcOffset::from_hms(sign * hh, sign * mm, 0).ok()?;
        }
    }
    Some(local.assume_offset(offset).to_offset(time::UtcOffset::UTC))
}

pub fn normalize(s: &str) -> String {
    let s = s.to_lowercase();
    let mut out = String::new();
    let mut space = false;
    for c in s.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c);
            space = false;
        } else if !space {
            out.push(' ');
            space = true;
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub fn similarity(a: &str, b: &str) -> f64 {
    if a == b {
        return 1.0;
    }
    if a.contains(b) || b.contains(a) {
        return 0.85;
    }
    let ab = bigrams(a);
    let bb = bigrams(b);
    if ab.is_empty() || bb.is_empty() {
        return 0.0;
    }
    let inter = ab.intersection(&bb).count();
    2.0 * inter as f64 / (ab.len() + bb.len()) as f64
}

fn bigrams(s: &str) -> HashSet<String> {
    let mut set = HashSet::new();
    let chars: Vec<char> = s.chars().collect();
    if chars.len() < 2 {
        if !s.is_empty() {
            set.insert(s.to_string());
        }
        return set;
    }
    for w in chars.windows(2) {
        set.insert(w.iter().collect());
    }
    set
}

pub fn google_images_transparent_url(query: &str) -> String {
    let q = if query.trim().is_empty() {
        "logo"
    } else {
        query.trim()
    };
    format!(
        "https://www.google.com/search?tbm=isch&tbs=ic:trans&q={}",
        urlencoding_minimal(q)
    )
}

fn urlencoding_minimal(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            b' ' => out.push_str("%20"),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

pub fn build_epg_audit(channels: &[ManagedChannel], catalog: &[CatalogEntry]) -> Vec<EpgAuditRow> {
    let mut by_id: HashMap<String, &CatalogEntry> = HashMap::new();
    for c in catalog {
        if !c.tvg_id.trim().is_empty() {
            by_id.entry(c.tvg_id.trim().to_ascii_lowercase()).or_insert(c);
        }
    }
    let mut by_exact_norm: HashMap<String, &CatalogEntry> = HashMap::new();
    let mut by_word: HashMap<String, Vec<(&CatalogEntry, String)>> = HashMap::new();
    for c in catalog {
        let norm = normalize(&c.name);
        if norm.is_empty() {
            continue;
        }
        by_exact_norm.entry(norm.clone()).or_insert(c);
        for word in norm.split_whitespace() {
            if word.len() < 3 {
                continue;
            }
            let bucket = by_word.entry(word.to_string()).or_default();
            if bucket.len() < 40 {
                bucket.push((c, norm.clone()));
            }
        }
    }

    let mut rows: Vec<EpgAuditRow> = channels
        .iter()
        .map(|ch| {
            let mut row = EpgAuditRow {
                managed_channel_id: ch.id.clone(),
                channel_name: ch.name.clone(),
                group_title: if ch.group_title.is_empty() {
                    "Ungrouped".into()
                } else {
                    ch.group_title.clone()
                },
                current_tvg_id: ch.tvg_id.clone(),
                status: "missing".into(),
                suggested_tvg_id: None,
                suggested_name: None,
                suggested_logo: None,
                score: 0.0,
                second_score: 0.0,
                match_kind: None,
            };
            let id = ch.tvg_id.as_deref().map(str::trim).unwrap_or("");
            if !id.is_empty() {
                if let Some(exact) = by_id.get(&id.to_ascii_lowercase()) {
                    row.status = "matched".into();
                    row.suggested_tvg_id = Some(exact.tvg_id.clone());
                    row.suggested_name = Some(exact.name.clone());
                    row.suggested_logo = exact.logo.clone();
                    row.score = 1.0;
                    row.match_kind = Some("exact".into());
                    return row;
                }
                row.status = "unknown".into();
            }
            apply_best_fuzzy(&mut row, &ch.name, &by_exact_norm, &by_word);
            row
        })
        .collect();
    rows.sort_by(|a, b| {
        a.group_title
            .to_lowercase()
            .cmp(&b.group_title.to_lowercase())
            .then_with(|| a.channel_name.to_lowercase().cmp(&b.channel_name.to_lowercase()))
    });
    rows
}

fn apply_best_fuzzy(
    row: &mut EpgAuditRow,
    name: &str,
    by_exact_norm: &HashMap<String, &CatalogEntry>,
    by_word: &HashMap<String, Vec<(&CatalogEntry, String)>>,
) {
    let norm = normalize(name);
    if norm.is_empty() {
        return;
    }
    if let Some(exact) = by_exact_norm.get(&norm) {
        row.suggested_tvg_id = Some(exact.tvg_id.clone());
        row.suggested_name = Some(exact.name.clone());
        row.suggested_logo = exact.logo.clone();
        row.score = 0.98;
        row.second_score = 0.0;
        row.match_kind = Some("fuzzy".into());
        return;
    }
    let mut candidates: HashMap<String, (&CatalogEntry, String)> = HashMap::new();
    for word in norm.split_whitespace() {
        if word.len() < 3 {
            continue;
        }
        if let Some(bucket) = by_word.get(word) {
            for item in bucket {
                if !item.0.tvg_id.is_empty() {
                    candidates.entry(item.0.tvg_id.clone()).or_insert((item.0, item.1.clone()));
                }
            }
        }
        if candidates.len() >= 120 {
            break;
        }
    }
    if candidates.is_empty() {
        return;
    }
    let mut best: Option<&CatalogEntry> = None;
    let mut best_score = 0.0;
    let mut second = 0.0;
    for (_, (c, n)) in candidates {
        let score = similarity(&norm, &n);
        if score > best_score {
            second = best_score;
            best_score = score;
            best = Some(c);
        } else if score > second {
            second = score;
        }
    }
    if best_score < 0.55 {
        return;
    }
    if let Some(best) = best {
        row.suggested_tvg_id = Some(best.tvg_id.clone());
        row.suggested_name = Some(best.name.clone());
        row.suggested_logo = best.logo.clone();
        row.score = best_score;
        row.second_score = second;
        row.match_kind = Some("fuzzy".into());
    }
}

pub fn should_auto_apply(row: &EpgAuditRow, min_score: f64, require_unique: bool) -> bool {
    if row.status == "matched" || !row.has_suggestion() {
        return false;
    }
    if row.score + 0.0001 < min_score {
        return false;
    }
    if require_unique && !row.is_unique_suggestion() {
        return false;
    }
    let id = row.suggested_tvg_id.as_deref().unwrap_or("");
    if id.to_ascii_lowercase().contains("dummy") {
        return false;
    }
    true
}

pub fn fetch_xmltv(url: &str) -> Result<Vec<u8>, String> {
    let resp = ureq::get(url)
        .set("User-Agent", USER_AGENT)
        .call()
        .map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    resp.into_reader()
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    Ok(bytes)
}

pub fn section_display(section: &str) -> String {
    if section.trim().is_empty() {
        return "Other".into();
    }
    let s = section.trim();
    let stripped: String = s
        .chars()
        .rev()
        .skip_while(|c| c.is_ascii_digit())
        .collect::<String>()
        .chars()
        .rev()
        .collect();
    if stripped.is_empty() {
        s.to_string()
    } else {
        stripped
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"<?xml version="1.0"?>
<tv>
  <channel id="CNN.us"><display-name>CNN</display-name><icon src="http://logo/cnn.png"/></channel>
  <channel id="Dummy.us"><display-name>Dummy</display-name></channel>
  <channel id="BBC.One.uk"><display-name>BBC One</display-name></channel>
  <programme start="20260101120000 +0000" stop="20260101130000 +0000" channel="CNN.us">
    <title>News Hour</title>
  </programme>
</tv>"#;

    #[test]
    fn strips_epgshare_and_defaults_monster() {
        let mut s = AppSettings::default();
        s.epg_xml_url = "https://epgshare01.online/x.xml".into();
        let urls = resolve_xml_urls(&s);
        assert_eq!(urls, vec![DEFAULT_XML_URL]);
    }

    #[test]
    fn parses_channels() {
        let ch = parse_xmltv_channels(SAMPLE, "epg.monster");
        assert_eq!(ch.len(), 3);
        assert_eq!(ch[0].tvg_id, "CNN.us");
        assert_eq!(ch[0].name, "CNN");
        assert_eq!(ch[0].logo.as_deref(), Some("http://logo/cnn.png"));
    }

    #[test]
    fn cleans_html_entities() {
        assert_eq!(clean_epg_token("Atfal.&amp;.Mawaheb.ae"), "Atfal.&.Mawaheb.ae");
    }

    #[test]
    fn exact_match_not_overwritten() {
        let catalog = parse_xmltv_channels(SAMPLE, "epg.monster");
        let ch = ManagedChannel {
            id: "1".into(),
            name: "Something Else".into(),
            group_title: "NEWS".into(),
            tvg_id: Some("CNN.us".into()),
            tvg_logo: None,
            notes: None,
            sort_order: 0,
            tvg_shift_hours: 0.0,
            in_tuner: false,
            tuner_number: None,
            variants: vec![],
            has_epg_match: false,
        };
        let rows = build_epg_audit(&[ch], &catalog);
        assert_eq!(rows[0].status, "matched");
        assert_eq!(rows[0].suggested_tvg_id.as_deref(), Some("CNN.us"));
        assert_eq!(rows[0].match_kind.as_deref(), Some("exact"));
    }

    #[test]
    fn dummy_id_is_not_auto_applied() {
        let row = EpgAuditRow {
            managed_channel_id: "1".into(),
            channel_name: "X".into(),
            group_title: "NEWS".into(),
            current_tvg_id: None,
            status: "missing".into(),
            suggested_tvg_id: Some("Dummy.us".into()),
            suggested_name: Some("Dummy".into()),
            suggested_logo: None,
            score: 0.99,
            second_score: 0.0,
            match_kind: Some("fuzzy".into()),
        };
        assert!(!should_auto_apply(&row, 0.85, true));
    }

    #[test]
    fn parses_xmltv_time() {
        let t = try_parse_xmltv_time("20240101120000 +0000").unwrap();
        assert_eq!(t.year(), 2024);
        assert_eq!(t.hour(), 12);
    }

    #[test]
    fn gzip_magic() {
        assert!(looks_like_gzip(&[0x1f, 0x8b, 0x00]));
        assert!(!looks_like_gzip(b"<tv>"));
    }
}
