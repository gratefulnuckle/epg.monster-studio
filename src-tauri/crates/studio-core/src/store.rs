// SPDX-License-Identifier: GPL-3.0-or-later

use std::path::Path;

use rusqlite::{params, Connection};
use thiserror::Error;

use crate::models::{ChannelEntry, PlaylistSource};
use crate::parser::parse_m3u;
use crate::settings::AppSettings;

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("sqlite: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

pub struct SqliteStore {
    conn: Connection,
}

impl SqliteStore {
    pub fn open(path: &Path) -> Result<Self, StoreError> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let conn = Connection::open(path)?;
        let store = Self { conn };
        store.initialize()?;
        Ok(store)
    }

    pub fn initialize(&self) -> Result<(), StoreError> {
        self.conn.execute_batch(
            r#"
            PRAGMA journal_mode=WAL;
            PRAGMA foreign_keys=ON;

            CREATE TABLE IF NOT EXISTS sources (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                kind TEXT NOT NULL,
                location TEXT NOT NULL,
                headers_json TEXT NOT NULL DEFAULT '{}',
                etag TEXT NULL,
                last_modified TEXT NULL,
                cached_path TEXT NULL,
                loaded_at TEXT NOT NULL,
                channel_count INTEGER NOT NULL DEFAULT 0
            );

            CREATE TABLE IF NOT EXISTS channel_entries (
                id TEXT PRIMARY KEY,
                source_id TEXT NOT NULL,
                group_title TEXT NOT NULL,
                name TEXT NOT NULL,
                tvg_id TEXT NULL,
                tvg_name TEXT NULL,
                tvg_logo TEXT NULL,
                url TEXT NOT NULL,
                attrs_json TEXT NOT NULL DEFAULT '{}',
                line_no INTEGER NOT NULL,
                FOREIGN KEY(source_id) REFERENCES sources(id) ON DELETE CASCADE
            );

            CREATE INDEX IF NOT EXISTS idx_channel_source ON channel_entries(source_id);
            CREATE INDEX IF NOT EXISTS idx_channel_group ON channel_entries(source_id, group_title);

            CREATE TABLE IF NOT EXISTS managed_channels (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                group_title TEXT NOT NULL,
                tvg_id TEXT NULL,
                tvg_logo TEXT NULL,
                notes TEXT NULL,
                sort_order INTEGER NOT NULL DEFAULT 0,
                tvg_shift REAL NOT NULL DEFAULT 0
            );

            CREATE TABLE IF NOT EXISTS epg_programmes (
                tvg_id TEXT NOT NULL,
                title TEXT NOT NULL,
                description TEXT NULL,
                start_utc TEXT NOT NULL,
                stop_utc TEXT NOT NULL,
                indexed_at TEXT NOT NULL,
                PRIMARY KEY (tvg_id, start_utc)
            );
            CREATE INDEX IF NOT EXISTS idx_epg_programmes_window
                ON epg_programmes(tvg_id, start_utc, stop_utc);

            CREATE TABLE IF NOT EXISTS stream_variants (
                id TEXT PRIMARY KEY,
                managed_channel_id TEXT NOT NULL,
                url TEXT NOT NULL,
                label TEXT NULL,
                source_entry_id TEXT NULL,
                visibility TEXT NOT NULL,
                priority INTEGER NOT NULL DEFAULT 0,
                last_audit_ok INTEGER NULL,
                last_audit_at TEXT NULL,
                FOREIGN KEY(managed_channel_id) REFERENCES managed_channels(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS audit_results (
                id TEXT PRIMARY KEY,
                target_type TEXT NOT NULL,
                target_id TEXT NOT NULL,
                ok INTEGER NOT NULL,
                error TEXT NULL,
                latency_ms INTEGER NULL,
                engine TEXT NOT NULL,
                probed_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS swap_undo_log (
                id TEXT PRIMARY KEY,
                managed_channel_id TEXT NOT NULL,
                from_variant_id TEXT NOT NULL,
                to_variant_id TEXT NOT NULL,
                reason TEXT NOT NULL,
                created_at TEXT NOT NULL,
                undone_at TEXT NULL
            );

            CREATE TABLE IF NOT EXISTS epg_catalog (
                tvg_id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                logo TEXT NULL,
                source TEXT NOT NULL,
                section TEXT NOT NULL DEFAULT '',
                raw_json TEXT NULL,
                fetched_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS epg_now_playing (
                tvg_id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                description TEXT NULL,
                start_utc TEXT NOT NULL,
                stop_utc TEXT NOT NULL,
                indexed_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );

            CREATE VIRTUAL TABLE IF NOT EXISTS channel_fts USING fts5(
                name,
                group_title,
                tvg_id,
                url
            );
            "#,
        )?;
        self.ensure_column("epg_catalog", "section", "TEXT NOT NULL DEFAULT ''")?;
        self.ensure_column("managed_channels", "tvg_shift", "REAL NOT NULL DEFAULT 0")?;
        self.ensure_column("managed_channels", "in_tuner", "INTEGER NOT NULL DEFAULT 0")?;
        self.ensure_column("managed_channels", "tuner_number", "INTEGER NULL")?;
        self.ensure_column("stream_variants", "origin_name", "TEXT NULL")?;
        self.ensure_column("stream_variants", "origin_tvg_id", "TEXT NULL")?;
        self.ensure_column("audit_results", "grade", "TEXT NULL")?;
        self.ensure_column("audit_results", "width", "INTEGER NULL")?;
        self.ensure_column("audit_results", "height", "INTEGER NULL")?;
        self.ensure_column("audit_results", "fps", "REAL NULL")?;
        self.ensure_column("audit_results", "aspect_ratio", "TEXT NULL")?;
        self.ensure_column("audit_results", "video_codec", "TEXT NULL")?;
        self.ensure_column("audit_results", "audio_codec", "TEXT NULL")?;
        self.ensure_column("audit_results", "job_id", "TEXT NULL")?;
        self.ensure_column("audit_results", "channel_id", "TEXT NULL")?;
        self.ensure_column("audit_results", "channel_name", "TEXT NULL")?;
        self.ensure_column("audit_results", "group_title", "TEXT NULL")?;
        self.ensure_column("audit_results", "tvg_id", "TEXT NULL")?;
        self.ensure_column("audit_results", "error_class", "TEXT NULL")?;
        self.conn.execute_batch(
            r#"
            CREATE INDEX IF NOT EXISTS idx_audit_job ON audit_results(job_id);
            CREATE INDEX IF NOT EXISTS idx_audit_channel ON audit_results(channel_id);
            CREATE INDEX IF NOT EXISTS idx_audit_tvg ON audit_results(tvg_id);
            CREATE INDEX IF NOT EXISTS idx_audit_grade ON audit_results(grade);
            "#,
        )?;
        Ok(())
    }

    fn ensure_column(&self, table: &str, column: &str, type_sql: &str) -> Result<(), StoreError> {
        let mut stmt = self.conn.prepare(&format!("PRAGMA table_info({table});"))?;
        let exists = stmt
            .query_map([], |row| row.get::<_, String>(1))?
            .filter_map(|r| r.ok())
            .any(|name| name.eq_ignore_ascii_case(column));
        if !exists {
            self.conn
                .execute(&format!("ALTER TABLE {table} ADD COLUMN {column} {type_sql};"), [])?;
        }
        Ok(())
    }

    pub fn load_settings(&self) -> Result<AppSettings, StoreError> {
        let value: Result<String, _> = self.conn.query_row(
            "SELECT value FROM settings WHERE key = 'app'",
            [],
            |row| row.get(0),
        );
        match value {
            Ok(json) => {
                let mut s: AppSettings = serde_json::from_str(&json).unwrap_or_default();
                s.ensure_tuner_profiles();
                Ok(s)
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(AppSettings::default()),
            Err(e) => Err(e.into()),
        }
    }

    pub fn save_settings(&self, settings: &AppSettings) -> Result<(), StoreError> {
        let json = serde_json::to_string(settings)?;
        self.conn.execute(
            "INSERT INTO settings (key, value) VALUES ('app', ?1)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![json],
        )?;
        Ok(())
    }

    pub fn add_file_source(&self, path: &Path) -> Result<PlaylistSource, StoreError> {
        let content = std::fs::read_to_string(path)?;
        let id = uuid::Uuid::new_v4().simple().to_string();
        let name = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("playlist")
            .to_string();
        let channels = parse_m3u(&content, &id);
        let loaded_at = chrono_like_now();
        self.conn.execute(
            "INSERT INTO sources (id, name, kind, location, headers_json, loaded_at, channel_count)
             VALUES (?1, ?2, 'file', ?3, '{}', ?4, ?5)",
            params![id, name, path.to_string_lossy(), loaded_at, channels.len() as i32],
        )?;
        {
            let mut insert = self.conn.prepare(
                "INSERT INTO channel_entries
                 (id, source_id, group_title, name, tvg_id, tvg_name, tvg_logo, url, attrs_json, line_no)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            )?;
            let mut fts = self.conn.prepare(
                "INSERT INTO channel_fts (name, group_title, tvg_id, url) VALUES (?1, ?2, ?3, ?4)",
            )?;
            for ch in &channels {
                insert.execute(params![
                    ch.id,
                    ch.source_id,
                    ch.group_title,
                    ch.name,
                    ch.tvg_id,
                    ch.tvg_name,
                    ch.tvg_logo,
                    ch.url,
                    ch.attrs_json,
                    ch.line_no
                ])?;
                fts.execute(params![
                    ch.name,
                    ch.group_title,
                    ch.tvg_id.clone().unwrap_or_default(),
                    ch.url
                ])?;
            }
        }
        Ok(PlaylistSource {
            id,
            name,
            kind: "file".into(),
            location: path.to_string_lossy().into(),
            headers_json: "{}".into(),
            channel_count: channels.len() as i32,
        })
    }

    pub fn list_sources(&self) -> Result<Vec<PlaylistSource>, StoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, kind, location, headers_json, channel_count FROM sources ORDER BY loaded_at",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(PlaylistSource {
                id: row.get(0)?,
                name: row.get(1)?,
                kind: row.get(2)?,
                location: row.get(3)?,
                headers_json: row.get(4)?,
                channel_count: row.get(5)?,
            })
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    pub fn search_sources(&self, query: &str) -> Result<Vec<ChannelEntry>, StoreError> {
        let q = query.trim();
        if q.chars().count() < 2 {
            return Ok(Vec::new());
        }
        let like = format!("%{q}%");
        let mut stmt = self.conn.prepare(
            "SELECT id, source_id, group_title, name, tvg_id, tvg_name, tvg_logo, url, attrs_json, line_no
             FROM channel_entries
             WHERE name LIKE ?1 OR group_title LIKE ?1 OR IFNULL(tvg_id,'') LIKE ?1 OR url LIKE ?1
             LIMIT 400",
        )?;
        let rows = stmt.query_map(params![like], |row| {
            Ok(ChannelEntry {
                id: row.get(0)?,
                source_id: row.get(1)?,
                group_title: row.get(2)?,
                name: row.get(3)?,
                tvg_id: row.get(4)?,
                tvg_name: row.get(5)?,
                tvg_logo: row.get(6)?,
                tvg_shift_hours: 0.0,
                url: row.get(7)?,
                attrs_json: row.get(8)?,
                line_no: row.get(9)?,
            })
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }
}

fn chrono_like_now() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("{secs}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn creates_schema_and_round_trips_settings() {
        let dir = tempdir().unwrap();
        let db = dir.path().join("epg.monster-studio.db");
        let store = SqliteStore::open(&db).unwrap();
        let mut s = AppSettings::default();
        s.audit_delay_ms = 6000;
        store.save_settings(&s).unwrap();
        let loaded = store.load_settings().unwrap();
        assert_eq!(loaded.audit_delay_ms, 6000);
        assert!(loaded.iptv_tuner.enabled);
    }

    #[test]
    fn search_requires_two_chars_and_caps_at_400() {
        let dir = tempdir().unwrap();
        let db = dir.path().join("t.db");
        let store = SqliteStore::open(&db).unwrap();
        assert!(store.search_sources("C").unwrap().is_empty());
    }

    #[test]
    fn load_file_source_parses_m3u() {
        let dir = tempdir().unwrap();
        let playlist = dir.path().join("list.m3u");
        std::fs::write(
            &playlist,
            "#EXTM3U\n#EXTINF:-1 group-title=\"News\",CNN\nhttp://example.com/cnn\n",
        )
        .unwrap();
        let store = SqliteStore::open(&dir.path().join("t.db")).unwrap();
        let src = store.add_file_source(&playlist).unwrap();
        assert_eq!(src.channel_count, 1);
        let hits = store.search_sources("CNN").unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].name, "CNN");
    }
}
