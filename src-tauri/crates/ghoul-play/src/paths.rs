// SPDX-License-Identifier: GPL-3.0-or-later

use std::path::{Path, PathBuf};

pub fn tool_file_name(stem: &str) -> String {
    if cfg!(windows) {
        format!("{stem}.exe")
    } else {
        stem.to_string()
    }
}

fn first_file(cands: impl IntoIterator<Item = PathBuf>) -> Option<PathBuf> {
    cands.into_iter().find(|p| p.is_file())
}

fn home_dir() -> PathBuf {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

fn find_on_path(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let p = dir.join(name);
        if p.is_file() {
            return Some(p);
        }
        #[cfg(windows)]
        {
            if !name
                .rsplit('.')
                .next()
                .is_some_and(|e| e.eq_ignore_ascii_case("exe"))
            {
                let with_exe = dir.join(format!("{name}.exe"));
                if with_exe.is_file() {
                    return Some(with_exe);
                }
            }
        }
    }
    None
}

fn mpv_candidates(app_root: &Path) -> Vec<PathBuf> {
    let mut c = vec![app_root.join("tools").join("mpv").join(tool_file_name("mpv"))];
    #[cfg(windows)]
    {
        let home = home_dir();
        c.push(home.join("scoop").join("apps").join("mpv").join("current").join("mpv.exe"));
        c.push(home.join("scoop").join("apps").join("mpv-git").join("current").join("mpv.exe"));
        c.push(PathBuf::from(r"C:\Program Files\mpv\mpv.exe"));
        c.push(PathBuf::from(r"C:\Program Files\MPV Player\mpv.exe"));
        c.push(PathBuf::from(r"C:\Program Files (x86)\mpv\mpv.exe"));
        if let Some(found) = find_on_path("mpv.exe") {
            c.push(found);
        }
    }
    #[cfg(not(windows))]
    {
        if let Some(found) = find_on_path("mpv") {
            c.push(found);
        }
        c.push(PathBuf::from("/usr/bin/mpv"));
        c.push(PathBuf::from("/usr/local/bin/mpv"));
        c.push(PathBuf::from("/opt/homebrew/bin/mpv"));
        c.push(PathBuf::from("/Applications/mpv.app/Contents/MacOS/mpv"));
    }
    c
}

fn libmpv_candidates(app_root: &Path) -> Vec<PathBuf> {
    let bundled_dir = app_root.join("tools").join("mpv");
    let name = if cfg!(windows) {
        "libmpv-2.dll"
    } else if cfg!(target_os = "macos") {
        "libmpv.dylib"
    } else {
        "libmpv.so.2"
    };
    let mut c = vec![bundled_dir.join(name)];
    if let Some(mpv) = first_file(mpv_candidates(app_root)) {
        if let Some(dir) = mpv.parent() {
            c.push(dir.join(name));
        }
    }
    #[cfg(not(windows))]
    {
        c.push(PathBuf::from("/usr/lib/libmpv.so.2"));
        c.push(PathBuf::from("/usr/lib/x86_64-linux-gnu/libmpv.so.2"));
        c.push(PathBuf::from("/usr/local/lib/libmpv.dylib"));
        c.push(PathBuf::from("/opt/homebrew/lib/libmpv.dylib"));
    }
    c
}

pub fn mpv_exe(app_root: &Path) -> PathBuf {
    first_file(mpv_candidates(app_root)).unwrap_or_else(|| {
        app_root.join("tools").join("mpv").join(tool_file_name("mpv"))
    })
}

pub fn libmpv_path(app_root: &Path) -> PathBuf {
    let name = if cfg!(windows) {
        "libmpv-2.dll"
    } else if cfg!(target_os = "macos") {
        "libmpv.dylib"
    } else {
        "libmpv.so.2"
    };
    first_file(libmpv_candidates(app_root))
        .unwrap_or_else(|| app_root.join("tools").join("mpv").join(name))
}

pub fn ghoul_gst_exe(app_root: &Path) -> PathBuf {
    app_root
        .join("tools")
        .join("ghoul")
        .join(tool_file_name("ghoul-gst"))
}

pub fn is_gst_root(p: &Path) -> bool {
    let bin = p.join("bin");
    if !bin.is_dir() {
        return false;
    }
    let dlls = [
        "libgstreamer-1.0-0.dll",
        "gstreamer-1.0-0.dll",
        "libgstreamer-1.0.dylib",
        "libgstreamer-1.0.so.0",
        "gst-launch-1.0.exe",
        "gst-launch-1.0",
    ];
    dlls.iter().any(|n| bin.join(*n).is_file())
        || p.join("lib").join("libgstreamer-1.0.dylib").is_file()
        || p.join("lib").join("gstreamer-1.0").is_dir()
}

/// Locate a GStreamer 1.x prefix (`bin/` + `lib/gstreamer-1.0`).
pub fn find_gst_root(cli: Option<&str>, app_root: &Path) -> Option<PathBuf> {
    let mut cands: Vec<PathBuf> = Vec::new();
    if let Some(s) = cli.map(str::trim).filter(|s| !s.is_empty()) {
        cands.push(PathBuf::from(s));
    }
    for key in [
        "GHOUL_GST_DIR",
        "GSTREAMER_1_0_ROOT_MINGW_X86_64",
        "GSTREAMER_1_0_ROOT_MSVC_X86_64",
        "GSTREAMER_1_0_ROOT",
    ] {
        if let Some(v) = std::env::var_os(key) {
            if !v.is_empty() {
                cands.push(PathBuf::from(v));
            }
        }
    }
    cands.push(app_root.join("gstreamer"));
    cands.push(app_root.join("tools").join("gstreamer"));
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            cands.push(dir.join("gstreamer"));
            cands.push(dir.join("tools").join("gstreamer"));
        }
    }
    #[cfg(windows)]
    {
        cands.push(PathBuf::from(r"S:\toolchains\gstreamer"));
        if let Some(local) = std::env::var_os("LOCALAPPDATA") {
            let base = PathBuf::from(local)
                .join("Programs")
                .join("gstreamer")
                .join("1.0");
            cands.push(base.join("mingw_x86_64"));
            cands.push(base.join("msvc_x86_64"));
        }
        cands.push(PathBuf::from(r"C:\Program Files\gstreamer\1.0\mingw_x86_64"));
        cands.push(PathBuf::from(r"C:\Program Files\gstreamer\1.0\msvc_x86_64"));
    }
    #[cfg(target_os = "macos")]
    {
        cands.push(PathBuf::from(
            "/Library/Frameworks/GStreamer.framework/Versions/1.0",
        ));
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        cands.push(PathBuf::from("/usr"));
        cands.push(PathBuf::from("/usr/local"));
    }
    cands.into_iter().find(|p| is_gst_root(p))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_paths_are_under_tools() {
        let root = PathBuf::from("/app");
        let mpv = root.join("tools").join("mpv").join(tool_file_name("mpv"));
        let gst = root.join("tools").join("ghoul").join(tool_file_name("ghoul-gst"));
        assert!(mpv.ends_with(tool_file_name("mpv")));
        assert!(gst.ends_with(tool_file_name("ghoul-gst")));
    }
}
