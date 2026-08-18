// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::{HashMap, HashSet};
use std::path::Path;

use rusqlite::{params, Connection};
use thiserror::Error;

use crate::epg::clean_epg_token;
use crate::models::{
    CatalogEntry, ChannelEntry, EpgSuggestion, ManagedChannel, NowPlaying, PlaylistSource,
    StreamVariant,
};
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

fn tvg_lookup_ids(tvg_id: &str) -> Vec<String> {
    let raw = tvg_id.trim().to_string();
    let cleaned = clean_epg_token(&raw);
    let mut out = Vec::new();
    for cand in [
        raw,
        cleaned.clone(),
        cleaned.replace(' ', "."),
        cleaned.replace('.', " "),
    ] {
        if !cand.is_empty() && !out.iter().any(|s| s == &cand) {
            out.push(cand);
        }
    }
    out
}

fn parse_rfc3339(s: &str) -> Option<time::OffsetDateTime> {
    time::OffsetDateTime::parse(s, &time::format_description::well_known::Rfc3339).ok()
}

fn format_short_time(dt: time::OffsetDateTime) -> String {
    dt.format(&time::macros::format_description!(
        "[hour repr:12 padding:none]:[minute] [period case:upper]"
    ))
    .unwrap_or_else(|_| dt.to_string())
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

    pub fn load_epg_cache_meta(&self) -> crate::epg::EpgCacheMeta {
        let value: Result<String, _> = self.conn.query_row(
            "SELECT value FROM settings WHERE key = 'epg_cache'",
            [],
            |row| row.get(0),
        );
        match value {
            Ok(json) => serde_json::from_str(&json).unwrap_or_default(),
            _ => crate::epg::EpgCacheMeta::default(),
        }
    }

    pub fn save_epg_cache_meta(&self, meta: &crate::epg::EpgCacheMeta) -> Result<(), StoreError> {
        let json = serde_json::to_string(meta)?;
        self.conn.execute(
            "INSERT INTO settings (key, value) VALUES ('epg_cache', ?1)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![json],
        )?;
        Ok(())
    }

    pub fn touch_epg_cache_meta(
        &self,
        fetched: bool,
        indexed: bool,
        etag: Option<&str>,
        last_modified: Option<&str>,
    ) -> Result<(), StoreError> {
        let mut meta = self.load_epg_cache_meta();
        let now = crate::audit::now_iso();
        if fetched {
            meta.last_fetch_at = Some(now.clone());
        }
        if indexed {
            meta.last_index_at = Some(now);
        }
        if let Some(e) = etag.map(str::trim).filter(|s| !s.is_empty()) {
            meta.etag = Some(e.to_string());
        }
        if let Some(lm) = last_modified.map(str::trim).filter(|s| !s.is_empty()) {
            meta.last_modified = Some(lm.to_string());
        }
        self.save_epg_cache_meta(&meta)
    }

    pub fn add_file_source(&self, path: &Path) -> Result<PlaylistSource, StoreError> {
        self.add_file_source_named(path, None)
    }

    pub fn add_file_source_named(
        &self,
        path: &Path,
        name: Option<&str>,
    ) -> Result<PlaylistSource, StoreError> {
        let content = std::fs::read_to_string(path)?;
        let id = uuid::Uuid::new_v4().simple().to_string();
        let name = name
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .unwrap_or_else(|| {
                path.file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("playlist")
                    .to_string()
            });
        self.insert_parsed_source(
            &id,
            &name,
            "file",
            &path.to_string_lossy(),
            "{}",
            None,
            &content,
        )
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
        self.search_channels_by_name_or_tvg(query, 400)
    }

    /// Add Sources search: all loaded sources, name or tvg-id, min 2 chars, cap 400.
    pub fn search_channels_by_name_or_tvg(
        &self,
        query: &str,
        limit: i32,
    ) -> Result<Vec<ChannelEntry>, StoreError> {
        let q = query.trim();
        if q.chars().count() < 2 {
            return Ok(Vec::new());
        }
        let limit = limit.clamp(1, 2000);
        let like = format!("%{}%", q.replace('%', "\\%").replace('_', "\\_"));
        let mut stmt = self.conn.prepare(
            "SELECT id, source_id, group_title, name, tvg_id, tvg_name, tvg_logo, url, attrs_json, line_no
             FROM channel_entries
             WHERE name LIKE ?1 ESCAPE '\\'
                OR IFNULL(tvg_id,'') LIKE ?1 ESCAPE '\\'
             ORDER BY name COLLATE NOCASE, group_title COLLATE NOCASE
             LIMIT ?2",
        )?;
        Ok(read_channels(&mut stmt, params![like, limit])?)
    }

    pub fn groups_with_counts(&self, source_id: &str) -> Result<Vec<(String, i32)>, StoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT group_title, COUNT(*) AS c
             FROM channel_entries
             WHERE source_id = ?1
             GROUP BY group_title
             ORDER BY group_title COLLATE NOCASE",
        )?;
        let rows = stmt.query_map(params![source_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i32>(1)?))
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    pub fn channels_by_group(
        &self,
        source_id: &str,
        group_title: &str,
    ) -> Result<Vec<ChannelEntry>, StoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, source_id, group_title, name, tvg_id, tvg_name, tvg_logo, url, attrs_json, line_no
             FROM channel_entries
             WHERE source_id = ?1 AND group_title = ?2
             ORDER BY line_no",
        )?;
        Ok(read_channels(&mut stmt, params![source_id, group_title])?)
    }

    pub fn remove_source(&self, source_id: &str) -> Result<(), StoreError> {
        self.conn
            .execute("DELETE FROM sources WHERE id = ?1", params![source_id])?;
        self.rebuild_channel_fts()?;
        Ok(())
    }

    fn rebuild_channel_fts(&self) -> Result<(), StoreError> {
        self.conn.execute_batch(
            r#"
            DELETE FROM channel_fts;
            INSERT INTO channel_fts(rowid, name, group_title, tvg_id, url)
            SELECT rowid, name, group_title, IFNULL(tvg_id,''), url FROM channel_entries;
            "#,
        )?;
        Ok(())
    }

    pub fn refresh_source(&self, source_id: &str, cache_dir: &Path) -> Result<PlaylistSource, StoreError> {
        let src = self
            .list_sources()?
            .into_iter()
            .find(|s| s.id == source_id)
            .ok_or_else(|| StoreError::Io(std::io::Error::other("source not found")))?;
        let content = if src.kind == "url" {
            std::fs::create_dir_all(cache_dir)?;
            let headers: std::collections::BTreeMap<String, String> =
                serde_json::from_str(&src.headers_json).unwrap_or_default();
            let mut req = ureq::get(&src.location);
            for (k, v) in &headers {
                req = req.set(k, v);
            }
            let body = req
                .call()
                .map_err(|e| StoreError::Io(std::io::Error::other(e.to_string())))?
                .into_string()
                .map_err(StoreError::Io)?;
            let cache_path = cache_dir.join(format!("{}.m3u", src.id));
            std::fs::write(&cache_path, &body)?;
            body
        } else {
            std::fs::read_to_string(&src.location)?
        };
        self.conn
            .execute("DELETE FROM channel_entries WHERE source_id = ?1", params![source_id])?;
        let channels = parse_m3u(&content, source_id);
        self.insert_channels(&channels)?;
        self.rebuild_channel_fts()?;
        let loaded_at = chrono_like_now();
        self.conn.execute(
            "UPDATE sources SET channel_count = ?1, loaded_at = ?2 WHERE id = ?3",
            params![channels.len() as i32, loaded_at, source_id],
        )?;
        Ok(PlaylistSource {
            channel_count: channels.len() as i32,
            ..src
        })
    }

    pub fn managed_count(&self) -> Result<i32, StoreError> {
        let n: i32 = self
            .conn
            .query_row("SELECT COUNT(*) FROM managed_channels", [], |row| row.get(0))?;
        Ok(n)
    }

    pub fn add_url_source(
        &self,
        url: &str,
        name: Option<&str>,
        headers: &std::collections::BTreeMap<String, String>,
        cache_dir: &Path,
    ) -> Result<PlaylistSource, StoreError> {
        std::fs::create_dir_all(cache_dir)?;
        let mut req = ureq::get(url);
        for (k, v) in headers {
            req = req.set(k, v);
        }
        let body = req
            .call()
            .map_err(|e| StoreError::Io(std::io::Error::other(e.to_string())))?
            .into_string()
            .map_err(StoreError::Io)?;
        let id = uuid::Uuid::new_v4().simple().to_string();
        let cache_path = cache_dir.join(format!("{id}.m3u"));
        std::fs::write(&cache_path, &body)?;
        let display = name
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or("URL")
            .to_string();
        self.insert_parsed_source(
            &id,
            &display,
            "url",
            url,
            &serde_json::to_string(headers)?,
            Some(cache_path.to_string_lossy().as_ref()),
            &body,
        )
    }

    fn insert_parsed_source(
        &self,
        id: &str,
        name: &str,
        kind: &str,
        location: &str,
        headers_json: &str,
        cached_path: Option<&str>,
        content: &str,
    ) -> Result<PlaylistSource, StoreError> {
        let channels = parse_m3u(content, id);
        let loaded_at = chrono_like_now();
        self.conn.execute(
            "INSERT INTO sources (id, name, kind, location, headers_json, cached_path, loaded_at, channel_count)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                id,
                name,
                kind,
                location,
                headers_json,
                cached_path,
                loaded_at,
                channels.len() as i32
            ],
        )?;
        self.insert_channels(&channels)?;
        Ok(PlaylistSource {
            id: id.to_string(),
            name: name.to_string(),
            kind: kind.to_string(),
            location: location.to_string(),
            headers_json: headers_json.to_string(),
            channel_count: channels.len() as i32,
        })
    }

    fn insert_channels(&self, channels: &[ChannelEntry]) -> Result<(), StoreError> {
        let mut insert = self.conn.prepare(
            "INSERT INTO channel_entries
             (id, source_id, group_title, name, tvg_id, tvg_name, tvg_logo, url, attrs_json, line_no)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        )?;
        let mut fts = self.conn.prepare(
            "INSERT INTO channel_fts (name, group_title, tvg_id, url) VALUES (?1, ?2, ?3, ?4)",
        )?;
        for ch in channels {
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
        Ok(())
    }

    pub fn managed_groups(&self) -> Result<Vec<(String, i32)>, StoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT group_title, COUNT(*) FROM managed_channels
             GROUP BY group_title ORDER BY group_title COLLATE NOCASE",
        )?;
        let rows = stmt.query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, i32>(1)?)))?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    pub fn list_managed(&self, group: Option<&str>) -> Result<Vec<ManagedChannel>, StoreError> {
        let mut sql = String::from(
            "SELECT id, name, group_title, tvg_id, tvg_logo, notes, sort_order,
                    IFNULL(tvg_shift,0), IFNULL(in_tuner,0), tuner_number
             FROM managed_channels",
        );
        if group.is_some() {
            sql.push_str(" WHERE group_title = ?1");
        }
        sql.push_str(" ORDER BY sort_order, name COLLATE NOCASE");
        let mut stmt = self.conn.prepare(&sql)?;
        let rows: Vec<ManagedChannel> = if let Some(g) = group {
            stmt.query_map(params![g], read_managed)?
                .filter_map(|r| r.ok())
                .collect()
        } else {
            stmt.query_map([], read_managed)?
                .filter_map(|r| r.ok())
                .collect()
        };
        let mut out = rows;
        self.hydrate_managed(&mut out)?;
        Ok(out)
    }

    /// One variants query + one catalog set instead of N+1 per channel.
    fn hydrate_managed(&self, channels: &mut [ManagedChannel]) -> Result<(), StoreError> {
        if channels.is_empty() {
            return Ok(());
        }
        let want: HashSet<&str> = channels.iter().map(|c| c.id.as_str()).collect();
        let mut by_id: HashMap<String, Vec<StreamVariant>> = HashMap::new();
        if want.len() == 1 {
            let id = channels[0].id.clone();
            by_id.insert(id.clone(), self.get_variants(&id)?);
        } else {
            for v in self.list_all_variants()? {
                if want.contains(v.managed_channel_id.as_str()) {
                    by_id.entry(v.managed_channel_id.clone()).or_default().push(v);
                }
            }
        }
        let known = self.catalog_tvg_lower()?;
        for ch in channels.iter_mut() {
            ch.variants = by_id.remove(&ch.id).unwrap_or_default();
            ch.has_epg_match = ch
                .tvg_id
                .as_deref()
                .map(|id| !id.is_empty() && known.contains(&id.to_ascii_lowercase()))
                .unwrap_or(false);
        }
        Ok(())
    }

    fn catalog_tvg_lower(&self) -> Result<HashSet<String>, StoreError> {
        let mut stmt = self.conn.prepare("SELECT tvg_id FROM epg_catalog")?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
        Ok(rows
            .filter_map(|r| r.ok())
            .map(|s| s.to_ascii_lowercase())
            .collect())
    }

    pub fn get_managed(&self, id: &str) -> Result<Option<ManagedChannel>, StoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, group_title, tvg_id, tvg_logo, notes, sort_order,
                    IFNULL(tvg_shift,0), IFNULL(in_tuner,0), tuner_number
             FROM managed_channels WHERE id = ?1",
        )?;
        let mut rows = stmt.query_map(params![id], read_managed)?;
        let Some(Ok(mut ch)) = rows.next() else {
            return Ok(None);
        };
        ch.variants = self.get_variants(&ch.id)?;
        ch.has_epg_match = self.is_known_tvg_id(ch.tvg_id.as_deref());
        Ok(Some(ch))
    }

    pub fn upsert_managed(&self, ch: &ManagedChannel) -> Result<(), StoreError> {
        self.conn.execute(
            "INSERT INTO managed_channels (id, name, group_title, tvg_id, tvg_logo, notes, sort_order, tvg_shift, in_tuner, tuner_number)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
             ON CONFLICT(id) DO UPDATE SET
                name=excluded.name, group_title=excluded.group_title, tvg_id=excluded.tvg_id,
                tvg_logo=excluded.tvg_logo, notes=excluded.notes, sort_order=excluded.sort_order,
                tvg_shift=excluded.tvg_shift, in_tuner=excluded.in_tuner, tuner_number=excluded.tuner_number",
            params![
                ch.id,
                ch.name,
                ch.group_title,
                ch.tvg_id,
                ch.tvg_logo,
                ch.notes,
                ch.sort_order,
                ch.tvg_shift_hours,
                ch.in_tuner as i32,
                ch.tuner_number
            ],
        )?;
        Ok(())
    }

    /// Persist editor Save: blank group → Ungrouped; non-empty primary URL writes the visible variant.
    pub fn save_managed_channel(
        &self,
        ch: &ManagedChannel,
        primary_url: Option<&str>,
    ) -> Result<(), StoreError> {
        let mut ch = ch.clone();
        let group = ch.group_title.trim();
        ch.group_title = if group.is_empty() {
            "Ungrouped".into()
        } else {
            group.to_string()
        };
        self.upsert_managed(&ch)?;
        let Some(url) = primary_url.map(str::trim).filter(|s| !s.is_empty()) else {
            return Ok(());
        };
        self.set_visible_url(&ch.id, url)
    }

    fn set_visible_url(&self, managed_id: &str, url: &str) -> Result<(), StoreError> {
        let mut variants = self.get_variants(managed_id)?;
        if let Some(i) = variants
            .iter()
            .position(|v| v.visibility == "visible")
            .or_else(|| variants.first().map(|_| 0))
        {
            if variants[i].url != url {
                variants[i].url = url.to_string();
                self.upsert_variant(&variants[i])?;
            }
            return Ok(());
        }
        self.upsert_variant(&StreamVariant {
            id: uuid::Uuid::new_v4().simple().to_string(),
            managed_channel_id: managed_id.to_string(),
            url: url.to_string(),
            label: Some("primary".into()),
            source_entry_id: None,
            origin_name: None,
            origin_tvg_id: None,
            visibility: "visible".into(),
            priority: 0,
            last_audit_ok: None,
            last_audit_at: None,
        })
    }

    pub fn clear_managed(&self) -> Result<(), StoreError> {
        self.conn.execute_batch("DELETE FROM stream_variants; DELETE FROM managed_channels;")?;
        Ok(())
    }

    /// Import a curated m3u. Identity: primary URL, else group+name. Never tvg-id alone.
    pub fn import_curated(&self, content: &str, replace: bool, label: &str) -> Result<(i32, i32), StoreError> {
        if replace {
            self.clear_managed()?;
        }
        let entries = parse_m3u(content, "import");
        let existing = self.list_managed(None)?;
        let mut by_url = std::collections::HashMap::<String, String>::new();
        let mut by_gn = std::collections::HashMap::<String, String>::new();
        for c in &existing {
            if let Some(u) = c
                .variants
                .iter()
                .find(|v| v.visibility == "visible")
                .or_else(|| c.variants.first())
                .map(|v| v.url.trim().to_string())
                .filter(|u| !u.is_empty())
            {
                by_url.entry(u.to_lowercase()).or_insert(c.id.clone());
            }
            let key = format!(
                "{}|{}",
                c.group_title.trim().to_lowercase(),
                c.name.trim().to_lowercase()
            );
            by_gn.entry(key).or_insert(c.id.clone());
        }
        let mut added = 0i32;
        let mut skipped = 0i32;
        for (i, e) in entries.iter().enumerate() {
            let group = if e.group_title.trim().is_empty() {
                "Ungrouped".to_string()
            } else {
                e.group_title.trim().to_string()
            };
            let name = if e.name.trim().is_empty() {
                e.url.trim().to_string()
            } else {
                e.name.trim().to_string()
            };
            let url = e.url.trim().to_string();
            let gn = format!("{}|{}", group.to_lowercase(), name.to_lowercase());
            if (!url.is_empty() && by_url.contains_key(&url.to_lowercase())) || by_gn.contains_key(&gn) {
                skipped += 1;
                continue;
            }
            let ch = ManagedChannel {
                id: uuid::Uuid::new_v4().simple().to_string(),
                name: name.clone(),
                group_title: group.clone(),
                tvg_id: e.tvg_id.clone(),
                tvg_logo: e.tvg_logo.clone(),
                notes: Some("Imported curated".into()),
                sort_order: if e.line_no > 0 { e.line_no } else { i as i32 + 1 },
                tvg_shift_hours: e.tvg_shift_hours,
                in_tuner: false,
                tuner_number: None,
                variants: vec![],
                has_epg_match: false,
            };
            self.upsert_managed(&ch)?;
            self.upsert_variant(&StreamVariant {
                id: uuid::Uuid::new_v4().simple().to_string(),
                managed_channel_id: ch.id.clone(),
                url,
                label: Some(label.to_string()),
                source_entry_id: None,
                origin_name: None,
                origin_tvg_id: None,
                visibility: "visible".into(),
                priority: 0,
                last_audit_ok: None,
                last_audit_at: None,
            })?;
            by_gn.insert(gn, ch.id);
            added += 1;
        }
        Ok((added, skipped))
    }

    pub fn delete_managed(&self, id: &str) -> Result<(), StoreError> {
        self.conn
            .execute("DELETE FROM managed_channels WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn rename_managed_group(&self, old: &str, new: &str) -> Result<i32, StoreError> {
        let old_name = old.trim();
        if old_name.is_empty() {
            return Ok(0);
        }
        let new_name = if new.trim().is_empty() {
            "Ungrouped"
        } else {
            new.trim()
        };
        let n = self.conn.execute(
            "UPDATE managed_channels SET group_title = ?1 WHERE lower(trim(group_title)) = lower(?2)",
            params![new_name, old_name],
        )?;
        Ok(n as i32)
    }

    pub fn get_variants(&self, managed_id: &str) -> Result<Vec<StreamVariant>, StoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, managed_channel_id, url, label, source_entry_id, visibility, priority,
                    origin_name, origin_tvg_id, last_audit_ok, last_audit_at
             FROM stream_variants WHERE managed_channel_id = ?1 ORDER BY priority, label",
        )?;
        let rows = stmt.query_map(params![managed_id], read_variant)?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    pub fn upsert_variant(&self, v: &StreamVariant) -> Result<(), StoreError> {
        self.conn.execute(
            "INSERT INTO stream_variants
             (id, managed_channel_id, url, label, source_entry_id, visibility, priority, origin_name, origin_tvg_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
             ON CONFLICT(id) DO UPDATE SET
                url=excluded.url, label=excluded.label, visibility=excluded.visibility,
                priority=excluded.priority, origin_name=excluded.origin_name, origin_tvg_id=excluded.origin_tvg_id",
            params![
                v.id,
                v.managed_channel_id,
                v.url,
                v.label,
                v.source_entry_id,
                v.visibility,
                v.priority,
                v.origin_name,
                v.origin_tvg_id
            ],
        )?;
        Ok(())
    }

    pub fn delete_variant(&self, id: &str) -> Result<(), StoreError> {
        self.conn
            .execute("DELETE FROM stream_variants WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn apply_variant_order(&self, managed_id: &str, ordered_ids: &[String]) -> Result<(), StoreError> {
        let tx = self.conn.unchecked_transaction()?;
        for (i, id) in ordered_ids.iter().enumerate() {
            tx.execute(
                "UPDATE stream_variants SET priority = ?1, visibility = ?2
                 WHERE id = ?3 AND managed_channel_id = ?4",
                params![
                    i as i32,
                    if i == 0 { "visible" } else { "hidden_backup" },
                    id,
                    managed_id
                ],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn add_stream(
        &self,
        managed_id: &str,
        url: &str,
        label: Option<&str>,
    ) -> Result<StreamVariant, StoreError> {
        let max: i32 = self.conn.query_row(
            "SELECT IFNULL(MAX(priority), -1) FROM stream_variants WHERE managed_channel_id = ?1",
            params![managed_id],
            |row| row.get(0),
        )?;
        let v = StreamVariant {
            id: uuid::Uuid::new_v4().simple().to_string(),
            managed_channel_id: managed_id.to_string(),
            url: url.trim().to_string(),
            label: label.map(|s| s.to_string()).filter(|s| !s.is_empty()),
            source_entry_id: None,
            origin_name: None,
            origin_tvg_id: None,
            visibility: if max < 0 { "visible" } else { "hidden_backup" }.into(),
            priority: max + 1,
            last_audit_ok: None,
            last_audit_at: None,
        };
        self.upsert_variant(&v)?;
        Ok(v)
    }

    pub fn headers_for_entry(&self, entry_id: &str) -> Vec<(String, String)> {
        let json: String = self
            .conn
            .query_row(
                "SELECT s.headers_json FROM channel_entries e
                 JOIN sources s ON s.id = e.source_id
                 WHERE e.id = ?1",
                params![entry_id],
                |r| r.get(0),
            )
            .unwrap_or_default();
        serde_json::from_str::<std::collections::BTreeMap<String, String>>(&json)
            .unwrap_or_default()
            .into_iter()
            .collect()
    }

    pub fn headers_for_channels(
        &self,
        channels: &[ManagedChannel],
    ) -> std::collections::HashMap<String, Vec<(String, String)>> {
        let mut map = std::collections::HashMap::new();
        for ch in channels {
            for v in &ch.variants {
                let Some(id) = v.source_entry_id.as_deref().filter(|s| !s.is_empty()) else {
                    continue;
                };
                let headers = self.headers_for_entry(id);
                if !headers.is_empty() {
                    map.insert(v.id.clone(), headers);
                }
            }
        }
        map
    }

    pub fn add_backup_from_entry(
        &self,
        managed_id: &str,
        entry_id: &str,
    ) -> Result<String, StoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, source_id, group_title, name, tvg_id, tvg_name, tvg_logo, url, attrs_json, line_no
             FROM channel_entries WHERE id = ?1",
        )?;
        let entry = read_channels(&mut stmt, params![entry_id])?
            .into_iter()
            .next()
            .ok_or_else(|| StoreError::Io(std::io::Error::other("source row not found")))?;
        let ch = self
            .get_managed(managed_id)?
            .ok_or_else(|| StoreError::Io(std::io::Error::other("channel not found")))?;
        if ch
            .variants
            .iter()
            .any(|v| v.url.eq_ignore_ascii_case(&entry.url))
        {
            return Err(StoreError::Io(std::io::Error::other(format!(
                "Already on {}",
                ch.name
            ))));
        }
        let label = if entry.name.trim().is_empty() {
            "backup"
        } else {
            entry.name.as_str()
        };
        let mut v = self.add_stream(managed_id, &entry.url, Some(label))?;
        v.source_entry_id = Some(entry.id);
        v.origin_name = Some(entry.name);
        v.origin_tvg_id = entry.tvg_id;
        self.upsert_variant(&v)?;
        Ok(ch.name)
    }

    pub fn add_from_source_entry(&self, entry_id: &str) -> Result<ManagedChannel, StoreError> {
        self.add_from_source_entry_labeled(entry_id, None)
    }

    pub fn add_from_source_entry_labeled(
        &self,
        entry_id: &str,
        source_label: Option<&str>,
    ) -> Result<ManagedChannel, StoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, source_id, group_title, name, tvg_id, tvg_name, tvg_logo, url, attrs_json, line_no
             FROM channel_entries WHERE id = ?1",
        )?;
        let entry = read_channels(&mut stmt, params![entry_id])?
            .into_iter()
            .next()
            .ok_or_else(|| StoreError::Io(std::io::Error::other("source row not found")))?;
        let group = if entry.group_title.trim().is_empty() {
            "Ungrouped".to_string()
        } else {
            entry.group_title.clone()
        };
        let ch = ManagedChannel {
            id: uuid::Uuid::new_v4().simple().to_string(),
            name: entry.name.clone(),
            group_title: group,
            tvg_id: entry.tvg_id.clone(),
            tvg_logo: entry.tvg_logo.clone(),
            notes: None,
            sort_order: entry.line_no,
            tvg_shift_hours: entry.tvg_shift_hours,
            in_tuner: false,
            tuner_number: None,
            variants: vec![],
            has_epg_match: false,
        };
        self.upsert_managed(&ch)?;
        let label = source_label
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or("primary");
        let v = StreamVariant {
            id: uuid::Uuid::new_v4().simple().to_string(),
            managed_channel_id: ch.id.clone(),
            url: entry.url,
            label: Some(label.into()),
            source_entry_id: Some(entry.id),
            origin_name: Some(entry.name),
            origin_tvg_id: entry.tvg_id,
            visibility: "visible".into(),
            priority: 0,
            last_audit_ok: None,
            last_audit_at: None,
        };
        self.upsert_variant(&v)?;
        Ok(self.get_managed(&ch.id)?.unwrap())
    }

    pub fn add_missing_from_source_entries(
        &self,
        entry_ids: &[String],
        source_label: Option<&str>,
    ) -> Result<(i32, i32), StoreError> {
        let existing = self.list_managed(None)?;
        let mut keys: HashSet<(String, String)> = existing
            .iter()
            .map(|c| {
                (
                    c.name.trim().to_ascii_lowercase(),
                    c.tvg_id
                        .as_deref()
                        .unwrap_or("")
                        .trim()
                        .to_ascii_lowercase(),
                )
            })
            .collect();
        let mut added = 0i32;
        let mut skipped = 0i32;
        for id in entry_ids {
            let ch = self.add_from_source_entry_labeled(id, source_label)?;
            let key = (
                ch.name.trim().to_ascii_lowercase(),
                ch.tvg_id
                    .as_deref()
                    .unwrap_or("")
                    .trim()
                    .to_ascii_lowercase(),
            );
            if keys.contains(&key) {
                self.delete_managed(&ch.id)?;
                skipped += 1;
                continue;
            }
            keys.insert(key);
            added += 1;
        }
        Ok((added, skipped))
    }

    pub fn is_known_tvg_id(&self, tvg_id: Option<&str>) -> bool {
        let Some(id) = tvg_id.map(str::trim).filter(|s| !s.is_empty()) else {
            return false;
        };
        self.conn
            .query_row(
                "SELECT 1 FROM epg_catalog WHERE tvg_id = ?1 COLLATE NOCASE LIMIT 1",
                params![id],
                |_| Ok(()),
            )
            .is_ok()
    }

    pub fn suggest_tvg(&self, query: &str) -> Result<Vec<EpgSuggestion>, StoreError> {
        let q = query.trim();
        if q.is_empty() {
            return Ok(Vec::new());
        }
        let like = format!("%{q}%");
        let mut stmt = self.conn.prepare(
            "SELECT tvg_id, name FROM epg_catalog
             WHERE tvg_id LIKE ?1 COLLATE NOCASE OR name LIKE ?1 COLLATE NOCASE
             LIMIT 200",
        )?;
        let mut rows: Vec<(String, String)> = stmt
            .query_map(params![like], |row| Ok((row.get(0)?, row.get(1)?)))?
            .filter_map(|r| r.ok())
            .collect();
        let ql = q.to_lowercase();
        rows.sort_by(|a, b| {
            let a_pref = a.0.to_lowercase().starts_with(&ql);
            let b_pref = b.0.to_lowercase().starts_with(&ql);
            b_pref.cmp(&a_pref).then_with(|| a.1.to_lowercase().cmp(&b.1.to_lowercase()))
        });
        Ok(rows
            .into_iter()
            .take(40)
            .map(|(tvg_id, name)| EpgSuggestion {
                line: format!("{tvg_id}  —  {name}"),
                tvg_id,
                name,
            })
            .collect())
    }

    pub fn now_playing(&self, tvg_id: &str, shift_hours: f64) -> Result<Option<NowPlaying>, StoreError> {
        let offset = time::UtcOffset::current_local_offset().unwrap_or(time::UtcOffset::UTC);
        self.now_playing_at(tvg_id, shift_hours, time::OffsetDateTime::now_utc(), offset)
    }

    pub fn now_playing_at(
        &self,
        tvg_id: &str,
        shift_hours: f64,
        now_utc: time::OffsetDateTime,
        local_offset: time::UtcOffset,
    ) -> Result<Option<NowPlaying>, StoreError> {
        if tvg_id.trim().is_empty() {
            return Ok(None);
        }
        let shift = time::Duration::seconds((shift_hours * 3600.0) as i64);
        let effective = now_utc - shift;
        let t = effective
            .format(&time::format_description::well_known::Rfc3339)
            .unwrap_or_default();
        for id in tvg_lookup_ids(tvg_id) {
            let row = self.conn.query_row(
                "SELECT title, start_utc, stop_utc FROM epg_programmes
                 WHERE tvg_id = ?1 COLLATE NOCASE AND start_utc <= ?2 AND stop_utc > ?2
                 ORDER BY start_utc DESC LIMIT 1",
                params![id, t],
                |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                    ))
                },
            );
            match row {
                Ok((title, start, stop)) => {
                    let start_dt = parse_rfc3339(&start).ok_or_else(|| {
                        StoreError::Io(std::io::Error::other("bad programme start"))
                    })?;
                    let stop_dt = parse_rfc3339(&stop).ok_or_else(|| {
                        StoreError::Io(std::io::Error::other("bad programme stop"))
                    })?;
                    return Ok(Some(NowPlaying {
                        title,
                        start_local: format_short_time((start_dt + shift).to_offset(local_offset)),
                        stop_local: format_short_time((stop_dt + shift).to_offset(local_offset)),
                    }));
                }
                Err(rusqlite::Error::QueryReturnedNoRows) => continue,
                Err(e) => return Err(e.into()),
            }
        }
        Ok(None)
    }

    pub fn replace_epg_catalog(&self, entries: &[CatalogEntry]) -> Result<(), StoreError> {
        let tx = self.conn.unchecked_transaction()?;
        tx.execute("DELETE FROM epg_catalog", [])?;
        let now = time::OffsetDateTime::now_utc()
            .format(&time::format_description::well_known::Rfc3339)
            .unwrap_or_default();
        {
            let mut ins = tx.prepare(
                "INSERT OR REPLACE INTO epg_catalog (tvg_id, name, logo, source, section, raw_json, fetched_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, NULL, ?6)",
            )?;
            for e in entries {
                ins.execute(params![
                    e.tvg_id,
                    e.name,
                    e.logo,
                    e.section,
                    e.section,
                    now
                ])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    pub fn replace_programmes(
        &self,
        items: &[(String, String, String, String)],
    ) -> Result<(), StoreError> {
        let tx = self.conn.unchecked_transaction()?;
        tx.execute_batch("DELETE FROM epg_programmes; DELETE FROM epg_now_playing;")?;
        let indexed = time::OffsetDateTime::now_utc()
            .format(&time::format_description::well_known::Rfc3339)
            .unwrap_or_default();
        {
            let mut ins = tx.prepare(
                "INSERT OR REPLACE INTO epg_programmes (tvg_id, title, description, start_utc, stop_utc, indexed_at)
                 VALUES (?1, ?2, NULL, ?3, ?4, ?5)",
            )?;
            let mut now_ins = tx.prepare(
                "INSERT OR REPLACE INTO epg_now_playing (tvg_id, title, description, start_utc, stop_utc, indexed_at)
                 VALUES (?1, ?2, NULL, ?3, ?4, ?5)",
            )?;
            let now = indexed.as_str();
            for (id, title, start, stop) in items {
                ins.execute(params![id, title, start, stop, indexed])?;
                if start.as_str() <= now && stop.as_str() > now {
                    now_ins.execute(params![id, title, start, stop, indexed])?;
                }
            }
        }
        tx.commit()?;
        Ok(())
    }

    pub fn list_catalog(&self) -> Result<Vec<CatalogEntry>, StoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT tvg_id, name, logo, IFNULL(section,'') FROM epg_catalog ORDER BY section, name COLLATE NOCASE",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(CatalogEntry {
                tvg_id: row.get(0)?,
                name: row.get(1)?,
                logo: row.get(2)?,
                section: row.get(3)?,
            })
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    pub fn catalog_count(&self) -> Result<i32, StoreError> {
        Ok(self
            .conn
            .query_row("SELECT COUNT(*) FROM epg_catalog", [], |r| r.get(0))?)
    }

    pub fn programme_count(&self) -> Result<i32, StoreError> {
        Ok(self
            .conn
            .query_row("SELECT COUNT(*) FROM epg_programmes", [], |r| r.get(0))?)
    }

    pub fn list_all_variants(&self) -> Result<Vec<StreamVariant>, StoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, managed_channel_id, url, label, source_entry_id, visibility, priority,
                    origin_name, origin_tvg_id, last_audit_ok, last_audit_at
             FROM stream_variants ORDER BY managed_channel_id, priority",
        )?;
        let rows = stmt.query_map([], read_variant)?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    pub fn get_variant(&self, id: &str) -> Result<Option<StreamVariant>, StoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, managed_channel_id, url, label, source_entry_id, visibility, priority,
                    origin_name, origin_tvg_id, last_audit_ok, last_audit_at
             FROM stream_variants WHERE id = ?1",
        )?;
        let mut rows = stmt.query_map(params![id], read_variant)?;
        Ok(rows.next().transpose()?)
    }

    pub fn insert_audit_result(&self, r: &crate::audit::AuditResult) -> Result<(), StoreError> {
        self.conn.execute(
            "INSERT INTO audit_results (
                id, target_type, target_id, ok, error, latency_ms, engine, probed_at,
                grade, width, height, fps, aspect_ratio, video_codec, audio_codec,
                job_id, channel_id, channel_name, group_title, tvg_id, error_class)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21)",
            params![
                r.id,
                r.target_type,
                r.target_id,
                r.ok as i32,
                r.error,
                r.latency_ms,
                r.engine,
                r.probed_at,
                r.grade,
                r.width,
                r.height,
                r.fps,
                r.aspect_ratio,
                r.video_codec,
                r.audio_codec,
                r.job_id,
                r.channel_id,
                r.channel_name,
                r.group_title,
                r.tvg_id,
                r.error_class
            ],
        )?;
        Ok(())
    }

    pub fn list_audit_results(&self, job_id: Option<&str>, limit: i32) -> Result<Vec<crate::audit::AuditResult>, StoreError> {
        let sql = if job_id.is_some() {
            "SELECT id, target_type, target_id, ok, error, latency_ms, engine, probed_at,
                    grade, width, height, fps, aspect_ratio, video_codec, audio_codec,
                    job_id, channel_id, channel_name, group_title, tvg_id, error_class
             FROM audit_results WHERE job_id = ?1 ORDER BY probed_at, id LIMIT ?2"
        } else {
            "SELECT id, target_type, target_id, ok, error, latency_ms, engine, probed_at,
                    grade, width, height, fps, aspect_ratio, video_codec, audio_codec,
                    job_id, channel_id, channel_name, group_title, tvg_id, error_class
             FROM audit_results ORDER BY probed_at DESC, id DESC LIMIT ?1"
        };
        let mut stmt = self.conn.prepare(sql)?;
        let map = |row: &rusqlite::Row<'_>| {
            Ok(crate::audit::AuditResult {
                id: row.get(0)?,
                target_type: row.get(1)?,
                target_id: row.get(2)?,
                ok: row.get::<_, i32>(3)? != 0,
                error: row.get(4)?,
                latency_ms: row.get(5)?,
                engine: row.get(6)?,
                probed_at: row.get(7)?,
                grade: row.get::<_, Option<String>>(8)?.unwrap_or_default(),
                width: row.get(9)?,
                height: row.get(10)?,
                fps: row.get(11)?,
                aspect_ratio: row.get(12)?,
                video_codec: row.get(13)?,
                audio_codec: row.get(14)?,
                job_id: row.get(15)?,
                channel_id: row.get(16)?,
                channel_name: row.get(17)?,
                group_title: row.get(18)?,
                tvg_id: row.get(19)?,
                error_class: row.get(20)?,
            })
        };
        let rows = if let Some(id) = job_id {
            stmt.query_map(params![id, limit], map)?
        } else {
            stmt.query_map(params![limit], map)?
        };
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    pub fn update_variant_audit(&self, variant_id: &str, ok: bool, at: &str) -> Result<(), StoreError> {
        self.conn.execute(
            "UPDATE stream_variants SET last_audit_ok = ?1, last_audit_at = ?2 WHERE id = ?3",
            params![ok as i32, at, variant_id],
        )?;
        Ok(())
    }

    pub fn swap_visible(
        &self,
        managed_id: &str,
        from_variant_id: &str,
        to_variant_id: &str,
        reason: &str,
    ) -> Result<(), StoreError> {
        let tx = self.conn.unchecked_transaction()?;
        tx.execute(
            "UPDATE stream_variants SET visibility='hidden_backup' WHERE managed_channel_id=?1",
            params![managed_id],
        )?;
        tx.execute(
            "UPDATE stream_variants SET visibility='visible' WHERE id=?1",
            params![to_variant_id],
        )?;
        tx.execute(
            "INSERT INTO swap_undo_log (id, managed_channel_id, from_variant_id, to_variant_id, reason, created_at, undone_at)
             VALUES (?1,?2,?3,?4,?5,?6,NULL)",
            params![
                uuid::Uuid::new_v4().simple().to_string(),
                managed_id,
                from_variant_id,
                to_variant_id,
                reason,
                time::OffsetDateTime::now_utc()
                    .format(&time::format_description::well_known::Rfc3339)
                    .unwrap_or_default()
            ],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn undo_last_swap(&self, managed_id: Option<&str>) -> Result<bool, StoreError> {
        let row = if let Some(mc) = managed_id {
            self.conn.query_row(
                "SELECT id, managed_channel_id, from_variant_id FROM swap_undo_log
                 WHERE undone_at IS NULL AND managed_channel_id=?1
                 ORDER BY created_at DESC LIMIT 1",
                params![mc],
                |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?)),
            )
        } else {
            self.conn.query_row(
                "SELECT id, managed_channel_id, from_variant_id FROM swap_undo_log
                 WHERE undone_at IS NULL
                 ORDER BY created_at DESC LIMIT 1",
                [],
                |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?)),
            )
        };
        let Ok((log_id, mc, from_id)) = row else {
            return Ok(false);
        };
        let tx = self.conn.unchecked_transaction()?;
        tx.execute(
            "UPDATE stream_variants SET visibility='hidden_backup' WHERE managed_channel_id=?1",
            params![mc],
        )?;
        if !from_id.is_empty() {
            tx.execute(
                "UPDATE stream_variants SET visibility='visible' WHERE id=?1",
                params![from_id],
            )?;
        }
        tx.execute(
            "UPDATE swap_undo_log SET undone_at=?1 WHERE id=?2",
            params![
                time::OffsetDateTime::now_utc()
                    .format(&time::format_description::well_known::Rfc3339)
                    .unwrap_or_default(),
                log_id
            ],
        )?;
        tx.commit()?;
        Ok(true)
    }

    pub fn list_programmes(&self, tvg_ids: &[String], from_utc: &str, to_utc: &str) -> Result<Vec<crate::models::EpgProgramme>, StoreError> {
        if tvg_ids.is_empty() {
            return Ok(Vec::new());
        }
        let mut sql = String::from(
            "SELECT tvg_id, title, description, start_utc, stop_utc FROM epg_programmes WHERE (",
        );
        for i in 0..tvg_ids.len() {
            if i > 0 {
                sql.push_str(" OR ");
            }
            sql.push_str(&format!("tvg_id = ?{} COLLATE NOCASE", i + 1));
        }
        sql.push_str(&format!(
            ") AND stop_utc > ?{} AND start_utc < ?{} ORDER BY tvg_id, start_utc",
            tvg_ids.len() + 1,
            tvg_ids.len() + 2
        ));
        let mut stmt = self.conn.prepare(&sql)?;
        let mut params: Vec<&dyn rusqlite::ToSql> = tvg_ids.iter().map(|s| s as &dyn rusqlite::ToSql).collect();
        params.push(&from_utc);
        params.push(&to_utc);
        let rows = stmt.query_map(params.as_slice(), |r| {
            Ok(crate::models::EpgProgramme {
                tvg_id: r.get(0)?,
                title: r.get(1)?,
                description: r.get(2)?,
                start_utc: r.get(3)?,
                stop_utc: r.get(4)?,
            })
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    pub fn pending_swap_count(&self, limit: i32) -> Result<i32, StoreError> {
        let n: i32 = self.conn.query_row(
            "SELECT COUNT(*) FROM (
                SELECT id FROM swap_undo_log WHERE undone_at IS NULL
                ORDER BY created_at DESC LIMIT ?1
             )",
            params![limit],
            |r| r.get(0),
        )?;
        Ok(n)
    }
}

fn read_managed(row: &rusqlite::Row<'_>) -> rusqlite::Result<ManagedChannel> {
    Ok(ManagedChannel {
        id: row.get(0)?,
        name: row.get(1)?,
        group_title: row.get(2)?,
        tvg_id: row.get(3)?,
        tvg_logo: row.get(4)?,
        notes: row.get(5)?,
        sort_order: row.get(6)?,
        tvg_shift_hours: row.get(7)?,
        in_tuner: row.get::<_, i32>(8)? != 0,
        tuner_number: row.get(9)?,
        variants: vec![],
        has_epg_match: false,
    })
}

fn read_variant(row: &rusqlite::Row<'_>) -> rusqlite::Result<StreamVariant> {
    Ok(StreamVariant {
        id: row.get(0)?,
        managed_channel_id: row.get(1)?,
        url: row.get(2)?,
        label: row.get(3)?,
        source_entry_id: row.get(4)?,
        visibility: row.get(5)?,
        priority: row.get(6)?,
        origin_name: row.get(7)?,
        origin_tvg_id: row.get(8)?,
        last_audit_ok: row
            .get::<_, Option<i32>>(9)
            .ok()
            .flatten()
            .map(|n| n != 0),
        last_audit_at: row.get(10).ok().flatten(),
    })
}

fn read_channels(
    stmt: &mut rusqlite::Statement<'_>,
    params: impl rusqlite::Params,
) -> Result<Vec<ChannelEntry>, StoreError> {
    let rows = stmt.query_map(params, |row| {
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
        let mut meta = crate::epg::EpgCacheMeta::default();
        meta.etag = Some("\"abc\"".into());
        store.save_epg_cache_meta(&meta).unwrap();
        let loaded_meta = store.load_epg_cache_meta();
        assert_eq!(loaded_meta.etag.as_deref(), Some("\"abc\""));
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

    #[test]
    fn groups_channels_and_remove_source() {
        let dir = tempdir().unwrap();
        let playlist = dir.path().join("list.m3u");
        std::fs::write(
            &playlist,
            "#EXTM3U\n#EXTINF:-1 group-title=\"News\",CNN\nhttp://example.com/cnn\n#EXTINF:-1 group-title=\"News\",MSNBC\nhttp://example.com/msnbc\n#EXTINF:-1 group-title=\"Sports\",ESPN\nhttp://example.com/espn\n",
        )
        .unwrap();
        let store = SqliteStore::open(&dir.path().join("t.db")).unwrap();
        let src = store.add_file_source(&playlist).unwrap();
        let groups = store.groups_with_counts(&src.id).unwrap();
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0], ("News".into(), 2));
        let news = store.channels_by_group(&src.id, "News").unwrap();
        assert_eq!(news.len(), 2);
        store.remove_source(&src.id).unwrap();
        assert!(store.list_sources().unwrap().is_empty());
        assert!(store.search_sources("CNN").unwrap().is_empty());
    }

    #[test]
    fn managed_channel_and_backup_order() {
        let dir = tempdir().unwrap();
        let store = SqliteStore::open(&dir.path().join("t.db")).unwrap();
        let ch = ManagedChannel {
            id: "c1".into(),
            name: "CNN".into(),
            group_title: "NEWS".into(),
            tvg_id: Some("CNN.us".into()),
            tvg_logo: None,
            notes: None,
            sort_order: 0,
            tvg_shift_hours: 0.0,
            in_tuner: false,
            tuner_number: None,
            variants: vec![],
            has_epg_match: false,
        };
        store.upsert_managed(&ch).unwrap();
        let a = store.add_stream("c1", "http://a", Some("A")).unwrap();
        let b = store.add_stream("c1", "http://b", Some("B")).unwrap();
        assert_eq!(a.visibility, "visible");
        assert_eq!(b.visibility, "hidden_backup");
        store.apply_variant_order("c1", &[b.id.clone(), a.id.clone()]).unwrap();
        let loaded = store.get_managed("c1").unwrap().unwrap();
        assert_eq!(loaded.variants[0].url, "http://b");
        assert_eq!(loaded.variants[0].visibility, "visible");
        assert_eq!(store.rename_managed_group("NEWS", "News HD").unwrap(), 1);
        assert_eq!(store.managed_groups().unwrap()[0].0, "News HD");
    }

    #[test]
    fn import_curated_skips_same_url() {
        let dir = tempdir().unwrap();
        let store = SqliteStore::open(&dir.path().join("t.db")).unwrap();
        let m3u = "#EXTM3U\n#EXTINF:-1 group-title=\"News\",CNN\nhttp://x/cnn\n";
        let (a, s) = store.import_curated(m3u, true, "file").unwrap();
        assert_eq!(a, 1);
        assert_eq!(s, 0);
        let (a2, s2) = store.import_curated(m3u, false, "file").unwrap();
        assert_eq!(a2, 0);
        assert_eq!(s2, 1);
    }

    fn sample_channel(id: &str, group: &str) -> ManagedChannel {
        ManagedChannel {
            id: id.into(),
            name: "CNN".into(),
            group_title: group.into(),
            tvg_id: Some("CNN.us".into()),
            tvg_logo: None,
            notes: None,
            sort_order: 0,
            tvg_shift_hours: 0.0,
            in_tuner: false,
            tuner_number: None,
            variants: vec![],
            has_epg_match: false,
        }
    }

    #[test]
    fn save_managed_blank_group_becomes_ungrouped() {
        let dir = tempdir().unwrap();
        let store = SqliteStore::open(&dir.path().join("t.db")).unwrap();
        store
            .save_managed_channel(&sample_channel("c1", "   "), Some("http://a"))
            .unwrap();
        let loaded = store.get_managed("c1").unwrap().unwrap();
        assert_eq!(loaded.group_title, "Ungrouped");
    }

    #[test]
    fn save_managed_updates_existing_visible_url() {
        let dir = tempdir().unwrap();
        let store = SqliteStore::open(&dir.path().join("t.db")).unwrap();
        store.upsert_managed(&sample_channel("c1", "News")).unwrap();
        store.add_stream("c1", "http://old", Some("primary")).unwrap();
        store.add_stream("c1", "http://backup", Some("B")).unwrap();
        store
            .save_managed_channel(&sample_channel("c1", "News"), Some("http://new"))
            .unwrap();
        let loaded = store.get_managed("c1").unwrap().unwrap();
        let visible = loaded
            .variants
            .iter()
            .find(|v| v.visibility == "visible")
            .unwrap();
        assert_eq!(visible.url, "http://new");
        assert_eq!(loaded.variants.len(), 2);
        assert!(loaded.variants.iter().any(|v| v.url == "http://backup"));
    }

    #[test]
    fn save_managed_creates_visible_when_none() {
        let dir = tempdir().unwrap();
        let store = SqliteStore::open(&dir.path().join("t.db")).unwrap();
        store
            .save_managed_channel(&sample_channel("c1", "News"), Some("http://first"))
            .unwrap();
        let loaded = store.get_managed("c1").unwrap().unwrap();
        assert_eq!(loaded.variants.len(), 1);
        assert_eq!(loaded.variants[0].url, "http://first");
        assert_eq!(loaded.variants[0].visibility, "visible");
        assert_eq!(loaded.variants[0].label.as_deref(), Some("primary"));
    }

    #[test]
    fn save_managed_empty_primary_leaves_variants() {
        let dir = tempdir().unwrap();
        let store = SqliteStore::open(&dir.path().join("t.db")).unwrap();
        store.upsert_managed(&sample_channel("c1", "News")).unwrap();
        store.add_stream("c1", "http://old", None).unwrap();
        store
            .save_managed_channel(&sample_channel("c1", "News"), Some("  "))
            .unwrap();
        let loaded = store.get_managed("c1").unwrap().unwrap();
        assert_eq!(loaded.variants[0].url, "http://old");
    }

    #[test]
    fn add_from_source_keeps_entry_group() {
        let dir = tempdir().unwrap();
        let playlist = dir.path().join("list.m3u");
        std::fs::write(
            &playlist,
            "#EXTM3U\n#EXTINF:-1 group-title=\"News\",CNN\nhttp://example.com/cnn\n",
        )
        .unwrap();
        let store = SqliteStore::open(&dir.path().join("t.db")).unwrap();
        let src = store.add_file_source(&playlist).unwrap();
        let entries = store.channels_by_group(&src.id, "News").unwrap();
        let ch = store.add_from_source_entry(&entries[0].id).unwrap();
        assert_eq!(ch.group_title, "News");
    }

    #[test]
    fn add_missing_skips_same_name_and_tvg() {
        let dir = tempdir().unwrap();
        let playlist = dir.path().join("list.m3u");
        std::fs::write(
            &playlist,
            "#EXTM3U\n#EXTINF:-1 tvg-id=\"CNN.us\" group-title=\"News\",CNN\nhttp://example.com/cnn\n#EXTINF:-1 tvg-id=\"MSNBC.us\" group-title=\"News\",MSNBC\nhttp://example.com/msnbc\n",
        )
        .unwrap();
        let store = SqliteStore::open(&dir.path().join("t.db")).unwrap();
        let src = store.add_file_source(&playlist).unwrap();
        let entries = store.channels_by_group(&src.id, "News").unwrap();
        let ids: Vec<String> = entries.iter().map(|e| e.id.clone()).collect();
        let (a1, s1) = store
            .add_missing_from_source_entries(&ids, Some("NewsSrc"))
            .unwrap();
        assert_eq!((a1, s1), (2, 0));
        let (a2, s2) = store
            .add_missing_from_source_entries(&ids, Some("NewsSrc"))
            .unwrap();
        assert_eq!((a2, s2), (0, 2));
    }

    #[test]
    fn now_playing_formats_local_times_and_applies_shift() {
        let dir = tempdir().unwrap();
        let store = SqliteStore::open(&dir.path().join("t.db")).unwrap();
        store
            .replace_programmes(&[(
                "CNN.us".into(),
                "News Hour".into(),
                "2026-08-18T15:00:00Z".into(),
                "2026-08-18T17:00:00Z".into(),
            )])
            .unwrap();
        let now = time::OffsetDateTime::parse(
            "2026-08-18T18:00:00Z",
            &time::format_description::well_known::Rfc3339,
        )
        .unwrap();
        let utc = time::UtcOffset::UTC;
        let hit = store
            .now_playing_at("CNN.us", 2.0, now, utc)
            .unwrap()
            .expect("in window after shift");
        assert_eq!(hit.title, "News Hour");
        assert_eq!(hit.start_local, "5:00 PM");
        assert_eq!(hit.stop_local, "7:00 PM");
        assert!(store
            .now_playing_at("CNN.us", 0.0, now, utc)
            .unwrap()
            .is_none());
    }
}
