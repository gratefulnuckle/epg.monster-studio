// SPDX-License-Identifier: GPL-3.0-or-later

//! Streamed XMLTV parse. Rows outside the retained window are dropped at parse
//! time so the snapshot cannot grow without bound.

use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::time::{SystemTime, UNIX_EPOCH};

use quick_xml::events::Event;
use quick_xml::Reader;
use serde::{Deserialize, Serialize};

use crate::tvg::tvg_keys;

pub const WINDOW_BACK_SECS: i64 = 8 * 3600;
pub const WINDOW_FWD_SECS: i64 = 16 * 3600;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Prog {
    pub start: i64,
    pub stop: i64,
    pub title: String,
}

pub fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn is_leap(y: i32) -> bool {
    y % 4 == 0 && (y % 100 != 0 || y % 400 == 0)
}

fn civil_to_unix(y: i32, mo: u32, d: u32, h: u32, mi: u32, s: u32) -> Option<i64> {
    if !(1..=12).contains(&mo) || d == 0 || d > 31 || h > 23 || mi > 59 || s > 60 {
        return None;
    }
    let mut days: i64 = 0;
    if y >= 1970 {
        for yy in 1970..y {
            days += if is_leap(yy) { 366 } else { 365 };
        }
    } else {
        for yy in y..1970 {
            days -= if is_leap(yy) { 366 } else { 365 };
        }
    }
    const MD: [u32; 12] = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    for m in 1..mo {
        days += MD[(m - 1) as usize] as i64;
        if m == 2 && is_leap(y) {
            days += 1;
        }
    }
    days += i64::from(d) - 1;
    Some(days * 86400 + i64::from(h) * 3600 + i64::from(mi) * 60 + i64::from(s))
}

pub fn xmltv_to_unix(raw: &str) -> Option<i64> {
    let raw = raw.trim();
    if raw.len() < 14 {
        return None;
    }
    let ts = &raw[..14];
    let y: i32 = ts[0..4].parse().ok()?;
    let mo: u32 = ts[4..6].parse().ok()?;
    let d: u32 = ts[6..8].parse().ok()?;
    let h: u32 = ts[8..10].parse().ok()?;
    let mi: u32 = ts[10..12].parse().ok()?;
    let s: u32 = ts[12..14].parse().ok()?;
    let mut unix = civil_to_unix(y, mo, d, h, mi, s)?;
    let rest = raw[14..].trim();
    if rest.len() >= 5 && (rest.starts_with('+') || rest.starts_with('-')) {
        let sign = if rest.starts_with('+') { 1i64 } else { -1 };
        let hh: i64 = rest[1..3].parse().ok()?;
        let mm: i64 = rest[3..5].parse().ok()?;
        unix -= sign * (hh * 3600 + mm * 60);
    }
    Some(unix)
}

pub fn hhmm(unix: i64) -> String {
    let secs = ((unix % 86400) + 86400) % 86400;
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    format!("{h:02}:{m:02}")
}

/// Parse programmes whose window intersects `[lo, hi)`. `tick` is called every
/// 200 kept rows so the UI can paint a partial snapshot.
pub fn parse_programmes<R: Read>(
    src: R,
    wanted: &HashSet<String>,
    lo: i64,
    hi: i64,
    mut tick: impl FnMut(&HashMap<String, Vec<Prog>>),
) -> Result<HashMap<String, Vec<Prog>>, String> {
    let mut reader = Reader::from_reader(std::io::BufReader::new(src));
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();
    let mut out: HashMap<String, Vec<Prog>> = HashMap::new();
    let mut kept = 0u32;
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) if e.name().as_ref() == b"programme" => {
                let mut channel = String::new();
                let mut start_raw = String::new();
                let mut stop_raw = String::new();
                for a in e.attributes().flatten() {
                    match a.key.as_ref() {
                        b"channel" => {
                            channel = a
                                .unescape_value()
                                .map(|s| s.into_owned())
                                .unwrap_or_else(|_| String::from_utf8_lossy(&a.value).into_owned());
                        }
                        b"start" => start_raw = String::from_utf8_lossy(&a.value).into_owned(),
                        b"stop" => stop_raw = String::from_utf8_lossy(&a.value).into_owned(),
                        _ => {}
                    }
                }
                let keys = tvg_keys(&channel);
                let keep = keys.iter().any(|k| wanted.contains(k));
                let mut title = String::new();
                let mut inner = Vec::new();
                loop {
                    match reader.read_event_into(&mut inner) {
                        Ok(Event::Start(ie)) if keep && ie.name().as_ref() == b"title" => {
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
                if keep {
                    if let (Some(start), Some(stop)) =
                        (xmltv_to_unix(&start_raw), xmltv_to_unix(&stop_raw))
                    {
                        if stop > lo && start < hi {
                            let prog = Prog {
                                start,
                                stop,
                                title: if title.is_empty() {
                                    "(no title)".into()
                                } else {
                                    title
                                },
                            };
                            for k in keys {
                                out.entry(k).or_default().push(prog.clone());
                            }
                            kept += 1;
                            if kept % 200 == 0 {
                                tick(&out);
                            }
                        }
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(e.to_string()),
            _ => {}
        }
        buf.clear();
    }
    for progs in out.values_mut() {
        progs.sort_by_key(|p| p.start);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fetch::wrap_maybe_gzip;
    use crate::tvg::tvg_keys;
    use std::io::Write;

    #[test]
    fn xmltv_time_utc() {
        let t = xmltv_to_unix("20240101120000 +0000").unwrap();
        assert_eq!(t, 1_704_110_400);
    }

    #[test]
    fn xmltv_time_offset() {
        let utc = xmltv_to_unix("20240101120000 +0000").unwrap();
        let plus = xmltv_to_unix("20240101120000 +0100").unwrap();
        assert_eq!(utc - plus, 3600);
    }

    #[test]
    fn parses_matching_channel_only() {
        let xml = r#"<?xml version="1.0"?>
<tv>
<programme start="20240101120000 +0000" stop="20240101130000 +0000" channel="CNN.us">
<title>The News</title>
</programme>
<programme start="20240101120000 +0000" stop="20240101130000 +0000" channel="OTHER">
<title>Skip me</title>
</programme>
</tv>"#;
        let mut wanted = HashSet::new();
        wanted.insert("cnn.us".into());
        let lo = xmltv_to_unix("20240101120000 +0000").unwrap() - 10;
        let hi = xmltv_to_unix("20240101130000 +0000").unwrap() + 10;
        let map = parse_programmes(xml.as_bytes(), &wanted, lo, hi, |_| {}).unwrap();
        assert!(map.contains_key("cnn.us"));
        assert!(!map.contains_key("other"));
        assert_eq!(map["cnn.us"][0].title, "The News");
    }

    #[test]
    fn parses_alias_and_entities() {
        let xml = r#"<?xml version="1.0"?>
<tv>
<programme start="20240101120000 +0000" stop="20240101130000 +0000" channel="KAUT-DT.us">
<title>News &amp; Weather</title>
</programme>
</tv>"#;
        let wanted: HashSet<String> = tvg_keys("KAUT-DT.us_locals1.us (src05)")
            .into_iter()
            .collect();
        let lo = xmltv_to_unix("20240101120000 +0000").unwrap() - 10;
        let hi = xmltv_to_unix("20240101130000 +0000").unwrap() + 10;
        let map = parse_programmes(xml.as_bytes(), &wanted, lo, hi, |_| {}).unwrap();
        assert!(map.contains_key("kaut-dt.us"));
        assert_eq!(map["kaut-dt.us"][0].title, "News & Weather");
    }

    #[test]
    fn drops_programmes_outside_window() {
        let xml = r#"<?xml version="1.0"?>
<tv>
<programme start="20240101000000 +0000" stop="20240101010000 +0000" channel="CNN.us">
<title>Too early</title>
</programme>
<programme start="20240101120000 +0000" stop="20240101130000 +0000" channel="CNN.us">
<title>In window</title>
</programme>
<programme start="20240102120000 +0000" stop="20240102130000 +0000" channel="CNN.us">
<title>Too late</title>
</programme>
</tv>"#;
        let mut wanted = HashSet::new();
        wanted.insert("cnn.us".into());
        let now = xmltv_to_unix("20240101120000 +0000").unwrap();
        let lo = now - WINDOW_BACK_SECS;
        let hi = now + WINDOW_FWD_SECS;
        let map = parse_programmes(xml.as_bytes(), &wanted, lo, hi, |_| {}).unwrap();
        let titles: Vec<_> = map["cnn.us"].iter().map(|p| p.title.as_str()).collect();
        assert_eq!(titles, vec!["In window"]);
    }

    #[test]
    fn parse_gzip_xml() {
        use flate2::write::GzEncoder;
        use flate2::Compression;
        let xml = br#"<?xml version="1.0"?><tv>
<programme start="20240101120000 +0000" stop="20240101130000 +0000" channel="CNN.us">
<title>Z</title>
</programme></tv>"#;
        let mut enc = GzEncoder::new(Vec::new(), Compression::default());
        enc.write_all(xml).unwrap();
        let gz = enc.finish().unwrap();
        let mut wanted = HashSet::new();
        wanted.insert("cnn.us".into());
        let lo = xmltv_to_unix("20240101120000 +0000").unwrap() - 10;
        let hi = xmltv_to_unix("20240101130000 +0000").unwrap() + 10;
        let reader = wrap_maybe_gzip(std::io::Cursor::new(gz)).unwrap();
        let map = parse_programmes(reader, &wanted, lo, hi, |_| {}).unwrap();
        assert_eq!(map["cnn.us"][0].title, "Z");
    }

    #[test]
    fn hhmm_is_24_hour() {
        assert_eq!(hhmm(1_704_110_400), "12:00");
        assert_eq!(hhmm(1_704_110_400 + 13 * 3600 + 5 * 60), "01:05");
    }
}
