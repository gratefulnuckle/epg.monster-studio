// SPDX-License-Identifier: GPL-3.0-or-later

use sha2::{Digest, Sha256};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use crate::paths::app_data_directory;
use crate::USER_AGENT;

const MANIFEST_JSON: &str = include_str!("../resources/tools-manifest.json");

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolSpec {
    pub id: String,
    pub label: String,
    pub url: String,
    pub sha256: String,
    pub exe_names: Vec<String>,
    pub dest_subdir: String,
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

pub fn find_app_root(hint: &Path) -> PathBuf {
    let mut p = hint.to_path_buf();
    for _ in 0..8 {
        if p.join("src-tauri").is_dir() && p.join("package.json").is_file() {
            return p;
        }
        if p.join("tools").join("ffmpeg").is_dir() || p.join("tools").join("mpv").is_dir() {
            return p;
        }
        if !p.pop() {
            break;
        }
    }
    hint.to_path_buf()
}

pub fn tools_root(app_root: &Path) -> PathBuf {
    app_root.join("tools")
}

pub fn tool_present(app_root: &Path, spec: &ToolSpec) -> bool {
    if spec.exe_names.is_empty() {
        return false;
    }
    let dest = tools_root(app_root).join(&spec.dest_subdir);
    spec.exe_names.iter().all(|n| dest.join(n).is_file())
}

pub fn missing_tools(app_root: &Path) -> Result<Vec<ToolSpec>, String> {
    let man = load_manifest()?;
    Ok(man.tools.into_iter().filter(|t| !tool_present(app_root, t)).collect())
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
    let cache = app_data_directory().join("tool-cache");
    fs::create_dir_all(&cache).map_err(|e| e.to_string())?;
    let total = missing.len() as f64;
    for (i, spec) in missing.iter().enumerate() {
        if spec.sha256.trim().is_empty() || spec.sha256.eq_ignore_ascii_case("PENDING") {
            return Err(format!("No SHA-256 pinned for {}.", spec.id));
        }
        let zip_path = cache.join(format!("{}.zip", spec.id));
        progress(ToolBootstrapProgress {
            tool_id: spec.id.clone(),
            message: format!("Downloading {}…", spec.label),
            percent: (i as f64 / total) * 100.0,
        });
        download(&spec.url, &zip_path, &spec.id, i, missing.len(), &mut progress)?;
        let actual = hash_file(&zip_path)?;
        if !actual.eq_ignore_ascii_case(&spec.sha256) {
            let _ = fs::remove_file(&zip_path);
            return Err(format!(
                "SHA-256 mismatch for {}. Expected {}, got {}.",
                spec.id, spec.sha256, actual
            ));
        }
        let dest = tools_root(app_root).join(&spec.dest_subdir);
        extract_exes(&zip_path, &dest, &spec.exe_names)?;
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
        .timeout(std::time::Duration::from_secs(15 * 60))
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
    }

    #[test]
    fn load_manifest_is_embedded() {
        let man = load_manifest().unwrap();
        assert!(man.tools.iter().any(|t| t.id == "ffmpeg"));
        assert!(man.tools.iter().any(|t| t.id == "mpv"));
        assert!(man.tools.iter().all(|t| t.url.starts_with("https://")));
        assert!(man.tools.iter().all(|t| {
            t.sha256.len() == 64 && t.sha256.chars().all(|c| c.is_ascii_hexdigit())
        }));
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
