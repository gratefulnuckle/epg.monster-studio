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

pub fn find_on_path(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let p = dir.join(name);
        if p.is_file() {
            return Some(p);
        }
        #[cfg(windows)]
        {
            if !name.rsplit('.').next().is_some_and(|e| e.eq_ignore_ascii_case("exe")) {
                let with_exe = dir.join(format!("{name}.exe"));
                if with_exe.is_file() {
                    return Some(with_exe);
                }
            }
        }
    }
    if !cfg!(windows) {
        for dir in ["/usr/bin", "/usr/local/bin", "/opt/homebrew/bin"] {
            let p = PathBuf::from(dir).join(name);
            if p.is_file() {
                return Some(p);
            }
        }
    }
    None
}

pub fn bundled_tool_path(app_root: &Path, folder: &str, stem: &str) -> PathBuf {
    app_root.join("tools").join(folder).join(tool_file_name(stem))
}

fn find_up_tool(start: &Path, folder: &str, stem: &str) -> Option<PathBuf> {
    let mut p = start.to_path_buf();
    for _ in 0..8 {
        let cand = bundled_tool_path(&p, folder, stem);
        if cand.is_file() {
            return Some(cand);
        }
        if !p.pop() {
            break;
        }
    }
    None
}

pub fn default_mpv_path(app_root: &Path) -> PathBuf {
    if let Some(found) = find_up_tool(app_root, "mpv", "mpv") {
        return found;
    }
    let bundled = bundled_tool_path(app_root, "mpv", "mpv");
    if cfg!(windows) {
        let local = dirs::data_local_dir().unwrap_or_else(|| PathBuf::from("."));
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        return first_existing(&[
            PathBuf::from(r"C:\Program Files\MPV Player\mpv.exe"),
            PathBuf::from(r"C:\Program Files\mpv\mpv.exe"),
            PathBuf::from(r"C:\Program Files (x86)\mpv\mpv.exe"),
            local.join("Microsoft").join("WinGet").join("Links").join("mpv.exe"),
            home.join("scoop").join("apps").join("mpv").join("current").join("mpv.exe"),
            PathBuf::from(r"C:\ProgramData\chocolatey\bin\mpv.exe"),
        ])
        .or_else(|| find_on_path("mpv.exe"))
        .unwrap_or(bundled);
    }
    find_on_path("mpv").unwrap_or(bundled)
}

/// `ffprobe.exe` / `ffplay.exe` next to a known `ffmpeg` binary.
pub fn sibling_tool(of: &Path, stem: &str) -> Option<PathBuf> {
    let p = of.parent()?.join(tool_file_name(stem));
    p.is_file().then_some(p)
}

fn windows_ffmpeg_bin(stem: &str) -> Vec<PathBuf> {
    let exe = tool_file_name(stem);
    let local = dirs::data_local_dir().unwrap_or_else(|| PathBuf::from("."));
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    vec![
        home.join("scoop")
            .join("apps")
            .join("ffmpeg")
            .join("current")
            .join("bin")
            .join(&exe),
        home.join("scoop").join("shims").join(&exe),
        local
            .join("Microsoft")
            .join("WinGet")
            .join("Links")
            .join(&exe),
        PathBuf::from(r"C:\ProgramData\chocolatey\bin").join(&exe),
        PathBuf::from(r"C:\ffmpeg\bin").join(&exe),
        PathBuf::from(r"C:\Program Files\ffmpeg\bin").join(&exe),
    ]
}

pub fn default_ffmpeg_path(app_root: &Path) -> PathBuf {
    if let Some(found) = find_up_tool(app_root, "ffmpeg", "ffmpeg") {
        return found;
    }
    let bundled = bundled_tool_path(app_root, "ffmpeg", "ffmpeg");
    if cfg!(windows) {
        return first_existing(&windows_ffmpeg_bin("ffmpeg"))
            .or_else(|| find_on_path("ffmpeg.exe"))
            .unwrap_or(bundled);
    }
    find_on_path("ffmpeg").unwrap_or(bundled)
}

pub fn default_ffprobe_path(app_root: &Path) -> PathBuf {
    if let Some(found) = find_up_tool(app_root, "ffmpeg", "ffprobe") {
        return found;
    }
    if let Some(p) = sibling_tool(&default_ffmpeg_path(app_root), "ffprobe") {
        return p;
    }
    let bundled = bundled_tool_path(app_root, "ffmpeg", "ffprobe");
    if cfg!(windows) {
        return first_existing(&windows_ffmpeg_bin("ffprobe"))
            .or_else(|| find_on_path("ffprobe.exe"))
            .unwrap_or(bundled);
    }
    find_on_path("ffprobe").unwrap_or(bundled)
}

pub fn default_ffplay_path(app_root: &Path) -> PathBuf {
    resolve_ffplay(app_root, "")
}

/// ffplay sits next to ffmpeg. Dev builds may have a cargo-target copy of
/// ffmpeg.exe without ffplay — keep searching cwd / configured ffmpeg.
pub fn resolve_ffplay(app_root: &Path, ffmpeg_path: &str) -> PathBuf {
    let name = tool_file_name("ffplay");
    let mut candidates = Vec::new();
    let push_sibling = |ffmpeg: &Path, out: &mut Vec<PathBuf>| {
        if let Some(dir) = ffmpeg.parent() {
            out.push(dir.join(&name));
        }
    };
    if !ffmpeg_path.trim().is_empty() {
        push_sibling(Path::new(ffmpeg_path.trim()), &mut candidates);
    }
    if let Some(ff) = find_up_tool(app_root, "ffmpeg", "ffmpeg") {
        push_sibling(&ff, &mut candidates);
    }
    let mut starts = vec![app_root.to_path_buf()];
    if let Ok(cwd) = std::env::current_dir() {
        starts.push(cwd);
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            starts.push(dir.to_path_buf());
        }
    }
    for start in &starts {
        if let Some(found) = find_up_tool(start, "ffmpeg", "ffplay") {
            candidates.push(found);
        }
    }
    candidates.push(bundled_tool_path(app_root, "ffmpeg", "ffplay"));
    if let Some(p) = find_on_path(if cfg!(windows) { "ffplay.exe" } else { "ffplay" }) {
        candidates.push(p);
    }
    first_existing(&candidates).unwrap_or_else(|| bundled_tool_path(app_root, "ffmpeg", "ffplay"))
}

pub fn default_vlc_path() -> PathBuf {
    if cfg!(windows) {
        return first_existing(&[
            PathBuf::from(r"C:\Program Files\VideoLAN\VLC\vlc.exe"),
            PathBuf::from(r"C:\Program Files (x86)\VideoLAN\VLC\vlc.exe"),
        ])
        .or_else(|| find_on_path("vlc.exe"))
        .unwrap_or_else(|| PathBuf::from(r"C:\Program Files\VideoLAN\VLC\vlc.exe"));
    }
    find_on_path("vlc")
        .or_else(|| first_existing(&[PathBuf::from("/Applications/VLC.app/Contents/MacOS/VLC")]))
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
        (
            "ffplay",
            app_root
                .join("tools")
                .join("ffmpeg")
                .join(tool_file_name("ffplay")),
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

    #[test]
    fn default_mpv_path_walks_parents_for_bundled() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("tools").join("mpv");
        std::fs::create_dir_all(&dir).unwrap();
        let mpv = dir.join(tool_file_name("mpv"));
        std::fs::write(&mpv, b"x").unwrap();
        let nested = tmp.path().join("src-tauri").join("target");
        std::fs::create_dir_all(&nested).unwrap();
        assert_eq!(default_mpv_path(&nested), mpv);
    }

    #[test]
    fn sibling_tool_finds_ffprobe_next_to_ffmpeg() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("bin");
        std::fs::create_dir_all(&dir).unwrap();
        let ffmpeg = dir.join(tool_file_name("ffmpeg"));
        let ffprobe = dir.join(tool_file_name("ffprobe"));
        std::fs::write(&ffmpeg, b"ff").unwrap();
        std::fs::write(&ffprobe, b"pr").unwrap();
        assert_eq!(sibling_tool(&ffmpeg, "ffprobe").as_deref(), Some(ffprobe.as_path()));
    }

    #[test]
    fn resolve_ffplay_uses_sibling_of_configured_ffmpeg() {
        let tmp = tempfile::tempdir().unwrap();
        let decoy = tmp.path().join("cargo-target");
        std::fs::create_dir_all(decoy.join("tools").join("ffmpeg")).unwrap();
        std::fs::write(
            decoy.join("tools").join("ffmpeg").join(tool_file_name("ffmpeg")),
            b"ff",
        )
        .unwrap();
        let real = tmp.path().join("project").join("tools").join("ffmpeg");
        std::fs::create_dir_all(&real).unwrap();
        let ffmpeg = real.join(tool_file_name("ffmpeg"));
        let ffplay = real.join(tool_file_name("ffplay"));
        std::fs::write(&ffmpeg, b"ff").unwrap();
        std::fs::write(&ffplay, b"play").unwrap();
        assert_eq!(
            resolve_ffplay(&decoy, ffmpeg.to_string_lossy().as_ref()),
            ffplay
        );
    }
}
