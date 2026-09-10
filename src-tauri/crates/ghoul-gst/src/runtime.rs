// SPDX-License-Identifier: GPL-3.0-or-later

use std::path::{Path, PathBuf};

/// Locate a GStreamer 1.x prefix (`bin/` + `lib/gstreamer-1.0`).
///
/// Windows ships a bundled prefix. Linux/macOS typically use the system
/// install (pkg-config / DYLD fallbacks) and only need this when the app
/// bundles its own runtime.
pub fn find_gst_root(cli: Option<&str>) -> Option<PathBuf> {
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
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            cands.push(dir.join("gstreamer"));
            cands.push(dir.join("tools").join("gstreamer"));
            cands.push(dir.join("..").join("tools").join("gstreamer"));
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        cands.push(cwd.join("tools").join("gstreamer"));
        cands.push(cwd.join("gstreamer"));
    }
    #[cfg(windows)]
    {
        cands.push(PathBuf::from(r"S:\toolchains\gstreamer"));
        if let Some(local) = std::env::var_os("LOCALAPPDATA") {
            let base = PathBuf::from(local).join("Programs").join("gstreamer").join("1.0");
            cands.push(base.join("mingw_x86_64"));
            cands.push(base.join("msvc_x86_64"));
        }
        cands.push(PathBuf::from(r"C:\Program Files\gstreamer\1.0\mingw_x86_64"));
        cands.push(PathBuf::from(r"C:\Program Files\gstreamer\1.0\msvc_x86_64"));
    }
    #[cfg(target_os = "macos")]
    {
        cands.push(PathBuf::from("/Library/Frameworks/GStreamer.framework/Versions/1.0"));
        cands.push(PathBuf::from(
            "/Library/Frameworks/GStreamer.framework/Versions/Current",
        ));
        if let Some(home) = std::env::var_os("HOME") {
            let h = PathBuf::from(home);
            cands.push(h.join("Library/Frameworks/GStreamer.framework/Versions/1.0"));
        }
    }
    cands.into_iter().find(|p| is_gst_root(p))
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
    dlls.iter().any(|n| bin.join(n).is_file())
        || p.join("lib").join("libgstreamer-1.0.dylib").is_file()
        || p.join("lib").join("gstreamer-1.0").is_dir()
}

/// Prepend the prefix to PATH / plugin search so playbin can load soup, libav, d3d11, etc.
pub fn apply_gst_root(root: &Path) {
    let bin = root.join("bin");
    if bin.is_dir() {
        prepend_env("PATH", &bin);
        #[cfg(target_os = "macos")]
        prepend_env("DYLD_LIBRARY_PATH", &root.join("lib"));
        #[cfg(unix)]
        prepend_env("LD_LIBRARY_PATH", &root.join("lib"));
    }
    let plugins = root.join("lib").join("gstreamer-1.0");
    if plugins.is_dir() {
        std::env::set_var("GST_PLUGIN_SYSTEM_PATH", &plugins);
        std::env::set_var("GST_PLUGIN_PATH", &plugins);
    }
    let scanner = if cfg!(windows) {
        bin.join("gst-plugin-scanner.exe")
    } else {
        root.join("libexec")
            .join("gstreamer-1.0")
            .join("gst-plugin-scanner")
    };
    if scanner.is_file() {
        std::env::set_var("GST_PLUGIN_SCANNER", scanner);
    }
    crate::log::line("info", &format!("gstreamer root {}", root.display()));
}

fn prepend_env(key: &str, dir: &Path) {
    let add = dir.to_string_lossy();
    match std::env::var(key) {
        Ok(prev) if !prev.is_empty() => {
            let sep = if cfg!(windows) { ';' } else { ':' };
            std::env::set_var(key, format!("{add}{sep}{prev}"));
        }
        _ => std::env::set_var(key, add.as_ref()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_empty_dir() {
        assert!(!is_gst_root(Path::new(".")));
    }
}
