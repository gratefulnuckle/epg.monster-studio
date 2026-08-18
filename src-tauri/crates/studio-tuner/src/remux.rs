// SPDX-License-Identifier: GPL-3.0-or-later

use std::io::{Read, Write};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemuxEngine {
    Ffmpeg,
    Vlc,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemuxProfile {
    Mpeg2Ac3,
    CopyAac,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemuxStop {
    ClientGone,
    UpstreamEnded,
    Failed,
}

pub const DEFAULT_PREROLL_BYTES: usize = 2_000_000;
pub const PREROLL_TIMEOUT: Duration = Duration::from_secs(16);
pub const DEFAULT_BUFFER_KB: i32 = 2048;

pub fn parse_engine(s: &str) -> RemuxEngine {
    if s.eq_ignore_ascii_case("vlc") {
        RemuxEngine::Vlc
    } else {
        RemuxEngine::Ffmpeg
    }
}

pub fn parse_profile(s: &str) -> RemuxProfile {
    if s.eq_ignore_ascii_case("copy_aac") {
        RemuxProfile::CopyAac
    } else {
        RemuxProfile::Mpeg2Ac3
    }
}

pub fn engine_key(e: RemuxEngine) -> &'static str {
    match e {
        RemuxEngine::Vlc => "vlc",
        RemuxEngine::Ffmpeg => "ffmpeg",
    }
}

pub fn profile_key(p: RemuxProfile) -> &'static str {
    match p {
        RemuxProfile::CopyAac => "copy_aac",
        RemuxProfile::Mpeg2Ac3 => "mpeg2_ac3",
    }
}

/// MPEG2+AC3 is ffmpeg-only (Plex-safe). VLC stays on Threadfin copy.
pub fn effective_engine(engine: RemuxEngine, profile: RemuxProfile) -> RemuxEngine {
    if profile == RemuxProfile::Mpeg2Ac3 {
        RemuxEngine::Ffmpeg
    } else {
        engine
    }
}

pub fn lineup_codecs(engine: RemuxEngine, profile: RemuxProfile) -> (&'static str, &'static str) {
    if effective_engine(engine, profile) == RemuxEngine::Ffmpeg && profile == RemuxProfile::Mpeg2Ac3 {
        ("MPEG2", "AC3")
    } else {
        ("H264", "AAC")
    }
}

pub fn clamp_buffer_kb(kb: i32) -> i32 {
    if kb < 512 {
        DEFAULT_BUFFER_KB
    } else if kb > 16384 {
        8192
    } else {
        kb
    }
}

pub fn build_args(
    url: &str,
    headers: Option<&[(String, String)]>,
    user_agent: &str,
    profile: RemuxProfile,
) -> String {
    let mut args = String::from(
        "-hide_banner -loglevel error -nostdin -fflags +genpts+discardcorrupt+igndts -analyzeduration 1000000 -probesize 1000000 -reconnect 1 -reconnect_streamed 1 -reconnect_delay_max 2 ",
    );
    let mut ua = user_agent.to_string();
    if let Some(hs) = headers {
        for (k, v) in hs {
            if k.eq_ignore_ascii_case("User-Agent") && !v.trim().is_empty() {
                ua = v.clone();
            }
        }
    }
    if !ua.trim().is_empty() {
        args.push_str("-user_agent ");
        args.push_str(&quote(&ua));
        args.push(' ');
    }
    if let Some(hs) = headers {
        let mut extra = String::new();
        for (k, v) in hs {
            if k.eq_ignore_ascii_case("User-Agent") {
                continue;
            }
            extra.push_str(k);
            extra.push_str(": ");
            extra.push_str(v);
            extra.push_str("\r\n");
        }
        if !extra.is_empty() {
            args.push_str("-headers ");
            args.push_str(&quote(&extra));
            args.push(' ');
        }
    }
    args.push_str("-i ");
    args.push_str(&quote(url));
    args.push_str(" -map 0:v:0? -map 0:a:0? ");
    if profile == RemuxProfile::CopyAac {
        args.push_str("-c:v copy -c:a aac -b:a 192k -ac 2 -ar 48000 -copyts ");
    } else {
        args.push_str("-c:v mpeg2video -q:v 8 -g 15 -bf 0 -c:a ac3 -ac 2 -ar 48000 -b:a 192k -muxpreload 0 -muxdelay 0 ");
    }
    args.push_str("-f mpegts -mpegts_flags +resend_headers+initial_discontinuity pipe:1");
    args
}

pub fn build_vlc_args(url: &str, dest_ts: &str, user_agent: &str) -> String {
    let dst = dest_ts.replace('\\', "/");
    let ua = if user_agent.trim().is_empty() {
        "epg.monster-studio"
    } else {
        user_agent
    };
    [
        "-I dummy -q --play-and-exit --vout=dummy --aout=dummy",
        "--sout-video --sout-audio --no-osd --no-video-title-show",
        "--network-caching=3000",
        &quote(url),
        &format!(":http-user-agent={}", quote(ua)),
        "--sout",
        &format!("#std{{mux=ts,access=file,dst={dst}}}"),
    ]
    .join(" ")
}

pub fn has_video_header(buf: &[u8]) -> bool {
    if buf.len() < 5 {
        return false;
    }
    for i in 0..buf.len() - 4 {
        if buf[i] != 0 || buf[i + 1] != 0 {
            continue;
        }
        if buf[i + 2] == 1 {
            let n = buf[i + 3];
            if matches!(n, 0xB3 | 0x67 | 0x27 | 0x40 | 0x42) {
                return true;
            }
        } else if buf[i + 2] == 0 && buf[i + 3] == 1 && i + 4 < buf.len() {
            let n = buf[i + 4];
            if matches!(n, 0x67 | 0x27 | 0x40 | 0x42) {
                return true;
            }
        }
    }
    false
}

pub fn copy(
    engine: RemuxEngine,
    profile: RemuxProfile,
    ffmpeg_path: &str,
    vlc_path: &str,
    url: &str,
    headers: Option<&[(String, String)]>,
    user_agent: &str,
    preroll_bytes: usize,
    output: &mut impl Write,
) -> RemuxStop {
    let engine = effective_engine(engine, profile);
    let preroll = if preroll_bytes < 64 * 1024 {
        DEFAULT_PREROLL_BYTES
    } else {
        preroll_bytes
    };
    if engine == RemuxEngine::Vlc {
        run_vlc(vlc_path, url, user_agent, preroll, output)
    } else {
        run_ffmpeg(ffmpeg_path, profile, url, headers, user_agent, preroll, output)
    }
}

fn run_ffmpeg(
    ffmpeg_path: &str,
    profile: RemuxProfile,
    url: &str,
    headers: Option<&[(String, String)]>,
    user_agent: &str,
    preroll: usize,
    output: &mut impl Write,
) -> RemuxStop {
    if ffmpeg_path.trim().is_empty() || !Path::new(ffmpeg_path).is_file() {
        return RemuxStop::Failed;
    }
    let argv = ffmpeg_argv(url, headers, user_agent, profile);
    let mut child = match spawn(ffmpeg_path, &argv, true) {
        Some(c) => c,
        None => return RemuxStop::Failed,
    };
    let stop = if let Some(mut stdout) = child.stdout.take() {
        pump_buffered(&mut stdout, output, preroll)
    } else {
        RemuxStop::Failed
    };
    kill(&mut child);
    stop
}

fn run_vlc(
    vlc_path: &str,
    url: &str,
    user_agent: &str,
    preroll: usize,
    output: &mut impl Write,
) -> RemuxStop {
    if vlc_path.trim().is_empty() || !Path::new(vlc_path).is_file() {
        return RemuxStop::Failed;
    }
    let tmp = std::env::temp_dir().join(format!("epg_vlc_{}", uuid::Uuid::new_v4().simple()));
    if std::fs::create_dir_all(&tmp).is_err() {
        return RemuxStop::Failed;
    }
    let dest = tmp.join("1.ts");
    let dest_s = dest.to_string_lossy().replace('\\', "/");
    let argv = vlc_argv(url, &dest_s, user_agent);
    let mut child = match spawn(vlc_path, &argv, false) {
        Some(c) => c,
        None => {
            let _ = std::fs::remove_dir_all(&tmp);
            return RemuxStop::Failed;
        }
    };
    let stop = match open_growing_file(&dest, &mut child) {
        Some(mut file) => {
            let mut grow = GrowingFile { file: &mut file, child: &mut child };
            pump_buffered(&mut grow, output, preroll)
        }
        None => RemuxStop::Failed,
    };
    kill(&mut child);
    let _ = std::fs::remove_dir_all(&tmp);
    stop
}

fn ffmpeg_argv(
    url: &str,
    headers: Option<&[(String, String)]>,
    user_agent: &str,
    profile: RemuxProfile,
) -> Vec<String> {
    let mut a = vec![
        "-hide_banner".into(),
        "-loglevel".into(),
        "error".into(),
        "-nostdin".into(),
        "-fflags".into(),
        "+genpts+discardcorrupt+igndts".into(),
        "-analyzeduration".into(),
        "1000000".into(),
        "-probesize".into(),
        "1000000".into(),
        "-reconnect".into(),
        "1".into(),
        "-reconnect_streamed".into(),
        "1".into(),
        "-reconnect_delay_max".into(),
        "2".into(),
    ];
    let mut ua = user_agent.to_string();
    if let Some(hs) = headers {
        for (k, v) in hs {
            if k.eq_ignore_ascii_case("User-Agent") && !v.trim().is_empty() {
                ua = v.clone();
            }
        }
    }
    if !ua.trim().is_empty() {
        a.push("-user_agent".into());
        a.push(ua);
    }
    if let Some(hs) = headers {
        let mut extra = String::new();
        for (k, v) in hs {
            if k.eq_ignore_ascii_case("User-Agent") {
                continue;
            }
            extra.push_str(k);
            extra.push_str(": ");
            extra.push_str(v);
            extra.push_str("\r\n");
        }
        if !extra.is_empty() {
            a.push("-headers".into());
            a.push(extra);
        }
    }
    a.push("-i".into());
    a.push(url.into());
    a.extend(["-map".into(), "0:v:0?".into(), "-map".into(), "0:a:0?".into()]);
    if profile == RemuxProfile::CopyAac {
        a.extend([
            "-c:v".into(),
            "copy".into(),
            "-c:a".into(),
            "aac".into(),
            "-b:a".into(),
            "192k".into(),
            "-ac".into(),
            "2".into(),
            "-ar".into(),
            "48000".into(),
            "-copyts".into(),
        ]);
    } else {
        a.extend([
            "-c:v".into(),
            "mpeg2video".into(),
            "-q:v".into(),
            "8".into(),
            "-g".into(),
            "15".into(),
            "-bf".into(),
            "0".into(),
            "-c:a".into(),
            "ac3".into(),
            "-ac".into(),
            "2".into(),
            "-ar".into(),
            "48000".into(),
            "-b:a".into(),
            "192k".into(),
            "-muxpreload".into(),
            "0".into(),
            "-muxdelay".into(),
            "0".into(),
        ]);
    }
    a.extend([
        "-f".into(),
        "mpegts".into(),
        "-mpegts_flags".into(),
        "+resend_headers+initial_discontinuity".into(),
        "pipe:1".into(),
    ]);
    a
}

fn vlc_argv(url: &str, dest: &str, user_agent: &str) -> Vec<String> {
    let ua = if user_agent.trim().is_empty() {
        "epg.monster-studio"
    } else {
        user_agent
    };
    vec![
        "-I".into(),
        "dummy".into(),
        "-q".into(),
        "--play-and-exit".into(),
        "--vout=dummy".into(),
        "--aout=dummy".into(),
        "--sout-video".into(),
        "--sout-audio".into(),
        "--no-osd".into(),
        "--no-video-title-show".into(),
        "--network-caching=3000".into(),
        url.into(),
        format!(":http-user-agent={ua}"),
        "--sout".into(),
        format!("#std{{mux=ts,access=file,dst={dest}}}"),
    ]
}

fn spawn(exe: &str, args: &[String], stdout: bool) -> Option<Child> {
    let mut cmd = Command::new(exe);
    cmd.args(args)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .stdout(if stdout { Stdio::piped() } else { Stdio::null() });
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd.spawn().ok()
}

fn kill(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

fn pump_buffered(src: &mut impl Read, dest: &mut impl Write, preroll: usize) -> RemuxStop {
    let mut bag = Vec::with_capacity(preroll.min(4 * 1024 * 1024) + 188 * 128);
    let mut buf = [0u8; 188 * 128];
    let mut ready = false;
    let mut saw_video = false;
    let deadline = Instant::now() + PREROLL_TIMEOUT;
    loop {
        let n = match src.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => {
                return if ready {
                    RemuxStop::ClientGone
                } else {
                    RemuxStop::Failed
                };
            }
        };
        if !saw_video {
            saw_video = has_video_header(&buf[..n]);
        }
        if !ready {
            bag.extend_from_slice(&buf[..n]);
            if (bag.len() >= preroll && saw_video) || Instant::now() >= deadline {
                ready = true;
                if !bag.is_empty() && dest.write_all(&bag).is_err() {
                    return RemuxStop::ClientGone;
                }
                bag.clear();
                bag.shrink_to_fit();
            }
            continue;
        }
        if dest.write_all(&buf[..n]).is_err() {
            return RemuxStop::ClientGone;
        }
    }
    if !ready && !bag.is_empty() && dest.write_all(&bag).is_err() {
        return RemuxStop::ClientGone;
    }
    RemuxStop::UpstreamEnded
}

fn open_growing_file(path: &Path, child: &mut Child) -> Option<std::fs::File> {
    let until = Instant::now() + Duration::from_secs(20);
    while Instant::now() < until {
        if path.is_file() {
            if let Ok(meta) = std::fs::metadata(path) {
                if meta.len() > 0 {
                    if let Ok(f) = std::fs::OpenOptions::new().read(true).open(path) {
                        return Some(f);
                    }
                }
            }
        }
        if child.try_wait().ok().flatten().is_some() {
            return None;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    if path.is_file() {
        std::fs::OpenOptions::new().read(true).open(path).ok()
    } else {
        None
    }
}

struct GrowingFile<'a> {
    file: &'a mut std::fs::File,
    child: &'a mut Child,
}

impl Read for GrowingFile<'_> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        loop {
            match self.file.read(buf) {
                Ok(0) => {
                    if self.child.try_wait()?.is_some() {
                        return Ok(0);
                    }
                    std::thread::sleep(Duration::from_millis(40));
                }
                other => return other,
            }
        }
    }
}

fn quote(s: &str) -> String {
    format!("\"{}\"", s.replace('"', "\\\""))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_args_mpeg2_ac3_is_plex_safe_default() {
        let args = build_args(
            "http://example/live.m3u8",
            None,
            "epg.monster-studio/test",
            RemuxProfile::Mpeg2Ac3,
        );
        assert!(args.contains("mpeg2video"));
        assert!(args.contains("-c:a ac3"));
        assert!(args.contains("-ac 2"));
        assert!(args.contains("-ar 48000"));
        assert!(args.contains("-f mpegts"));
        assert!(args.contains("resend_headers"));
        assert!(!args.contains("libx264"));
        assert!(!args.contains("provider"));
    }

    #[test]
    fn build_args_copy_aac_matches_threadfin() {
        let args = build_args(
            "http://example/live.m3u8",
            None,
            "epg.monster-studio/test",
            RemuxProfile::CopyAac,
        );
        assert!(args.contains("-c:v copy"));
        assert!(args.contains("-c:a aac"));
        assert!(args.contains("-ac 2"));
        assert!(args.contains("-f mpegts"));
        assert!(!args.contains("mpeg2video"));
    }

    #[test]
    fn build_vlc_args_is_threadfin_copy_to_file() {
        let args = build_vlc_args(
            "http://example/live.m3u8",
            r"C:\Temp\1.ts",
            "epg.monster-studio/test",
        );
        assert!(args.contains("-I dummy"));
        assert!(args.contains("#std{mux=ts,access=file,dst=C:/Temp/1.ts}"));
        assert!(!args.contains("access=http"));
        assert!(!args.contains("transcode"));
        assert!(!args.contains("provider"));
    }

    #[test]
    fn has_video_header_detects_mpeg2_sequence() {
        let buf = [0x47, 0x00, 0x00, 0x01, 0xB3, 0x20, 0x00];
        assert!(has_video_header(&buf));
        assert!(!has_video_header(&[0x00, 0x00, 0x01, 0x00]));
    }

    #[test]
    fn mpeg2_profile_forces_ffmpeg_even_if_vlc_selected() {
        assert_eq!(
            effective_engine(RemuxEngine::Vlc, RemuxProfile::Mpeg2Ac3),
            RemuxEngine::Ffmpeg
        );
        assert_eq!(
            effective_engine(RemuxEngine::Vlc, RemuxProfile::CopyAac),
            RemuxEngine::Vlc
        );
        let (v, a) = lineup_codecs(RemuxEngine::Ffmpeg, RemuxProfile::Mpeg2Ac3);
        assert_eq!(v, "MPEG2");
        assert_eq!(a, "AC3");
    }
}
