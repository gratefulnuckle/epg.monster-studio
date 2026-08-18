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
