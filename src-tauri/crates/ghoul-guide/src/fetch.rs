// SPDX-License-Identifier: GPL-3.0-or-later

//! Streamed fetch of playlists and XMLTV. Caps bytes, inflates gzip, never
//! `read_to_string`s an unbounded source. Error strings do not include URLs.

use std::io::{self, Read};
use std::thread;
use std::time::Duration;

pub const PLAYLIST_MAX: u64 = 32 * 1024 * 1024;
pub const XML_MAX: u64 = 256 * 1024 * 1024;

const IO_ATTEMPTS: u32 = 8;

fn io_transient(err: &io::Error) -> bool {
    matches!(
        err.kind(),
        io::ErrorKind::PermissionDenied | io::ErrorKind::AlreadyExists
    ) || matches!(err.raw_os_error(), Some(5) | Some(32))
}

/// 8 attempts; backoff 50ms, 100ms, 200ms, … on lock/permission errors.
pub fn retry_io<T>(mut op: impl FnMut() -> io::Result<T>) -> io::Result<T> {
    let mut delay = Duration::from_millis(50);
    let mut attempt = 0u32;
    loop {
        match op() {
            Ok(v) => return Ok(v),
            Err(e) => {
                attempt += 1;
                if attempt >= IO_ATTEMPTS || !io_transient(&e) {
                    return Err(e);
                }
                thread::sleep(delay);
                delay = delay.saturating_mul(2);
            }
        }
    }
}

pub fn looks_like_http(s: &str) -> bool {
    let s = s.trim();
    s.len() > 7
        && (s[..7].eq_ignore_ascii_case("http://")
            || s.get(..8)
                .is_some_and(|p| p.eq_ignore_ascii_case("https://")))
}

/// Host + last path segment only. Never the query string or credentials.
pub fn redact_url(url: &str) -> String {
    let t = url.trim();
    if t.starts_with("file:") {
        let last = t.rsplit(['/', '\\']).find(|s| !s.is_empty()).unwrap_or("");
        return format!("file:…/{last}");
    }
    if let Some(scheme_end) = t.find("://") {
        let rest = &t[scheme_end + 3..];
        let host = rest.split(['/', '?', '#']).next().unwrap_or("");
        let path = rest.split(['?', '#']).next().unwrap_or(rest);
        let last = path.rsplit('/').find(|s| !s.is_empty()).unwrap_or("");
        return format!("{}://{host}/…/{last}", &t[..scheme_end]);
    }
    "…".into()
}

pub fn wrap_maybe_gzip<R: Read + 'static>(mut r: R) -> Result<Box<dyn Read>, String> {
    let mut peek = [0u8; 2];
    let n = r.read(&mut peek).map_err(|e| e.to_string())?;
    let chained = std::io::Cursor::new(peek[..n].to_vec()).chain(r);
    if n >= 2 && peek[0] == 0x1f && peek[1] == 0x8b {
        Ok(Box::new(flate2::read::GzDecoder::new(chained)))
    } else {
        Ok(Box::new(chained))
    }
}

fn http_get(src: &str, timeout_secs: u64) -> Result<Box<dyn Read>, String> {
    let resp = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(timeout_secs))
        .user_agent("G-houl/2.0")
        .build()
        .get(src)
        .call()
        .map_err(|e| match e {
            ureq::Error::Status(code, _) => format!("fetch failed (HTTP {code})"),
            _ => "fetch failed".into(),
        })?;
    Ok(Box::new(resp.into_reader()))
}

fn open_src(src: &str, timeout_secs: u64) -> Result<Box<dyn Read + 'static>, String> {
    if looks_like_http(src) {
        let r = http_get(src, timeout_secs)?;
        Ok(r)
    } else {
        let f = retry_io(|| std::fs::File::open(src)).map_err(|_| "file open failed".to_string())?;
        Ok(Box::new(f))
    }
}

/// Playlist bytes, gzip-inflated, capped at 32 MiB.
pub fn load_playlist_text(src: &str) -> Result<String, String> {
    let raw = open_src(src, 30)?;
    let mut reader = wrap_maybe_gzip(raw.take(PLAYLIST_MAX))?;
    let mut buf = String::new();
    reader
        .read_to_string(&mut buf)
        .map_err(|_| "playlist read failed".to_string())?;
    Ok(buf)
}

/// XMLTV reader, gzip-inflated, capped at 256 MiB. Caller parses the stream.
pub fn open_xml_reader(src: &str) -> Result<Box<dyn Read>, String> {
    let raw = open_src(src, 45)?;
    wrap_maybe_gzip(raw.take(XML_MAX))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_http_to_host_and_tail() {
        let s = redact_url("https://cdn.example/live/secret.ts?token=abc");
        assert!(s.starts_with("https://cdn.example/"));
        assert!(s.ends_with("/secret.ts"));
        assert!(!s.contains("token"));
        assert!(!s.contains("?"));
    }

    #[test]
    fn looks_like_http_is_case_insensitive() {
        assert!(looks_like_http("HTTPS://X/a"));
        assert!(looks_like_http("http://x"));
        assert!(!looks_like_http("file:///tmp/a.m3u"));
        assert!(!looks_like_http("C:\\\\playlists\\\\a.m3u"));
    }
}
