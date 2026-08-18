// SPDX-License-Identifier: GPL-3.0-or-later

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use crate::info::PRODUCT_ID;

pub const DATABASE_FILE_NAME: &str = "epg.monster-studio.db";

static APP_DIR: OnceLock<PathBuf> = OnceLock::new();

/// Directory that contains the executable (or the `.app` / AppImage).
pub fn configure_app_dir(dir: PathBuf) {
    let _ = APP_DIR.set(dir);
}

pub fn configure_from_current_exe() {
    if let Ok(img) = env::var("APPIMAGE") {
        if let Some(parent) = app_dir_from_appimage(Path::new(&img)) {
            configure_app_dir(parent);
            return;
        }
    }
    configure_app_dir(exe_install_dir());
}

pub fn app_dir_from_appimage(appimage: &Path) -> Option<PathBuf> {
    let parent = appimage.parent()?;
    if parent.as_os_str().is_empty() {
        None
    } else {
        Some(parent.to_path_buf())
    }
}

pub fn exe_install_dir() -> PathBuf {
    let exe = env::current_exe().unwrap_or_else(|_| PathBuf::from("."));
    install_dir_from_exe(&exe)
}

/// Directory that contains the executable, or the folder that contains a `.app` bundle.
pub fn install_dir_from_exe(exe: &Path) -> PathBuf {
    let mut dir = exe
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    // …/Foo.app/Contents/MacOS → folder that contains Foo.app
    if dir.ends_with("MacOS") {
        if let Some(contents) = dir.parent() {
            if contents.file_name().and_then(|s| s.to_str()) == Some("Contents") {
                if let Some(app) = contents.parent() {
                    if app.extension().and_then(|s| s.to_str()) == Some("app") {
                        if let Some(outer) = app.parent() {
                            dir = outer.to_path_buf();
                        }
                    }
                }
            }
        }
    }
    dir
}

pub fn macos_user_data_parent(home: &Path) -> PathBuf {
    home.join("Library").join("Application Support")
}

pub fn linux_user_data_parent(home: &Path, xdg_data_home: Option<&str>) -> PathBuf {
    match xdg_data_home.map(str::trim).filter(|s| !s.is_empty()) {
        Some(xdg) => PathBuf::from(xdg),
        None => home.join(".local").join("share"),
    }
}

pub fn local_data_root() -> PathBuf {
    if cfg!(target_os = "macos") {
        return macos_user_data_parent(
            &dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")),
        );
    }
    if cfg!(target_os = "linux") {
        return linux_user_data_parent(
            &dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")),
            env::var("XDG_DATA_HOME").ok().as_deref(),
        );
    }
    dirs::data_local_dir().unwrap_or_else(|| {
        linux_user_data_parent(&dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")), None)
    })
}

pub fn user_data_directory() -> PathBuf {
    local_data_root().join(PRODUCT_ID)
}

/// Portable install (writable app dir) → `{app}/data`. System install → OS user data folder.
/// Does not search for or copy a C# `%LocalAppData%\epg.monster-studio` or `iptv-studio` tree.
pub fn app_data_directory() -> PathBuf {
    let app_dir = APP_DIR.get().cloned().unwrap_or_else(exe_install_dir);
    app_data_directory_for(&app_dir)
}

pub fn app_data_directory_for(app_dir: &Path) -> PathBuf {
    let path = if dir_is_user_writable(app_dir) {
        app_dir.join("data")
    } else {
        user_data_directory()
    };
    let _ = fs::create_dir_all(&path);
    let _ = fs::create_dir_all(path.join("cache"));
    path
}

fn dir_is_user_writable(dir: &Path) -> bool {
    if !dir.is_dir() {
        return false;
    }
    let probe = dir.join(".studio-write-probe");
    match fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&probe)
    {
        Ok(_) => {
            let _ = fs::remove_file(&probe);
            true
        }
        Err(_) => false,
    }
}

pub fn database_file_in(dir: &Path) -> PathBuf {
    dir.join(DATABASE_FILE_NAME)
}

pub fn database_path() -> PathBuf {
    database_file_in(&app_data_directory())
}

pub fn audit_process_db_path() -> PathBuf {
    app_data_directory().join("auditprocess.db")
}

pub fn logs_directory() -> PathBuf {
    let dir = app_data_directory().join("logs");
    let _ = fs::create_dir_all(&dir);
    dir
}

pub fn crashes_directory() -> PathBuf {
    let dir = logs_directory().join("crashes");
    let _ = fs::create_dir_all(&dir);
    dir
}

pub fn current_log_path() -> PathBuf {
    let now = time::OffsetDateTime::now_utc();
    logs_directory().join(format!(
        "{:04}-{:02}-{:02}.log",
        now.year(),
        now.month() as u8,
        now.day()
    ))
}

pub fn offline_slates_directory() -> PathBuf {
    let dir = app_data_directory().join("offline-slates");
    let _ = fs::create_dir_all(&dir);
    dir
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_folder_is_epg_monster_studio() {
        assert!(user_data_directory().ends_with(PRODUCT_ID));
    }

    #[test]
    fn writable_install_dir_uses_sidecar_data() {
        let tmp = tempfile::tempdir().unwrap();
        let data = app_data_directory_for(tmp.path());
        assert_eq!(data, tmp.path().join("data"));
        assert!(data.is_dir());
        assert!(data.join("cache").is_dir());
    }

    #[test]
    fn missing_install_dir_uses_user_folder() {
        let missing = PathBuf::from("/this/does/not/exist/studio-app-dir");
        let data = app_data_directory_for(&missing);
        assert!(data.ends_with(PRODUCT_ID));
    }

    #[test]
    fn file_as_install_dir_uses_user_folder() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("not-a-dir");
        fs::write(&file, b"x").unwrap();
        let data = app_data_directory_for(&file);
        assert!(data.ends_with(PRODUCT_ID));
    }

    #[test]
    fn database_file_in_does_not_rename_iptv_studio_db() {
        let tmp = tempfile::tempdir().unwrap();
        let leftover = tmp.path().join("iptv-studio.db");
        fs::write(&leftover, b"legacy").unwrap();
        let path = database_file_in(tmp.path());
        assert_eq!(path, tmp.path().join(DATABASE_FILE_NAME));
        assert!(!path.exists());
        assert_eq!(fs::read(&leftover).unwrap(), b"legacy");
    }

    #[test]
    fn writable_app_dir_does_not_copy_iptv_studio_tree() {
        let tmp = tempfile::tempdir().unwrap();
        let app = tmp.path().join("app");
        fs::create_dir(&app).unwrap();
        let legacy = tmp.path().join("iptv-studio");
        fs::create_dir_all(&legacy).unwrap();
        fs::write(legacy.join("iptv-studio.db"), b"old").unwrap();
        let data = app_data_directory_for(&app);
        assert_eq!(data, app.join("data"));
        assert!(!data.join("iptv-studio.db").exists());
        assert!(!data.join(DATABASE_FILE_NAME).exists());
        assert_eq!(fs::read(legacy.join("iptv-studio.db")).unwrap(), b"old");
    }

    #[test]
    fn macos_user_data_parent_is_application_support_once() {
        let home = Path::new("/Users/grateful");
        assert_eq!(
            macos_user_data_parent(home),
            PathBuf::from("/Users/grateful/Library/Application Support")
        );
    }

    #[test]
    fn linux_user_data_parent_prefers_xdg() {
        let home = Path::new("/home/grateful");
        assert_eq!(
            linux_user_data_parent(home, Some("/var/data")),
            PathBuf::from("/var/data")
        );
        assert_eq!(
            linux_user_data_parent(home, Some("  ")),
            PathBuf::from("/home/grateful/.local/share")
        );
        assert_eq!(
            linux_user_data_parent(home, None),
            PathBuf::from("/home/grateful/.local/share")
        );
    }

    #[test]
    fn install_dir_from_exe_uses_folder_that_contains_app_bundle() {
        let tmp = tempfile::tempdir().unwrap();
        let macos = tmp
            .path()
            .join("epg.monster studio.app")
            .join("Contents")
            .join("MacOS");
        fs::create_dir_all(&macos).unwrap();
        let exe = macos.join("epg-monster-studio");
        assert_eq!(install_dir_from_exe(&exe), tmp.path());
    }

    #[test]
    fn install_dir_from_exe_is_parent_of_plain_binary() {
        let tmp = tempfile::tempdir().unwrap();
        let exe = tmp.path().join("epg-monster-studio.exe");
        assert_eq!(install_dir_from_exe(&exe), tmp.path());
    }

    #[test]
    fn app_dir_from_appimage_is_parent() {
        let img = Path::new("/home/grateful/Apps/epg.monster studio.AppImage");
        assert_eq!(
            app_dir_from_appimage(img),
            Some(PathBuf::from("/home/grateful/Apps"))
        );
    }
}
