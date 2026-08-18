// SPDX-License-Identifier: GPL-3.0-or-later

use std::fs;
use std::path::{Path, PathBuf};

use crate::info::PRODUCT_ID;

pub const DATABASE_FILE_NAME: &str = "epg.monster-studio.db";

pub fn local_data_root() -> PathBuf {
    dirs::data_local_dir().unwrap_or_else(|| {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".local")
            .join("share")
    })
}

pub fn app_data_directory() -> PathBuf {
    let root = local_data_root();
    let path = root.join(PRODUCT_ID);
    migrate_legacy(&root, &path);
    let _ = fs::create_dir_all(&path);
    let _ = fs::create_dir_all(path.join("cache"));
    path
}

pub fn database_path() -> PathBuf {
    let dir = app_data_directory();
    let neu = dir.join(DATABASE_FILE_NAME);
    let leftover = dir.join("iptv-studio.db");
    if leftover.exists() && !neu.exists() {
        let _ = fs::rename(&leftover, &neu);
    }
    if neu.exists() {
        neu
    } else if leftover.exists() {
        leftover
    } else {
        neu
    }
}

pub fn audit_process_db_path() -> PathBuf {
    app_data_directory().join("auditprocess.db")
}

pub fn offline_slates_directory() -> PathBuf {
    let dir = app_data_directory().join("offline-slates");
    let _ = fs::create_dir_all(&dir);
    dir
}

fn migrate_legacy(local_app_data: &Path, dest: &Path) {
    let legacy = local_app_data.join("iptv-studio");
    let dest_db = dest.join(DATABASE_FILE_NAME);
    let dest_old = dest.join("iptv-studio.db");
    let src_db = legacy.join("iptv-studio.db");
    if dest_db.exists() || dest_old.exists() || !src_db.exists() {
        return;
    }
    let _ = copy_dir(&legacy, dest);
    if dest_old.exists() && !dest_db.exists() {
        let _ = fs::rename(&dest_old, &dest_db);
    }
}

fn copy_dir(src: &Path, dest: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dest)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let to = dest.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &to)?;
        } else if !to.exists() {
            fs::copy(entry.path(), to)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn product_folder_is_epg_monster_studio() {
        let p = app_data_directory();
        assert!(p.ends_with(PRODUCT_ID));
    }
}
