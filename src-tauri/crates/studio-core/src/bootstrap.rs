// SPDX-License-Identifier: GPL-3.0-or-later

use sha2::{Digest, Sha256};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use crate::paths::app_data_directory;
use crate::tools::find_on_path;
use crate::USER_AGENT;

const MANIFEST_JSON: &str = include_str!("../resources/tools-manifest.json");

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolTarget {
    pub host: String,
    #[serde(default = "download_source")]
    pub source: String,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub sha256: String,
    #[serde(default)]
    pub exe_names: Vec<String>,
    #[serde(default)]
    pub dest_subdir: String,
    #[serde(default)]
    pub install_hint: String,
    #[serde(default)]
    pub archive: String,
    #[serde(default)]
    pub optional: bool,
}

fn download_source() -> String {
    "download".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolSpec {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub sha256: String,
    #[serde(default)]
    pub exe_names: Vec<String>,
    #[serde(default)]
    pub dest_subdir: String,
    #[serde(default)]
    pub targets: Vec<ToolTarget>,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub install_hint: String,
    #[serde(default)]
    pub archive: String,
    #[serde(default)]
    pub optional: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolsManifest {
    pub schema: String,
    pub version: i32,
    pub tools: Vec<ToolSpec>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolBootstrapProgress {
    pub tool_id: String,
    pub message: String,
    pub percent: f64,
}

pub fn parse_manifest(json: &str) -> Result<ToolsManifest, String> {
    let doc: ToolsManifest = serde_json::from_str(json).map_err(|e| e.to_string())?;
    if doc.tools.is_empty() {
        return Err("tools-manifest.json is empty.".into());
    }
    Ok(doc)
}

pub fn load_manifest() -> Result<ToolsManifest, String> {
    parse_manifest(MANIFEST_JSON)
}

pub fn current_host() -> String {
    let os = if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else {
        "linux"
    };
    let arch = if cfg!(target_arch = "x86_64") {
        "x86_64"
    } else if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else {
        std::env::consts::ARCH
    };
    format!("{os}-{arch}")
}

pub fn resolve_tool(spec: &ToolSpec, host: &str) -> Option<ToolSpec> {
    if let Some(t) = spec.targets.iter().find(|t| t.host == host) {
        return Some(flatten_target(spec, t));
    }
    // Legacy unflattened pin: Windows zip only. Never feed a gyan/mpv Windows
    // archive to Linux or macOS.
    if spec.targets.is_empty() && !spec.url.trim().is_empty() && host.starts_with("windows-") {
        let mut out = spec.clone();
        out.source = if out.source.trim().is_empty() {
            "download".into()
        } else {
            out.source
        };
        return Some(out);
    }
    None
}

fn flatten_target(spec: &ToolSpec, t: &ToolTarget) -> ToolSpec {
    ToolSpec {
        id: spec.id.clone(),
        label: spec.label.clone(),
        url: if t.url.trim().is_empty() {
            spec.url.clone()
        } else {
            t.url.clone()
        },
        sha256: if t.sha256.trim().is_empty() {
            spec.sha256.clone()
        } else {
            t.sha256.clone()
        },
        exe_names: if t.exe_names.is_empty() {
            spec.exe_names.clone()
        } else {
            t.exe_names.clone()
        },
        dest_subdir: if t.dest_subdir.trim().is_empty() {
            spec.dest_subdir.clone()
        } else {
            t.dest_subdir.clone()
        },
        targets: Vec::new(),
        source: if t.source.trim().is_empty() {
            "download".into()
        } else {
            t.source.clone()
        },
        install_hint: t.install_hint.clone(),
        archive: if t.archive.trim().is_empty() {
            spec.archive.clone()
        } else {
            t.archive.clone()
        },
        optional: t.optional || spec.optional,
    }
}

pub fn find_app_root(hint: &Path) -> PathBuf {
    let mut p = hint.to_path_buf();
    let mut tools_hit = None;
    for _ in 0..8 {
        if p.join("src-tauri").is_dir() && p.join("package.json").is_file() {
            return p;
        }
        let ffmpeg_exe = p
            .join("tools")
            .join("ffmpeg")
            .join(crate::tools::tool_file_name("ffmpeg"));
        let mpv_exe = p
            .join("tools")
            .join("mpv")
            .join(crate::tools::tool_file_name("mpv"));
        if tools_hit.is_none() && (ffmpeg_exe.is_file() || mpv_exe.is_file()) {
            tools_hit = Some(p.clone());
        }
        if !p.pop() {
            break;
        }
    }
    tools_hit.unwrap_or_else(|| hint.to_path_buf())
}

pub fn tools_root(app_root: &Path) -> PathBuf {
    app_root.join("tools")
}

fn dest_dir(app_root: &Path, spec: &ToolSpec) -> PathBuf {
    let sub = if spec.dest_subdir.trim().is_empty() {
        spec.id.as_str()
    } else {
        spec.dest_subdir.as_str()
    };
    tools_root(app_root).join(sub)
}

fn bundled_present(app_root: &Path, spec: &ToolSpec) -> bool {
    if spec.exe_names.is_empty() {
        return false;
    }
    let dest = dest_dir(app_root, spec);
    spec.exe_names.iter().all(|n| {
        dest.join(n).is_file() || dest.join("bin").join(n).is_file()
    })
}

fn system_present(spec: &ToolSpec) -> bool {
    if spec.exe_names.is_empty() {
        return false;
    }
    spec.exe_names.iter().all(|n| find_on_path(n).is_some())
}

pub fn is_system_source(spec: &ToolSpec) -> bool {
    spec.source.eq_ignore_ascii_case("system")
}

pub fn tool_present(app_root: &Path, spec: &ToolSpec) -> bool {
    if bundled_present(app_root, spec) {
        return true;
    }
    if is_system_source(spec) {
        return system_present(spec);
    }
    false
}

pub fn missing_tools(app_root: &Path) -> Result<Vec<ToolSpec>, String> {
    let man = load_manifest()?;
    let host = current_host();
    let mut out = Vec::new();
    for spec in man.tools {
        let resolved = resolve_tool(&spec, &host).ok_or_else(|| {
            format!(
                "No {} pin for {host}. Install ffmpeg and mpv, or add a host target in tools-manifest.json.",
                spec.id
            )
        })?;
        if resolved.optional {
            continue;
        }
        if !tool_present(app_root, &resolved) {
            out.push(resolved);
        }
    }
    Ok(out)
}

pub fn hash_file(path: &Path) -> Result<String, String> {
    let mut f = fs::File::open(path).map_err(|e| e.to_string())?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 128 * 1024];
    loop {
        let n = f.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(format!("{:X}", hasher.finalize()))
}

pub fn extract_zip_tree(zip_path: &Path, dest_dir: &Path) -> Result<(), String> {
    fs::create_dir_all(dest_dir).map_err(|e| e.to_string())?;
    let file = fs::File::open(zip_path).map_err(|e| e.to_string())?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
        let name = entry.name().replace('\\', "/");
        if name.contains("..") {
            continue;
        }
        let rel = name.trim_start_matches('/');
        if rel.is_empty() {
            continue;
        }
        let out = dest_dir.join(rel);
        if entry.is_dir() || name.ends_with('/') {
            fs::create_dir_all(&out).map_err(|e| e.to_string())?;
            continue;
        }
        if let Some(parent) = out.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let mut dest = fs::File::create(&out).map_err(|e| e.to_string())?;
        std::io::copy(&mut entry, &mut dest).map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn extract_exes(zip_path: &Path, dest_dir: &Path, exe_names: &[String]) -> Result<(), String> {
    fs::create_dir_all(dest_dir).map_err(|e| e.to_string())?;
    let file = fs::File::open(zip_path).map_err(|e| e.to_string())?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
    for name in exe_names {
        let mut found = None;
        for i in 0..archive.len() {
            let e = archive.by_index(i).map_err(|e| e.to_string())?;
            let file_name = Path::new(e.name())
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("");
            if file_name.eq_ignore_ascii_case(name) && e.size() > 0 {
                found = Some(i);
                break;
            }
        }
        let i = found.ok_or_else(|| format!("Zip is missing {name}."))?;
        let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
        let dest = dest_dir.join(name);
        let mut out = fs::File::create(&dest).map_err(|e| e.to_string())?;
        std::io::copy(&mut entry, &mut out).map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn ensure<F>(app_root: &Path, mut progress: F) -> Result<(), String>
where
    F: FnMut(ToolBootstrapProgress),
{
    let missing = missing_tools(app_root)?;
    if missing.is_empty() {
        return Ok(());
    }

    let system: Vec<&ToolSpec> = missing
        .iter()
        .filter(|t| is_system_source(t) || t.url.trim().is_empty())
        .collect();
    if !system.is_empty() {
        let hints: Vec<String> = system
            .iter()
            .map(|t| {
                if t.install_hint.trim().is_empty() {
                    format!("Install {} and put it on PATH.", t.label)
                } else {
                    t.install_hint.clone()
                }
            })
            .collect();
        return Err(hints.join(" "));
    }

    let cache = app_data_directory().join("tool-cache");
    fs::create_dir_all(&cache).map_err(|e| e.to_string())?;
    let total = missing.len() as f64;
    for (i, spec) in missing.iter().enumerate() {
        if spec.sha256.trim().is_empty() || spec.sha256.eq_ignore_ascii_case("PENDING") {
            return Err(format!("No SHA-256 pinned for {}.", spec.id));
        }
        if !spec.url.starts_with("https://") {
            return Err(format!("Refusing non-HTTPS tool URL for {}.", spec.id));
        }
        let pack_path = cache.join(format!("{}.zip", spec.id));
        progress(ToolBootstrapProgress {
            tool_id: spec.id.clone(),
            message: format!("Downloading {}…", spec.label),
            percent: (i as f64 / total) * 100.0,
        });
        download(&spec.url, &pack_path, &spec.id, i, missing.len(), &mut progress)?;
        let actual = hash_file(&pack_path)?;
        if !actual.eq_ignore_ascii_case(&spec.sha256) {
            let _ = fs::remove_file(&pack_path);
            return Err(format!(
                "SHA-256 mismatch for {}. Expected {}, got {}.",
                spec.id, spec.sha256, actual
            ));
        }
        let dest = dest_dir(app_root, spec);
        extract_exes(&pack_path, &dest, &spec.exe_names)?;
        progress(ToolBootstrapProgress {
            tool_id: spec.id.clone(),
            message: format!("{} ready", spec.label),
            percent: ((i + 1) as f64 / total) * 100.0,
        });
    }
    Ok(())
}

fn download<F>(
    url: &str,
    dest: &Path,
    tool_id: &str,
    index: usize,
    total: usize,
    progress: &mut F,
) -> Result<(), String>
where
    F: FnMut(ToolBootstrapProgress),
{
    let resp = ureq::get(url)
        .set("User-Agent", USER_AGENT)
        .timeout(std::time::Duration::from_secs(60 * 60))
        .call()
        .map_err(|e| e.to_string())?;
    let total_bytes = resp
        .header("Content-Length")
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0);
    let mut input = resp.into_reader();
    let mut output = fs::File::create(dest).map_err(|e| e.to_string())?;
    let mut buf = [0u8; 128 * 1024];
    let mut read = 0u64;
    loop {
        let n = input.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        output.write_all(&buf[..n]).map_err(|e| e.to_string())?;
        read += n as u64;
        if total_bytes == 0 {
            continue;
        }
        let slice = read as f64 / total_bytes as f64;
        let overall = ((index as f64 + slice) / total as f64) * 100.0;
        progress(ToolBootstrapProgress {
            tool_id: tool_id.into(),
            message: format!("Downloading {tool_id}… {}%", (slice * 100.0) as i32),
            percent: overall,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_manifest_reads_pinned_tools() {
        let json = r#"{
              "schema": "epg.monster.tools",
              "version": 1,
              "tools": [
                {
                  "id": "ffmpeg",
                  "label": "ffmpeg",
                  "url": "https://example.com/ffmpeg.zip",
                  "sha256": "abc",
                  "exeNames": [ "ffmpeg.exe", "ffprobe.exe" ],
                  "destSubdir": "ffmpeg"
                }
              ]
            }"#;
        let man = parse_manifest(json).unwrap();
        assert_eq!(man.schema, "epg.monster.tools");
        assert_eq!(man.tools.len(), 1);
        assert_eq!(man.tools[0].id, "ffmpeg");
        assert!(man.tools[0].exe_names.iter().any(|n| n == "ffprobe.exe"));
        let win = resolve_tool(&man.tools[0], "windows-x86_64").unwrap();
        assert_eq!(win.source, "download");
        assert!(resolve_tool(&man.tools[0], "linux-x86_64").is_none());
    }

    #[test]
    fn find_app_root_prefers_package_json_over_tools_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let project = tmp.path().join("project");
        std::fs::create_dir_all(project.join("src-tauri")).unwrap();
        std::fs::write(project.join("package.json"), b"{}").unwrap();
        let nested = project.join("src-tauri").join("nested");
        std::fs::create_dir_all(&nested).unwrap();
        assert_eq!(find_app_root(&nested), project);
    }

    #[test]
    fn load_manifest_is_embedded() {
        let man = load_manifest().unwrap();
        assert!(man.tools.iter().any(|t| t.id == "ffmpeg"));
        assert!(man.tools.iter().any(|t| t.id == "mpv" && t.optional));
        assert_eq!(man.tools.len(), 2);
        let hosts = [
            "windows-x86_64",
            "linux-x86_64",
            "linux-aarch64",
            "macos-x86_64",
            "macos-aarch64",
        ];
        for spec in &man.tools {
            for host in hosts {
                let resolved = resolve_tool(spec, host)
                    .unwrap_or_else(|| panic!("{} missing pin for {host}", spec.id));
                assert!(!resolved.exe_names.is_empty());
                if resolved.optional {
                    continue;
                }
                assert_eq!(resolved.source, "system");
                assert!(!resolved.install_hint.is_empty());
                assert!(resolved.url.is_empty() || resolved.sha256.is_empty());
            }
        }
    }

    #[test]
    fn embedded_manifest_resolves_current_host() {
        let man = load_manifest().unwrap();
        let host = current_host();
        assert!(host.contains('-'));
        for id in ["ffmpeg", "mpv"] {
            let spec = man.tools.iter().find(|t| t.id == id).unwrap();
            let resolved = resolve_tool(spec, &host).expect("host pin");
            assert!(!resolved.exe_names.is_empty());
        }
    }

    #[test]
    fn system_tool_on_path_counts_as_present() {
        let tmp = tempfile::tempdir().unwrap();
        let bin = tmp.path().join("bin");
        fs::create_dir_all(&bin).unwrap();
        let name = if cfg!(windows) {
            "studio-fake-ffmpeg.exe"
        } else {
            "studio-fake-ffmpeg"
        };
        let exe = bin.join(name);
        fs::write(&exe, b"x").unwrap();
        let old = std::env::var_os("PATH");
        let mut paths = vec![bin.clone()];
        if let Some(ref p) = old {
            paths.extend(std::env::split_paths(p));
        }
        std::env::set_var("PATH", std::env::join_paths(&paths).unwrap());
        let spec = ToolSpec {
            id: "ffmpeg".into(),
            label: "ffmpeg".into(),
            url: String::new(),
            sha256: String::new(),
            exe_names: vec![name.into()],
            dest_subdir: "ffmpeg".into(),
            targets: Vec::new(),
            source: "system".into(),
            install_hint: "apt install ffmpeg".into(),
            archive: String::new(),
            optional: false,
        };
        let present = tool_present(tmp.path(), &spec);
        match old {
            Some(p) => std::env::set_var("PATH", p),
            None => std::env::remove_var("PATH"),
        }
        assert!(present);
    }

    #[test]
    fn extract_exes_pulls_named_binaries_from_nested_zip() {
        let tmp = tempfile::tempdir().unwrap();
        let zip_path = tmp.path().join("pack.zip");
        {
            let f = fs::File::create(&zip_path).unwrap();
            let mut zw = zip::ZipWriter::new(f);
            let opts = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Stored);
            zw.start_file("ffmpeg-9.0.1-essentials_build/bin/ffmpeg.exe", opts)
                .unwrap();
            zw.write_all(b"ffmpeg").unwrap();
            zw.start_file("ffmpeg-9.0.1-essentials_build/bin/ffprobe.exe", opts)
                .unwrap();
            zw.write_all(b"ffprobe").unwrap();
            zw.finish().unwrap();
        }
        let dest = tmp.path().join("out");
        extract_exes(
            &zip_path,
            &dest,
            &["ffmpeg.exe".into(), "ffprobe.exe".into()],
        )
        .unwrap();
        assert_eq!(fs::read_to_string(dest.join("ffmpeg.exe")).unwrap(), "ffmpeg");
        assert_eq!(fs::read_to_string(dest.join("ffprobe.exe")).unwrap(), "ffprobe");
    }

    #[test]
    fn extract_zip_tree_keeps_prefix_layout() {
        let tmp = tempfile::tempdir().unwrap();
        let zip_path = tmp.path().join("tree.zip");
        {
            let f = fs::File::create(&zip_path).unwrap();
            let mut zw = zip::ZipWriter::new(f);
            let opts = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Stored);
            zw.start_file("bin/tool.exe", opts).unwrap();
            zw.write_all(b"launch").unwrap();
            zw.start_file("lib/plugins/plugin.dll", opts).unwrap();
            zw.write_all(b"plug").unwrap();
            zw.finish().unwrap();
        }
        let dest = tmp.path().join("prefix");
        extract_zip_tree(&zip_path, &dest).unwrap();
        assert_eq!(
            fs::read_to_string(dest.join("bin").join("tool.exe")).unwrap(),
            "launch"
        );
        assert_eq!(
            fs::read_to_string(dest.join("lib").join("plugins").join("plugin.dll")).unwrap(),
            "plug"
        );
    }

    #[test]
    fn hash_file_is_stable() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("t.txt");
        fs::write(&path, "hello").unwrap();
        let h = hash_file(&path).unwrap();
        assert_eq!(h.len(), 64);
        assert_eq!(h, hash_file(&path).unwrap());
        assert_eq!(
            h,
            "2CF24DBA5FB0A30E26E83B2AC5B9E29E1B161E5C1FA7425E73043362938B9824"
        );
    }
}
