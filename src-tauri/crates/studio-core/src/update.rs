// SPDX-License-Identifier: GPL-3.0-or-later

use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Serialize;

use crate::info::{latest_github_release, remote_is_newer, GithubAsset, GithubRelease, VERSION};
use crate::paths::app_data_directory;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateFlavor {
    Desktop,
    Server,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyPlan {
    pub tag: String,
    pub asset_name: String,
    pub relaunching: bool,
}

pub fn flavor_from_process() -> UpdateFlavor {
    if std::env::var("STUDIO_BIND").is_ok()
        || std::env::var("STUDIO_UI_DIR").is_ok()
        || std::env::var("STUDIO_HEADLESS").is_ok()
    {
        return UpdateFlavor::Server;
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(name) = exe.file_name().and_then(|s| s.to_str()) {
            if name.to_ascii_lowercase().contains("studio-server") {
                return UpdateFlavor::Server;
            }
        }
    }
    UpdateFlavor::Desktop
}

pub fn pick_binary_asset<'a>(
    rel: &'a GithubRelease,
    flavor: UpdateFlavor,
) -> Option<&'a GithubAsset> {
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;
    let mut best: Option<(&'a GithubAsset, i32)> = None;
    for a in &rel.assets {
        let n = a.name.to_ascii_lowercase();
        let mut score = match flavor {
            UpdateFlavor::Server => {
                if !n.contains("studio-server") {
                    continue;
                }
                if n.contains("web-dist") || n.contains("dist.zip") {
                    continue;
                }
                5
            }
            UpdateFlavor::Desktop => {
                if n.contains("studio-server") {
                    continue;
                }
                if n.contains("web-dist") {
                    continue;
                }
                if n.contains("epg-monster-studio") || n.contains("epg.monster-studio") {
                    5
                } else if n.contains("appimage") {
                    4
                } else {
                    continue;
                }
            }
        };
        match os {
            "windows" => {
                if n.ends_with(".exe") {
                    score += 10;
                } else if n.contains("windows") && (n.ends_with(".zip") || n.ends_with(".exe")) {
                    score += 6;
                } else {
                    continue;
                }
            }
            "linux" => {
                if n.ends_with(".appimage") {
                    score += 10;
                } else if n.contains("linux") && !n.ends_with(".deb") {
                    score += 6;
                } else {
                    continue;
                }
            }
            "macos" => {
                if n.contains("darwin") || n.contains("macos") {
                    score += 6;
                } else {
                    continue;
                }
                if n.ends_with(".dmg") {
                    score -= 8;
                }
            }
            _ => continue,
        }
        if n.contains("aarch64") || n.contains("arm64") {
            if arch == "aarch64" {
                score += 2;
            } else {
                score -= 6;
            }
        }
        if n.contains("x86_64") || n.contains("amd64") || n.contains("x64") {
            if arch == "x86_64" {
                score += 1;
            }
        }
        if score > best.map(|(_, s)| s).unwrap_or(i32::MIN) {
            best = Some((a, score));
        }
    }
    best.filter(|(_, s)| *s > 0).map(|(a, _)| a)
}

pub fn pick_web_dist_asset(rel: &GithubRelease) -> Option<&GithubAsset> {
    rel.assets.iter().find(|a| {
        let n = a.name.to_ascii_lowercase();
        n.contains("web-dist") || n == "dist.zip" || n.contains("studio-web") && n.ends_with(".zip")
    })
}

pub fn update_can_apply(rel: &GithubRelease) -> bool {
    pick_binary_asset(rel, flavor_from_process()).is_some()
}

pub fn download_url_to(url: &str, dest: &Path) -> Result<(), String> {
    if let Some(dir) = dest.parent() {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let resp = ureq::get(url)
        .set("User-Agent", crate::info::USER_AGENT)
        .set("Accept", "application/octet-stream")
        .call()
        .map_err(|e| e.to_string())?;
    let mut reader = resp.into_reader();
    let mut out = File::create(dest).map_err(|e| e.to_string())?;
    io::copy(&mut reader, &mut out).map_err(|e| e.to_string())?;
    out.flush().map_err(|e| e.to_string())?;
    Ok(())
}

fn sidecar(exe: &Path, extra: &str) -> PathBuf {
    let mut s = exe.as_os_str().to_os_string();
    s.push(extra);
    PathBuf::from(s)
}

pub fn cleanup_stale_update_files() {
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    let _ = fs::remove_file(sidecar(&exe, ".bak"));
    let _ = fs::remove_file(sidecar(&exe, ".new"));
}

fn updates_dir() -> PathBuf {
    let d = app_data_directory().join("updates");
    let _ = fs::create_dir_all(&d);
    d
}

fn extract_zip(zip_path: &Path, dest: &Path) -> Result<(), String> {
    let f = File::open(zip_path).map_err(|e| e.to_string())?;
    let mut archive = zip::ZipArchive::new(f).map_err(|e| e.to_string())?;
    fs::create_dir_all(dest).map_err(|e| e.to_string())?;
    for i in 0..archive.len() {
        let mut file = archive.by_index(i).map_err(|e| e.to_string())?;
        let name = file.mangled_name();
        let out = dest.join(&name);
        if !out.starts_with(dest) {
            continue;
        }
        if file.is_dir() {
            fs::create_dir_all(&out).map_err(|e| e.to_string())?;
            continue;
        }
        if let Some(p) = out.parent() {
            fs::create_dir_all(p).map_err(|e| e.to_string())?;
        }
        let mut w = File::create(&out).map_err(|e| e.to_string())?;
        io::copy(&mut file, &mut w).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Download the matching GitHub asset and spawn a helper that replaces this
/// process after it exits. Caller must then exit.
pub fn stage_update_and_relaunch() -> Result<ApplyPlan, String> {
    let rel = latest_github_release()?;
    if !remote_is_newer(&rel.tag, VERSION) {
        return Err(format!("Already current ({VERSION})."));
    }
    let flavor = flavor_from_process();
    let asset = pick_binary_asset(&rel, flavor).ok_or_else(|| {
        format!(
            "Release {} has no portable {} build for this OS. Open the GitHub release and install it yourself.",
            rel.tag,
            match flavor {
                UpdateFlavor::Desktop => "desktop",
                UpdateFlavor::Server => "server",
            }
        )
    })?;
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let staged = sidecar(&exe, ".new");
    download_url_to(&asset.url, &staged)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut p = fs::metadata(&staged)
            .map_err(|e| e.to_string())?
            .permissions();
        p.set_mode(0o755);
        fs::set_permissions(&staged, p).map_err(|e| e.to_string())?;
    }
    let mut dist_next = PathBuf::new();
    if flavor == UpdateFlavor::Server {
        if let Some(web) = pick_web_dist_asset(&rel) {
            let zip_path = updates_dir().join("studio-web-dist.zip");
            download_url_to(&web.url, &zip_path)?;
            let ui = std::env::var("STUDIO_UI_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|_| exe.parent().unwrap_or(Path::new(".")).join("dist"));
            dist_next = ui.with_file_name(format!(
                "{}.next",
                ui.file_name()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "dist".into())
            ));
            let _ = fs::remove_dir_all(&dist_next);
            extract_zip(&zip_path, &dist_next)?;
            if !dist_next.join("index.html").is_file() && dist_next.join("dist").join("index.html").is_file()
            {
                dist_next = dist_next.join("dist");
            }
        }
    }
    let tag = rel.tag.clone();
    let asset_name = asset.name.clone();
    spawn_relaunch_helper(&exe, &staged, dist_next.as_path())?;
    Ok(ApplyPlan {
        tag,
        asset_name,
        relaunching: true,
    })
}

fn spawn_relaunch_helper(exe: &Path, staged: &Path, dist_next: &Path) -> Result<(), String> {
    let pid = std::process::id();
    let dir = updates_dir();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args_path = dir.join("relaunch-args.json");
    fs::write(&args_path, serde_json::to_string(&args).unwrap_or_else(|_| "[]".into()))
        .map_err(|e| e.to_string())?;
    #[cfg(windows)]
    {
        let script = dir.join("relaunch.ps1");
        let body = r#"
param($WaitPid, $Exe, $New, $DistNext, $DistDir, $ArgsFile)
Wait-Process -Id $WaitPid -ErrorAction SilentlyContinue
Start-Sleep -Seconds 1
if ($New -and (Test-Path -LiteralPath $New)) {
  if (Test-Path -LiteralPath $Exe) {
    Move-Item -LiteralPath $Exe -Destination ($Exe + '.bak') -Force -ErrorAction SilentlyContinue
  }
  Move-Item -LiteralPath $New -Destination $Exe -Force
}
if ($DistNext -and $DistDir -and (Test-Path -LiteralPath $DistNext)) {
  if (Test-Path -LiteralPath $DistDir) {
    Remove-Item -LiteralPath $DistDir -Recurse -Force -ErrorAction SilentlyContinue
  }
  Move-Item -LiteralPath $DistNext -Destination $DistDir -Force
}
$wd = Split-Path -Parent $Exe
$extra = @()
if ($ArgsFile -and (Test-Path -LiteralPath $ArgsFile)) {
  $extra = @(Get-Content -LiteralPath $ArgsFile -Raw | ConvertFrom-Json)
}
if ($extra.Count -gt 0) {
  Start-Process -FilePath $Exe -WorkingDirectory $wd -ArgumentList $extra
} else {
  Start-Process -FilePath $Exe -WorkingDirectory $wd
}
"#;
        fs::write(&script, body.trim_start()).map_err(|e| e.to_string())?;
        let dist_dir = std::env::var("STUDIO_UI_DIR").unwrap_or_default();
        let mut cmd = Command::new("powershell");
        cmd.args([
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-WindowStyle",
            "Hidden",
            "-File",
            script.to_str().ok_or("script path")?,
            "-WaitPid",
            &pid.to_string(),
            "-Exe",
            exe.to_str().ok_or("exe path")?,
            "-New",
            staged.to_str().ok_or("staged path")?,
            "-DistNext",
            dist_next.to_str().unwrap_or(""),
            "-DistDir",
            &dist_dir,
            "-ArgsFile",
            args_path.to_str().unwrap_or(""),
        ])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
        {
            use std::os::windows::process::CommandExt;
            const DETACHED_PROCESS: u32 = 0x0000_0008;
            const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
            cmd.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
        }
        cmd.spawn().map_err(|e| e.to_string())?;
        return Ok(());
    }
    #[cfg(not(windows))]
    {
        let script = dir.join("relaunch.sh");
        let body = r#"#!/bin/sh
waitpid="$1"
exe="$2"
new="$3"
distnext="$4"
distdir="$5"
argsfile="$6"
while kill -0 "$waitpid" 2>/dev/null; do sleep 1; done
sleep 1
if [ -n "$new" ] && [ -f "$new" ]; then
  mv -f "$new" "$exe"
  chmod +x "$exe" 2>/dev/null || true
fi
if [ -n "$distnext" ] && [ -n "$distdir" ] && [ -d "$distnext" ]; then
  rm -rf "$distdir"
  mv -f "$distnext" "$distdir"
fi
cd "$(dirname "$exe")" || true
if [ -n "$argsfile" ] && [ -f "$argsfile" ] && command -v python3 >/dev/null 2>&1; then
  eval "set -- $(python3 -c 'import json,sys,shlex; a=json.load(open(sys.argv[1])); print(" ".join(shlex.quote(x) for x in a))' "$argsfile")"
  exec "$exe" "$@"
fi
exec "$exe"
"#;
        fs::write(&script, body).map_err(|e| e.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut p = fs::metadata(&script)
                .map_err(|e| e.to_string())?
                .permissions();
            p.set_mode(0o755);
            fs::set_permissions(&script, p).map_err(|e| e.to_string())?;
        }
        let dist_dir = std::env::var("STUDIO_UI_DIR").unwrap_or_default();
        let mut cmd = Command::new(&script);
        cmd.args([
            pid.to_string(),
            exe.display().to_string(),
            staged.display().to_string(),
            dist_next.display().to_string(),
            dist_dir,
            args_path.display().to_string(),
        ])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            cmd.process_group(0);
        }
        cmd.spawn().map_err(|e| e.to_string())?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rel(names: &[&str]) -> GithubRelease {
        GithubRelease {
            tag: "v3.1.0".into(),
            html_url: "https://example".into(),
            body: None,
            assets: names
                .iter()
                .map(|n| GithubAsset {
                    name: (*n).into(),
                    url: format!("https://example/{n}"),
                    size: 1,
                })
                .collect(),
        }
    }

    #[test]
    fn picks_windows_desktop_exe() {
        if std::env::consts::OS != "windows" {
            return;
        }
        let r = rel(&[
            "studio-server.exe",
            "epg-monster-studio.exe",
            "studio-web-dist.zip",
        ]);
        let a = pick_binary_asset(&r, UpdateFlavor::Desktop).unwrap();
        assert_eq!(a.name, "epg-monster-studio.exe");
        let s = pick_binary_asset(&r, UpdateFlavor::Server).unwrap();
        assert_eq!(s.name, "studio-server.exe");
        assert!(pick_web_dist_asset(&r).is_some());
    }

    #[test]
    fn skips_deb_for_portable_linux() {
        if std::env::consts::OS != "linux" {
            return;
        }
        let r = rel(&["foo_amd64.deb", "epg-monster-studio_amd64.AppImage"]);
        let a = pick_binary_asset(&r, UpdateFlavor::Desktop).unwrap();
        assert!(a.name.to_ascii_lowercase().contains("appimage"));
    }
}
