// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::HashMap;
use std::io::Write;
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Instant;

use studio_core::hdhr;
use studio_core::lineup;
use studio_core::logo;
use studio_core::models::{EpgProgramme, ManagedChannel, StreamVariant};
use studio_core::settings::TunerServerProfile;

use crate::http;
use crate::remux::{self, RemuxStop};

#[derive(Clone, Default)]
pub struct TunerStats {
    pub discover: u32,
    pub lineup: u32,
    pub guide: u32,
    pub m3u: u32,
    pub stream: u32,
    pub not_found: u32,
    pub bytes: u64,
}

pub struct TunerSnapshot {
    pub channels: Vec<ManagedChannel>,
    pub programmes: Vec<EpgProgramme>,
    pub remux: bool,
    pub epg_url: Option<String>,
    pub host_logos: bool,
    pub use_local_logos: bool,
    pub logo_root: String,
    pub video_codec: String,
    pub audio_codec: String,
    pub ffmpeg_path: String,
    pub vlc_path: String,
    pub remux_engine: String,
    pub remux_profile: String,
    pub remux_buffer_bytes: i32,
    pub user_agent: String,
    pub variant_headers: HashMap<String, Vec<(String, String)>>,
    pub note_failover: Option<std::sync::Arc<dyn Fn(&ManagedChannel, &StreamVariant) + Send + Sync>>,
}

impl Default for TunerSnapshot {
    fn default() -> Self {
        Self {
            channels: vec![],
            programmes: vec![],
            remux: true,
            epg_url: None,
            host_logos: false,
            use_local_logos: false,
            logo_root: String::new(),
            video_codec: "H264".into(),
            audio_codec: "AAC".into(),
            ffmpeg_path: String::new(),
            vlc_path: String::new(),
            remux_engine: "ffmpeg".into(),
            remux_profile: "mpeg2_ac3".into(),
            remux_buffer_bytes: remux::DEFAULT_PREROLL_BYTES as i32,
            user_agent: studio_core::USER_AGENT.into(),
            variant_headers: HashMap::new(),
            note_failover: None,
        }
    }
}

pub struct TunerHost {
    profile: TunerServerProfile,
    snapshot: Arc<dyn Fn() -> TunerSnapshot + Send + Sync>,
    stop: Arc<AtomicBool>,
    active: Arc<AtomicU32>,
    max: Arc<AtomicU32>,
    pub last_error: Mutex<Option<String>>,
    pub stats: Mutex<TunerStats>,
    pub prefix: Mutex<String>,
}

impl TunerHost {
    pub fn new(
        profile: TunerServerProfile,
        snapshot: Arc<dyn Fn() -> TunerSnapshot + Send + Sync>,
    ) -> Self {
        let max = profile.tuner_count.clamp(1, 16) as u32;
        Self {
            profile,
            snapshot,
            stop: Arc::new(AtomicBool::new(false)),
            active: Arc::new(AtomicU32::new(0)),
            max: Arc::new(AtomicU32::new(max)),
            last_error: Mutex::new(None),
            stats: Mutex::new(TunerStats::default()),
            prefix: Mutex::new(String::new()),
        }
    }

    pub fn kind(&self) -> &str {
        &self.profile.kind
    }
    pub fn active(&self) -> u32 {
        self.active.load(Ordering::SeqCst)
    }
    pub fn max(&self) -> u32 {
        self.max.load(Ordering::SeqCst)
    }
    pub fn set_max(&self, n: i32) {
        self.max.store(n.clamp(1, 16) as u32, Ordering::SeqCst);
    }

    pub fn start(self: &Arc<Self>) -> Result<(), String> {
        let mut profile = self.profile.clone();
        profile.ensure_identity();
        let port = profile.port as u16;
        let bind = if profile.allow_lan {
            format!("0.0.0.0:{port}")
        } else {
            format!("127.0.0.1:{port}")
        };
        let listener = match TcpListener::bind(&bind) {
            Ok(l) => {
                *self.prefix.lock().unwrap() = if profile.allow_lan {
                    format!("http://0.0.0.0:{port}/")
                } else {
                    format!("http://127.0.0.1:{port}/")
                };
                *self.last_error.lock().unwrap() = None;
                l
            }
            Err(e) if profile.allow_lan => {
                let fallback = format!("127.0.0.1:{port}");
                let l = TcpListener::bind(&fallback).map_err(|e2| explain_bind(&e2))?;
                *self.prefix.lock().unwrap() = format!("http://127.0.0.1:{port}/");
                *self.last_error.lock().unwrap() = Some(format!(
                    "LAN bind failed — listening on 127.0.0.1 only. {}",
                    explain_bind(&e)
                ));
                l
            }
            Err(e) => {
                let msg = explain_bind(&e);
                *self.last_error.lock().unwrap() = Some(msg.clone());
                return Err(msg);
            }
        };
        listener.set_nonblocking(false).ok();
        let host = Arc::clone(self);
        thread::spawn(move || {
            for incoming in listener.incoming() {
                if host.stop.load(Ordering::SeqCst) {
                    break;
                }
                let Ok(mut stream) = incoming else { continue };
                let remote = stream.peer_addr().ok();
                let h = Arc::clone(&host);
                thread::spawn(move || {
                    let _ = handle_client(&h, &mut stream, remote);
                });
            }
        });
        Ok(())
    }

    pub fn stop(&self) {
        self.stop.store(true, Ordering::SeqCst);
    }
}

impl Drop for TunerHost {
    fn drop(&mut self) {
        self.stop();
    }
}

pub fn explain_bind(err: &std::io::Error) -> String {
    let msg = err.to_string();
    if msg.to_ascii_lowercase().contains("already") || matches!(err.kind(), std::io::ErrorKind::AddrInUse) {
        return "Port is already in use. Stop the other program or change the port in Settings.".into();
    }
    if matches!(err.kind(), std::io::ErrorKind::PermissionDenied) {
        return "Windows blocked this listen. Pick another port in Settings, or allow the app through the firewall once.".into();
    }
    msg
}

fn handle_client(host: &TunerHost, stream: &mut TcpStream, remote: Option<std::net::SocketAddr>) -> std::io::Result<()> {
    let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(15)));
    let Some(req) = http::read_request(stream, remote) else {
        return Ok(());
    };
    dispatch(host, stream, &req)
}

fn dispatch(host: &TunerHost, stream: &mut TcpStream, req: &http::TunerHttpRequest) -> std::io::Result<()> {
    let mut path = req.path.trim_end_matches('/').to_ascii_lowercase();
    if path.is_empty() {
        path = "/".into();
    }
    let sw = Instant::now();
    let base = public_base(&host.profile, req);

    if path == "/" || path == "/discover.json" {
        let body = hdhr::discover_json(&host.profile, &base);
        bump(host, |s| s.discover += 1, body.len());
        return http::write_text(stream, 200, "OK", "application/json", &body);
    }
    if path == "/lineup_status.json" || path == "/lineup.post" {
        let body = hdhr::lineup_status_json();
        bump(host, |s| s.lineup += 1, body.len());
        return http::write_text(stream, 200, "OK", "application/json", body);
    }
    if path == "/lineup.json" {
        let snap = (host.snapshot)();
        let body = hdhr::lineup_json(
            &snap.channels,
            &base,
            Some(&snap.video_codec),
            Some(&snap.audio_codec),
        );
        bump(host, |s| s.lineup += 1, body.len());
        return http::write_text(stream, 200, "OK", "application/json", &body);
    }
    if path.starts_with("/logos/") {
        let snap = (host.snapshot)();
        if !snap.host_logos && !snap.use_local_logos {
            bump(host, |s| s.not_found += 1, 0);
            return http::write_status(stream, 404, "Not Found", None);
        }
        if let Some(file) = logo::try_resolve_hosted(Path::new(&snap.logo_root), &path) {
            if let Ok(bytes) = std::fs::read(&file) {
                bump(host, |s| s.discover += 0, bytes.len());
                return http::write_bytes(stream, 200, "OK", "image/png", &bytes);
            }
        }
        bump(host, |s| s.not_found += 1, 0);
        return http::write_status(stream, 404, "Not Found", None);
    }
    if path == "/guide.xml" || path == "/xmltv.xml" {
        let snap = (host.snapshot)();
        let body = hdhr::guide_xml(&snap.channels, &snap.programmes, Some(&base), snap.use_local_logos);
        bump(host, |s| s.guide += 1, body.len());
        return http::write_text(stream, 200, "OK", "application/xml", &body);
    }
    if matches!(
        path.as_str(),
        "/tuner.m3u" | "/lineup.m3u" | "/playlist.m3u8" | "/playlist.m3u"
    ) {
        let snap = (host.snapshot)();
        let remux = host.profile.kind != "Iptv" || (host.profile.remux_enabled && snap.remux);
        let body = hdhr::tuner_m3u(
            &snap.channels,
            &base,
            snap.epg_url.as_deref(),
            remux,
            snap.use_local_logos,
        );
        bump(host, |s| s.m3u += 1, body.len());
        return http::write_text(stream, 200, "OK", "application/vnd.apple.mpegurl", &body);
    }
    if path == "/downspiral" || path.starts_with("/downspiral/") {
        if !host.profile.downspiral_enabled {
            bump(host, |s| s.not_found += 1, 0);
            return http::write_status(stream, 404, "Not Found", None);
        }
        return dispatch_downspiral(host, stream, &path, &base);
    }
    if let Some(number) = parse_channel(&path) {
        if req.method == "HEAD" {
            bump(host, |s| s.stream += 1, 0);
            return http::write_stream_headers(stream);
        }
        return stream_channel(host, stream, number, &path, sw);
    }
    bump(host, |s| s.not_found += 1, 0);
    http::write_status(stream, 404, "Not Found", None)
}

fn dispatch_downspiral(
    host: &TunerHost,
    stream: &mut TcpStream,
    path: &str,
    root: &str,
) -> std::io::Result<()> {
    let snap = (host.snapshot)();
    if path == "/downspiral" || path == "/downspiral/index.json" {
        let body = hdhr::downspiral_index_json(&snap.channels, root);
        bump(host, |s| s.m3u += 1, body.len());
        return http::write_text(stream, 200, "OK", "application/json", &body);
    }
    let leaf = &path["/downspiral/".len()..];
    let (slug, want_xml, want_m3u) = if let Some(s) = leaf.strip_suffix(".xml") {
        (s, true, false)
    } else if let Some(s) = leaf.strip_suffix(".m3u8") {
        (s, false, true)
    } else if let Some(s) = leaf.strip_suffix(".m3u") {
        (s, false, true)
    } else {
        bump(host, |s| s.not_found += 1, 0);
        return http::write_status(stream, 404, "Not Found", None);
    };
    let lists = hdhr::downspiral_lists(&snap.channels);
    let Some(hit) = lists.iter().find(|g| g.slug.eq_ignore_ascii_case(slug)) else {
        bump(host, |s| s.not_found += 1, 0);
        return http::write_status(stream, 404, "Not Found", None);
    };
    if want_xml {
        let xml = hdhr::guide_xml(&hit.channels, &snap.programmes, Some(root), snap.use_local_logos);
        bump(host, |s| s.guide += 1, xml.len());
        return http::write_text(stream, 200, "OK", "application/xml", &xml);
    }
    if want_m3u {
        let remux = host.profile.kind != "Iptv" || (host.profile.remux_enabled && snap.remux);
        let epg = format!("{}/downspiral/{}.xml", root.trim_end_matches('/'), hit.slug);
        let m3u = hdhr::tuner_m3u(&hit.channels, root, Some(&epg), remux, snap.use_local_logos);
        bump(host, |s| s.m3u += 1, m3u.len());
        return http::write_text(stream, 200, "OK", "application/vnd.apple.mpegurl", &m3u);
    }
    bump(host, |s| s.not_found += 1, 0);
    http::write_status(stream, 404, "Not Found", None)
}

fn stream_channel(
    host: &TunerHost,
    stream: &mut TcpStream,
    number: i32,
    path: &str,
    _sw: Instant,
) -> std::io::Result<()> {
    let snap = (host.snapshot)();
    let ch = lineup::by_number(&snap.channels, number);
    if ch.is_none() {
        let mut extra = HashMap::new();
        extra.insert("X-HDHomeRun-Error".into(), "801".into());
        bump(host, |s| s.not_found += 1, 0);
        return http::write_status(stream, 404, "Not Found", Some(&extra));
    }
    let ch = ch.unwrap();
    if failover_order(&ch).is_empty() {
        bump(host, |s| s.not_found += 1, 0);
        return http::write_status(stream, 404, "Not Found", None);
    }
    if host.active.load(Ordering::SeqCst) >= host.max.load(Ordering::SeqCst) {
        let mut extra = HashMap::new();
        extra.insert("X-HDHomeRun-Error".into(), "805".into());
        bump(host, |s| s.stream += 1, 0);
        return http::write_status(stream, 503, "Busy", Some(&extra));
    }
    let remux_snap = (host.snapshot)();
    let engine = remux::parse_engine(&remux_snap.remux_engine);
    let profile = remux::parse_profile(&remux_snap.remux_profile);
    let effective = remux::effective_engine(engine, profile);
    if effective == remux::RemuxEngine::Ffmpeg
        && (remux_snap.ffmpeg_path.is_empty() || !Path::new(&remux_snap.ffmpeg_path).is_file())
    {
        bump(host, |s| s.stream += 1, 0);
        return http::write_status(stream, 503, "No ffmpeg", None);
    }
    if effective == remux::RemuxEngine::Vlc
        && (remux_snap.vlc_path.is_empty() || !Path::new(&remux_snap.vlc_path).is_file())
    {
        bump(host, |s| s.stream += 1, 0);
        return http::write_status(stream, 503, "No VLC", None);
    }
    let preroll = if remux_snap.remux_buffer_bytes > 0 {
        remux_snap.remux_buffer_bytes as usize
    } else {
        remux::DEFAULT_PREROLL_BYTES
    };
    host.active.fetch_add(1, Ordering::SeqCst);
    let mut headers_sent = false;
    let mut bytes: u64 = 0;
    let mut first = true;
    for variant in failover_order(&ch) {
        if variant.url.trim().is_empty() {
            continue;
        }
        if !first && bytes == 0 {
            if let Some(cb) = &remux_snap.note_failover {
                cb(&ch, variant);
            }
        }
        first = false;
        let mut dest = FirstWrite {
            inner: &mut *stream,
            headers_sent: &mut headers_sent,
            bytes: &mut bytes,
        };
        let headers = remux_snap.variant_headers.get(&variant.id);
        let stop = remux::copy(
            engine,
            profile,
            &remux_snap.ffmpeg_path,
            &remux_snap.vlc_path,
            &variant.url,
            headers.map(|h| h.as_slice()),
            &remux_snap.user_agent,
            preroll,
            &mut dest,
        );
        if stop == RemuxStop::ClientGone {
            break;
        }
    }
    host.active.fetch_sub(1, Ordering::SeqCst);
    let _ = path;
    if !headers_sent {
        bump(host, |s| s.stream += 1, 0);
        return http::write_status(stream, 503, "No stream", None);
    }
    bump(host, |s| s.stream += 1, bytes as usize);
    let _ = stream.flush();
    Ok(())
}

pub fn failover_order(ch: &ManagedChannel) -> Vec<&StreamVariant> {
    let vis = ch.variants.iter().find(|v| v.visibility == "visible");
    let mut out = Vec::new();
    if let Some(v) = vis {
        out.push(v);
    }
    let mut backups: Vec<&StreamVariant> = ch
        .variants
        .iter()
        .filter(|v| v.visibility == "hidden_backup" && !v.url.trim().is_empty())
        .collect();
    backups.sort_by_key(|v| v.priority);
    for b in backups {
        if vis.is_some_and(|v| v.id == b.id) {
            continue;
        }
        out.push(b);
    }
    out
}

struct FirstWrite<'a> {
    inner: &'a mut TcpStream,
    headers_sent: &'a mut bool,
    bytes: &'a mut u64,
}

impl Write for FirstWrite<'_> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if !*self.headers_sent {
            http::write_stream_headers(self.inner)?;
            *self.headers_sent = true;
        }
        let n = self.inner.write(buf)?;
        *self.bytes += n as u64;
        Ok(n)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

fn parse_channel(path: &str) -> Option<i32> {
    let rest = path.strip_prefix("/auto/v")?;
    let n: i32 = rest.parse().ok()?;
    (n > 0).then_some(n)
}

fn public_base(profile: &TunerServerProfile, req: &http::TunerHttpRequest) -> String {
    let mut host = req.host.clone();
    if host.trim().is_empty() {
        return profile.base_url();
    }
    if !host.contains(':') {
        host = format!("{}:{}", host, profile.port);
    }
    format!("http://{host}")
}

fn bump(host: &TunerHost, f: impl FnOnce(&mut TunerStats), bytes: usize) {
    if let Ok(mut s) = host.stats.lock() {
        f(&mut s);
        s.bytes += bytes as u64;
    }
}

pub fn lan_ipv4() -> Vec<String> {
    let mut ips = Vec::new();
    if let Ok(s) = std::net::UdpSocket::bind("0.0.0.0:0") {
        if s.connect("8.8.8.8:80").is_ok() {
            if let Ok(addr) = s.local_addr() {
                if let std::net::IpAddr::V4(v) = addr.ip() {
                    if !v.is_loopback() && !v.is_unspecified() {
                        ips.push(v.to_string());
                    }
                }
            }
        }
    }
    ips
}
