// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::SocketAddr;

pub struct TunerHttpRequest {
    pub method: String,
    pub path: String,
    pub host: String,
    pub remote: Option<SocketAddr>,
}

pub fn read_request(stream: &mut impl Read, remote: Option<SocketAddr>) -> Option<TunerHttpRequest> {
    let mut buf = Vec::new();
    let mut tmp = [0u8; 1024];
    while buf.len() < 65_536 {
        let n = stream.read(&mut tmp).ok()?;
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&tmp[..n]);
        if let Some(end) = find_header_end(&buf) {
            let header = std::str::from_utf8(&buf[..end]).ok()?;
            let mut lines = header.split("\r\n");
            let first = lines.next()?;
            let mut parts = first.splitn(3, ' ');
            let method = parts.next()?.to_ascii_uppercase();
            let raw = parts.next()?;
            let path_only = raw.split('?').next().unwrap_or("/");
            let mut host = String::new();
            for line in lines {
                if line.len() >= 5 && line[..5].eq_ignore_ascii_case("host:") {
                    host = line[5..].trim().to_string();
                    break;
                }
            }
            return Some(TunerHttpRequest {
                method,
                path: if path_only.trim().is_empty() {
                    "/".into()
                } else {
                    path_only.to_string()
                },
                host,
                remote,
            });
        }
    }
    None
}

fn find_header_end(buf: &[u8]) -> Option<usize> {
    buf.windows(4).position(|w| w == b"\r\n\r\n")
}

pub fn write_text(
    stream: &mut impl Write,
    status: u16,
    reason: &str,
    content_type: &str,
    body: &str,
) -> std::io::Result<()> {
    let bytes = body.as_bytes();
    let head = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\nCache-Control: no-store\r\n\r\n",
        bytes.len()
    );
    stream.write_all(head.as_bytes())?;
    if !bytes.is_empty() {
        stream.write_all(bytes)?;
    }
    stream.flush()
}

pub fn write_bytes(
    stream: &mut impl Write,
    status: u16,
    reason: &str,
    content_type: &str,
    body: &[u8],
) -> std::io::Result<()> {
    let head = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\nCache-Control: public, max-age=86400\r\n\r\n",
        body.len()
    );
    stream.write_all(head.as_bytes())?;
    if !body.is_empty() {
        stream.write_all(body)?;
    }
    stream.flush()
}

pub fn write_status(
    stream: &mut impl Write,
    status: u16,
    reason: &str,
    extra: Option<&HashMap<String, String>>,
) -> std::io::Result<()> {
    let mut head = format!("HTTP/1.1 {status} {reason}\r\nContent-Length: 0\r\nConnection: close\r\n");
    if let Some(map) = extra {
        for (k, v) in map {
            head.push_str(k);
            head.push_str(": ");
            head.push_str(v);
            head.push_str("\r\n");
        }
    }
    head.push_str("\r\n");
    stream.write_all(head.as_bytes())?;
    stream.flush()
}

pub fn write_stream_headers(stream: &mut impl Write) -> std::io::Result<()> {
    stream.write_all(
        b"HTTP/1.1 200 OK\r\nContent-Type: video/mp2t\r\nConnection: close\r\nCache-Control: no-store\r\n\r\n",
    )?;
    stream.flush()
}
