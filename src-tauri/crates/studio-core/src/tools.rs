// SPDX-License-Identifier: GPL-3.0-or-later

use std::path::{Path, PathBuf};

pub fn tool_file_name(base: &str) -> String {
    let name = base.strip_suffix(".exe").unwrap_or(base);
    if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    }
}

fn first_existing(paths: &[PathBuf]) -> Option<PathBuf> {
    paths.iter().find(|p| p.is_file()).cloned()
}

fn first_on_path(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let p = dir.join(name);
        if p.is_file() {
            return Some(p);
        }
    }
    None
}

pub fn bundled_tool_path(app_root: &Path, folder: &str, stem: &str) -> PathBuf {
    app_root.join("tools").join(folder).join(tool_file_name(stem))
}

pub fn default_mpv_path(app_root: &Path) -> PathBuf {
    let bundled = bundled_tool_path(app_root, "mpv", "mpv");
    if bundled.is_file() {
        return bundled;
    }
    if cfg!(windows) {
        let local = dirs::data_local_dir().unwrap_or_else(|| PathBuf::from("."));
        return first_existing(&[
            PathBuf::from(r"C:\Program Files\MPV Player\mpv.exe"),
            local.join("Microsoft").join("WinGet").join("Links").join("mpv.exe"),
        ])
        .or_else(|| first_on_path("mpv.exe"))
        .unwrap_or(bundled);
    }
    first_on_path("mpv")
        .or_else(|| first_existing(&[PathBuf::from("/usr/bin/mpv"), PathBuf::from("/usr/local/bin/mpv")]))
        .unwrap_or(bundled)
}

pub fn default_ffmpeg_path(app_root: &Path) -> PathBuf {
    let bundled = bundled_tool_path(app_root, "ffmpeg", "ffmpeg");
    if bundled.is_file() {
        return bundled;
    }
    if cfg!(windows) {
        return bundled;
    }
    first_on_path("ffmpeg")
        .or_else(|| first_existing(&[PathBuf::from("/usr/bin/ffmpeg"), PathBuf::from("/usr/local/bin/ffmpeg")]))
        .unwrap_or(bundled)
}

pub fn default_ffprobe_path(app_root: &Path) -> PathBuf {
    let bundled = bundled_tool_path(app_root, "ffmpeg", "ffprobe");
    if bundled.is_file() {
        return bundled;
    }
    if cfg!(windows) {
        return bundled;
    }
    first_on_path("ffprobe")
        .or_else(|| first_existing(&[PathBuf::from("/usr/bin/ffprobe"), PathBuf::from("/usr/local/bin/ffprobe")]))
        .unwrap_or(bundled)
}

pub fn default_vlc_path() -> PathBuf {
    if cfg!(windows) {
        return first_existing(&[
            PathBuf::from(r"C:\Program Files\VideoLAN\VLC\vlc.exe"),
            PathBuf::from(r"C:\Program Files (x86)\VideoLAN\VLC\vlc.exe"),
        ])
        .or_else(|| first_on_path("vlc.exe"))
        .unwrap_or_else(|| PathBuf::from(r"C:\Program Files\VideoLAN\VLC\vlc.exe"));
    }
    first_on_path("vlc")
        .or_else(|| first_existing(&[PathBuf::from("/usr/bin/vlc"), PathBuf::from("/usr/local/bin/vlc")]))
        .unwrap_or_else(|| PathBuf::from("/usr/bin/vlc"))
}

pub fn detect_bundled(app_root: &Path) -> Vec<(String, PathBuf)> {
    let candidates = [
        ("mpv", app_root.join("tools").join("mpv").join(tool_file_name("mpv"))),
        (
            "ffmpeg",
            app_root
                .join("tools")
                .join("ffmpeg")
                .join(tool_file_name("ffmpeg")),
        ),
        (
            "ffprobe",
            app_root
                .join("tools")
                .join("ffmpeg")
                .join(tool_file_name("ffprobe")),
        ),
    ];
    candidates
        .into_iter()
        .filter(|(_, p)| p.exists())
        .map(|(name, path)| (name.to_string(), path))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_tool_name_has_exe() {
        let name = tool_file_name("ffmpeg");
        if cfg!(windows) {
            assert_eq!(name, "ffmpeg.exe");
        } else {
            assert_eq!(name, "ffmpeg");
        }
    }
}
