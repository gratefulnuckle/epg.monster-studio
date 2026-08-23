// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::{HashMap, HashSet};
use std::io::{BufRead, BufReader, Cursor, Read};
use std::path::Path;

use flate2::read::GzDecoder;
use quick_xml::events::Event;
use quick_xml::Reader;
use time::format_description::well_known::Rfc3339;
use time::{Duration, OffsetDateTime, PrimitiveDateTime};

use crate::models::{CatalogEntry, EpgAuditRow, ManagedChannel};
use crate::settings::AppSettings;
use crate::USER_AGENT;

pub const DEFAULT_XML_URL: &str = "https://epg.monster/epg.xml.gz";
pub const REFRESH_INTERVAL_SECS: i64 = 30 * 60;
/// HTTP body cap (gzip or raw). epg.monster's gzip is typically well under this;
/// uncompressed `epg.xml` is not — we rewrite that URL to `.gz`.
pub const XMLTV_DOWNLOAD_MAX: u64 = 512 * 1024 * 1024;
/// On-disk uncompressed XML cap. Streamed to disk; not loaded as one String.
pub const XMLTV_FILE_MAX: u64 = 2 * 1024 * 1024 * 1024;
/// Legacy alias used by purge / size checks on cache files.
pub const XMLTV_MAX_BYTES: u64 = XMLTV_FILE_MAX;

pub fn xmltv_too_large(len: u64) -> bool {
    len > XMLTV_FILE_MAX
}

/// Prefer the gzip catalog. Uncompressed epg.monster/epg.xml is hundreds of MB
/// and trips download caps; `.gz` is the supported v2 URL.
pub fn prefer_compact_xmltv_url(url: &str) -> String {
    let t = url.trim();
    let lower = t.to_ascii_lowercase();
    if (lower == "https://epg.monster/epg.xml" || lower == "http://epg.monster/epg.xml")
        && !lower.ends_with(".gz")
    {
        return "https://epg.monster/epg.xml.gz".into();
    }
    t.to_string()
}

pub fn read_capped<R: Read>(src: R, max: u64) -> Result<Vec<u8>, String> {
    let mut buf = Vec::new();
    let n = src
        .take(max.saturating_add(1))
        .read_to_end(&mut buf)
        .map_err(|e| e.to_string())?;
    if n as u64 > max {
        return Err(format!(
            "XMLTV larger than {} MB — refused.",
            max / (1024 * 1024)
        ));
    }
    Ok(buf)
}

fn copy_capped<R: Read>(src: R, dest: &Path, max: u64) -> Result<u64, String> {
    if let Some(dir) = dest.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let mut out = std::fs::File::create(dest).map_err(|e| e.to_string())?;
    let n = std::io::copy(&mut src.take(max.saturating_add(1)), &mut out)
        .map_err(|e| e.to_string())?;
    if n > max {
        let _ = std::fs::remove_file(dest);
        return Err(format!(
            "XMLTV larger than {} MB — refused.",
            max / (1024 * 1024)
        ));
    }
    Ok(n)
}

/// Delete XMLTV / gzip cache files above [`XMLTV_MAX_BYTES`] so rebuild cannot reopen them.
pub fn purge_oversized_xmltv(dir: &Path) -> usize {
    purge_oversized_xmltv_over(dir, XMLTV_MAX_BYTES)
}

fn purge_oversized_xmltv_over(dir: &Path, max: u64) -> usize {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return 0;
    };
    let mut n = 0;
    for e in rd.flatten() {
        let p = e.path();
        let ext = p
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if ext != "xml" && ext != "gz" {
            continue;
        }
        let Ok(meta) = e.metadata() else {
            continue;
        };
        if meta.len() == 0 || meta.len() <= max {
            continue;
        }
        if std::fs::remove_file(&p).is_ok() {
            n += 1;
        }
    }
    n
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub struct EpgCacheMeta {
    pub last_fetch_at: Option<String>,
    pub last_index_at: Option<String>,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
}

impl EpgCacheMeta {
    pub fn index_is_fresh(&self, max_age_secs: i64) -> bool {
        let Some(raw) = self.last_index_at.as_deref() else {
            return false;
        };
        let Ok(t) = OffsetDateTime::parse(raw, &Rfc3339) else {
            return false;
        };
        let age = OffsetDateTime::now_utc() - t;
        age < Duration::seconds(max_age_secs)
    }
}

pub enum FetchXmltv {
    NotModified,
    Body {
        bytes: Vec<u8>,
        etag: Option<String>,
        last_modified: Option<String>,
    },
}

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
        .map(|u| prefer_compact_xmltv_url(&u))
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

/// IDs to try when matching a playlist tvg-id to catalog/XMLTV (`KAUT-DT.us_locals1.us (src05)` → `KAUT-DT.us`).
pub fn tvg_lookup_ids(tvg_id: &str) -> Vec<String> {
    let raw = tvg_id.trim().to_string();
    let cleaned = clean_epg_token(&raw);
    let mut out = Vec::new();
    let mut push = |s: String| {
        let s = s.trim().to_string();
        if !s.is_empty() && !out.iter().any(|x: &String| x.eq_ignore_ascii_case(&s)) {
            out.push(s);
        }
    };
    push(raw);
    push(cleaned.clone());
    push(cleaned.replace(' ', "."));
    push(cleaned.replace('.', " "));
    let no_paren = cleaned
        .split(" (")
        .next()
        .unwrap_or(&cleaned)
        .trim()
        .to_string();
    push(no_paren.clone());
    let lower = no_paren.to_ascii_lowercase();
    if let Some(idx) = lower.find(".us_locals") {
        let stem = &no_paren[..idx];
        push(format!("{stem}.us"));
        push(stem.to_string());
    }
    out
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
        let mut out = Vec::new();
        let n = GzDecoder::new(Cursor::new(bytes))
            .take(XMLTV_FILE_MAX.saturating_add(1))
            .read_to_end(&mut out)?;
        if n as u64 > XMLTV_FILE_MAX {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!(
                    "decompressed XMLTV larger than {} MB — refused.",
                    XMLTV_FILE_MAX / (1024 * 1024)
                ),
            ));
        }
        std::fs::write(dest, out)
    } else {
        if xmltv_too_large(bytes.len() as u64) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!(
                    "XMLTV larger than {} MB — refused.",
                    XMLTV_MAX_BYTES / (1024 * 1024)
                ),
            ));
        }
        std::fs::write(dest, bytes)
    }
}

pub fn parse_xmltv_channels(xml: &str, source_label: &str) -> Vec<CatalogEntry> {
    parse_xmltv_channels_read(xml.as_bytes(), source_label)
}

pub fn parse_xmltv_channels_from_path(
    path: &Path,
    source_label: &str,
) -> Result<Vec<CatalogEntry>, String> {
    let f = std::fs::File::open(path).map_err(|e| e.to_string())?;
    Ok(parse_xmltv_channels_read(BufReader::new(f), source_label))
}

fn parse_xmltv_channels_read<R: BufRead>(src: R, source_label: &str) -> Vec<CatalogEntry> {
    let source = if source_label.trim().is_empty() {
        "xmltv"
    } else {
        source_label.trim()
    };
    let mut list = Vec::new();
    let mut reader = Reader::from_reader(src);
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

/// Compact UTC for SQLite lexicographic `start <= now < stop` (ISO-8601 `"O"` prefix).
pub fn format_utc_z(dt: OffsetDateTime) -> String {
    let dt = dt.to_offset(time::UtcOffset::UTC);
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        dt.year(),
        u8::from(dt.month()),
        dt.day(),
        dt.hour(),
        dt.minute(),
        dt.second()
    )
}

pub fn index_programmes_from_xml(xml: &str) -> Vec<(String, String, String, String)> {
    parse_xmltv_programmes(xml, OffsetDateTime::now_utc())
}

pub fn index_programmes_from_path(path: &Path) -> Result<Vec<(String, String, String, String)>, String> {
    let meta = std::fs::metadata(path).map_err(|e| e.to_string())?;
    if xmltv_too_large(meta.len()) {
        return Err(format!(
            "XMLTV {} is {} MB (max {} MB) — skipped.",
            path.display(),
            meta.len() / (1024 * 1024),
            XMLTV_MAX_BYTES / (1024 * 1024)
        ));
    }
    let f = std::fs::File::open(path).map_err(|e| e.to_string())?;
    Ok(parse_xmltv_programmes_read(
        BufReader::new(f),
        OffsetDateTime::now_utc(),
    ))
}

pub fn parse_xmltv_programmes(xml: &str, now: OffsetDateTime) -> Vec<(String, String, String, String)> {
    parse_xmltv_programmes_read(xml.as_bytes(), now)
}

fn parse_xmltv_programmes_read<R: BufRead>(
    src: R,
    now: OffsetDateTime,
) -> Vec<(String, String, String, String)> {
    let window_start = now - Duration::hours(8);
    let window_end = now + Duration::hours(16);
    let mut list = Vec::new();
    let mut reader = Reader::from_reader(src);
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
                list.push((channel, title, format_utc_z(start), format_utc_z(stop)));
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

pub(crate) fn urlencoding_minimal(s: &str) -> String {
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
    match fetch_xmltv_conditional(url, None, None)? {
        FetchXmltv::Body { bytes, .. } => Ok(bytes),
        FetchXmltv::NotModified => Err("not modified".into()),
    }
}

pub fn fetch_xmltv_conditional(
    url: &str,
    etag: Option<&str>,
    last_modified: Option<&str>,
) -> Result<FetchXmltv, String> {
    let mut req = ureq::get(url).set("User-Agent", USER_AGENT);
    if let Some(e) = etag.map(str::trim).filter(|s| !s.is_empty()) {
        req = req.set("If-None-Match", e);
    }
    if let Some(lm) = last_modified.map(str::trim).filter(|s| !s.is_empty()) {
        req = req.set("If-Modified-Since", lm);
    }
    match req.call() {
        Err(ureq::Error::Status(304, _)) => Ok(FetchXmltv::NotModified),
        Err(e) => Err(e.to_string()),
        Ok(resp) => {
            if resp.status() == 304 {
                return Ok(FetchXmltv::NotModified);
            }
            let etag = resp.header("etag").map(|s| s.to_string());
            let last_modified = resp.header("last-modified").map(|s| s.to_string());
            if let Some(len) = resp
                .header("content-length")
                .and_then(|s| s.parse::<u64>().ok())
            {
                if len > XMLTV_DOWNLOAD_MAX {
                    return Err(format!(
                        "XMLTV Content-Length is {} MB (max {} MB) — refused.",
                        len / (1024 * 1024),
                        XMLTV_DOWNLOAD_MAX / (1024 * 1024)
                    ));
                }
            }
            let bytes = read_capped(resp.into_reader(), XMLTV_DOWNLOAD_MAX)?;
            Ok(FetchXmltv::Body {
                bytes,
                etag,
                last_modified,
            })
        }
    }
}

/// Download XMLTV (prefer gzip) and write uncompressed XML to `dest`. Streams
/// to disk so epg.monster's catalog does not need a 256 MB RAM cap.
pub fn fetch_xmltv_to_path(
    url: &str,
    dest: &Path,
    etag: Option<&str>,
    last_modified: Option<&str>,
) -> Result<FetchXmltv, String> {
    let url = prefer_compact_xmltv_url(url);
    let mut req = ureq::get(&url).set("User-Agent", USER_AGENT);
    if let Some(e) = etag.map(str::trim).filter(|s| !s.is_empty()) {
        req = req.set("If-None-Match", e);
    }
    if let Some(lm) = last_modified.map(str::trim).filter(|s| !s.is_empty()) {
        req = req.set("If-Modified-Since", lm);
    }
    match req.call() {
        Err(ureq::Error::Status(304, _)) => Ok(FetchXmltv::NotModified),
        Err(e) => Err(e.to_string()),
        Ok(resp) => {
            if resp.status() == 304 {
                return Ok(FetchXmltv::NotModified);
            }
            let etag = resp.header("etag").map(|s| s.to_string());
            let last_modified = resp.header("last-modified").map(|s| s.to_string());
            if let Some(len) = resp
                .header("content-length")
                .and_then(|s| s.parse::<u64>().ok())
            {
                if len > XMLTV_DOWNLOAD_MAX {
                    return Err(format!(
                        "XMLTV Content-Length is {} MB (max {} MB gzip) — refused. Use epg.xml.gz.",
                        len / (1024 * 1024),
                        XMLTV_DOWNLOAD_MAX / (1024 * 1024)
                    ));
                }
            }
            let part = dest.with_extension("part");
            copy_capped(resp.into_reader(), &part, XMLTV_DOWNLOAD_MAX)?;
            let gzip = {
                let mut mag = [0u8; 2];
                let n = std::fs::File::open(&part)
                    .and_then(|mut f| f.read(&mut mag))
                    .unwrap_or(0);
                n >= 2 && mag[0] == 0x1f && mag[1] == 0x8b
            };
            if gzip {
                let gz = std::fs::File::open(&part).map_err(|e| e.to_string())?;
                copy_capped(GzDecoder::new(gz), dest, XMLTV_FILE_MAX)?;
                let _ = std::fs::remove_file(&part);
            } else {
                let len = std::fs::metadata(&part).map(|m| m.len()).unwrap_or(0);
                if xmltv_too_large(len) {
                    let _ = std::fs::remove_file(&part);
                    return Err(format!(
                        "XMLTV is {} MB uncompressed (max {} MB) — use https://epg.monster/epg.xml.gz.",
                        len / (1024 * 1024),
                        XMLTV_FILE_MAX / (1024 * 1024)
                    ));
                }
                std::fs::rename(&part, dest).map_err(|e| e.to_string())?;
            }
            Ok(FetchXmltv::Body {
                bytes: Vec::new(),
                etag,
                last_modified,
            })
        }
    }
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
    fn lookup_ids_strip_locals_suffix() {
        let ids = tvg_lookup_ids("KAUT-DT.us_locals1.us (src05)");
        assert!(ids.iter().any(|s| s.eq_ignore_ascii_case("KAUT-DT.us")));
        assert!(ids.iter().any(|s| s.eq_ignore_ascii_case("KAUT-DT.us_locals1.us")));
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
    fn programme_window_keeps_on_air_at_index_now() {
        let now = try_parse_xmltv_time("20260101123000 +0000").unwrap();
        let xml = r#"<?xml version="1.0"?>
<tv>
  <programme start="20260101120000 +0000" stop="20260101130000 +0000" channel="CNN.us">
    <title>News Hour</title>
  </programme>
  <programme start="20260102120000 +0000" stop="20260102130000 +0000" channel="CNN.us">
    <title>Tomorrow</title>
  </programme>
</tv>"#;
        let rows = parse_xmltv_programmes(xml, now);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].1, "News Hour");
        assert_eq!(rows[0].2, "2026-01-01T12:00:00Z");
        assert_eq!(rows[0].3, "2026-01-01T13:00:00Z");
    }

    #[test]
    fn cache_meta_freshness() {
        let mut m = EpgCacheMeta::default();
        assert!(!m.index_is_fresh(REFRESH_INTERVAL_SECS));
        m.last_index_at = Some(OffsetDateTime::now_utc().format(&Rfc3339).unwrap());
        assert!(m.index_is_fresh(REFRESH_INTERVAL_SECS));
        m.last_index_at = Some(
            (OffsetDateTime::now_utc() - Duration::minutes(31))
                .format(&Rfc3339)
                .unwrap(),
        );
        assert!(!m.index_is_fresh(REFRESH_INTERVAL_SECS));
    }

    #[test]
    fn gzip_magic() {
        assert!(looks_like_gzip(&[0x1f, 0x8b, 0x00]));
        assert!(!looks_like_gzip(b"<tv>"));
    }

    #[test]
    fn read_capped_refuses_over_max() {
        let data = vec![0u8; 32];
        assert!(read_capped(Cursor::new(&data), 16).is_err());
        assert_eq!(read_capped(Cursor::new(&data), 32).unwrap().len(), 32);
    }

    #[test]
    fn purge_oversized_xmltv_deletes_only_huge_guides() {
        let tmp = tempfile::tempdir().unwrap();
        let keep = tmp.path().join("keep.xml");
        let dropf = tmp.path().join("huge.xml");
        let skip = tmp.path().join("notes.txt");
        std::fs::write(&keep, b"<tv/>").unwrap();
        std::fs::write(&dropf, vec![0u8; 64]).unwrap();
        std::fs::write(&skip, b"x").unwrap();
        assert_eq!(purge_oversized_xmltv_over(tmp.path(), 32), 1);
        assert!(keep.is_file());
        assert!(!dropf.is_file());
        assert!(skip.is_file());
    }
}
