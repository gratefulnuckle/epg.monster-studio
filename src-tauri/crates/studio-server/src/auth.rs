// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use studio_core::paths::app_data_directory;
use uuid::Uuid;

const USER: &str = "admin";
const SESSION_TTL: Duration = Duration::from_secs(7 * 24 * 3600);
const COOKIE: &str = "studio_sid";

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AuthFile {
    username: String,
    salt: String,
    hash: String,
    must_change: bool,
}

#[derive(Clone)]
pub struct Session {
    pub username: String,
    pub must_change: bool,
    pub created: Instant,
}

#[derive(Clone, Default)]
pub struct Auth {
    sessions: HashMap<String, Session>,
}

fn auth_path() -> PathBuf {
    app_data_directory().join("web-auth.json")
}

fn hash_password(salt_hex: &str, password: &str) -> String {
    let mut h = Sha256::new();
    h.update(salt_hex.as_bytes());
    h.update(password.as_bytes());
    format!("{:x}", h.finalize())
}

fn random_hex(n: usize) -> String {
    let mut out = String::new();
    while out.len() < n * 2 {
        out.push_str(&Uuid::new_v4().simple().to_string());
    }
    out.chars().take(n * 2).collect()
}

fn random_password() -> String {
    const CHARS: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz23456789";
    let mut out = String::with_capacity(12);
    let bytes = Uuid::new_v4().into_bytes();
    let extra = Uuid::new_v4().into_bytes();
    for i in 0..12 {
        let b = if i < 16 { bytes[i] } else { extra[i - 16] };
        out.push(CHARS[(b as usize) % CHARS.len()] as char);
    }
    out
}

fn load_file() -> Result<AuthFile, String> {
    let p = auth_path();
    let raw = std::fs::read_to_string(&p).map_err(|_| {
        "No web login is set. Run studio.ps1 --makepass / studio.sh --makepass.".to_string()
    })?;
    serde_json::from_str(&raw).map_err(|e| e.to_string())
}

fn save_file(f: &AuthFile) -> Result<(), String> {
    let dir = app_data_directory();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let p = auth_path();
    let body = serde_json::to_string_pretty(f).map_err(|e| e.to_string())?;
    std::fs::write(p, body).map_err(|e| e.to_string())
}

/// Create a temporary admin password. Always requires a change on next login.
pub fn make_temp_password(explicit: Option<&str>) -> Result<String, String> {
    let password = explicit
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .unwrap_or_else(random_password);
    let salt = random_hex(16);
    let hash = hash_password(&salt, &password);
    save_file(&AuthFile {
        username: USER.into(),
        salt,
        hash,
        must_change: true,
    })?;
    Ok(password)
}

const KEY_PREFIX: &str = "epgs_";

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApiKeyFile {
    #[serde(default)]
    keys: Vec<ApiKeyRecord>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApiKeyRecord {
    id: String,
    name: String,
    salt: String,
    hash: String,
    prefix: String,
    created: u64,
    #[serde(default)]
    last_used: Option<u64>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiKeyInfo {
    pub id: String,
    pub name: String,
    pub prefix: String,
    pub created: u64,
    pub last_used: Option<u64>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NewApiKey {
    pub id: String,
    pub name: String,
    pub key: String,
    pub created: u64,
}

fn keys_path() -> PathBuf {
    app_data_directory().join("api-keys.json")
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn load_keys() -> ApiKeyFile {
    let raw = std::fs::read_to_string(keys_path()).ok();
    raw.and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or(ApiKeyFile { keys: Vec::new() })
}

fn save_keys(f: &ApiKeyFile) -> Result<(), String> {
    let dir = app_data_directory();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let body = serde_json::to_string_pretty(f).map_err(|e| e.to_string())?;
    std::fs::write(keys_path(), body).map_err(|e| e.to_string())
}

fn random_api_key() -> String {
    format!("{KEY_PREFIX}{}", random_hex(16))
}

/// Desktop / automation key. Plaintext is returned once.
pub fn create_api_key(name: Option<&str>) -> Result<NewApiKey, String> {
    let name = name
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("Desktop")
        .to_string();
    let key = random_api_key();
    let salt = random_hex(16);
    let hash = hash_password(&salt, &key);
    let created = unix_now();
    let id = Uuid::new_v4().simple().to_string();
    let prefix: String = key.chars().take(12).collect();
    let mut file = load_keys();
    file.keys.push(ApiKeyRecord {
        id: id.clone(),
        name: name.clone(),
        salt,
        hash,
        prefix: prefix.clone(),
        created,
        last_used: None,
    });
    save_keys(&file)?;
    Ok(NewApiKey {
        id,
        name,
        key,
        created,
    })
}

pub fn list_api_keys() -> Vec<ApiKeyInfo> {
    load_keys()
        .keys
        .into_iter()
        .map(|k| ApiKeyInfo {
            id: k.id,
            name: k.name,
            prefix: k.prefix,
            created: k.created,
            last_used: k.last_used,
        })
        .collect()
}

pub fn revoke_api_key(id: &str) -> Result<(), String> {
    let mut file = load_keys();
    let n = file.keys.len();
    file.keys.retain(|k| k.id != id);
    if file.keys.len() == n {
        return Err("No API key with that id.".into());
    }
    save_keys(&file)
}

pub fn api_key_count() -> usize {
    load_keys().keys.len()
}

pub fn verify_api_key(raw: &str) -> bool {
    let raw = raw.trim();
    if !raw.starts_with(KEY_PREFIX) {
        return false;
    }
    let mut file = load_keys();
    let now = unix_now();
    let mut ok = false;
    let mut dirty = false;
    for k in &mut file.keys {
        if hash_password(&k.salt, raw) == k.hash {
            if k.last_used.map(|t| now.saturating_sub(t) > 60).unwrap_or(true) {
                k.last_used = Some(now);
                dirty = true;
            }
            ok = true;
            break;
        }
    }
    if dirty {
        let _ = save_keys(&file);
    }
    ok
}

pub fn parse_bearer(header: Option<&str>) -> Option<String> {
    let h = header?.trim();
    let rest = h
        .strip_prefix("Bearer ")
        .or_else(|| h.strip_prefix("bearer "))?;
    let t = rest.trim();
    if t.is_empty() {
        None
    } else {
        Some(t.to_string())
    }
}

impl Auth {
    pub fn parse_cookie(header: Option<&str>) -> Option<String> {
        let h = header?;
        for part in h.split(';') {
            let p = part.trim();
            if let Some(v) = p.strip_prefix(COOKIE).and_then(|s| s.strip_prefix('=')) {
                let t = v.trim();
                if !t.is_empty() {
                    return Some(t.to_string());
                }
            }
        }
        None
    }

    pub fn session(&self, token: &str) -> Option<Session> {
        let s = self.sessions.get(token)?;
        if s.created.elapsed() > SESSION_TTL {
            return None;
        }
        Some(s.clone())
    }

    pub fn login(&mut self, username: &str, password: &str) -> Result<(String, Session), String> {
        let file = load_file()?;
        if !username.trim().eq_ignore_ascii_case(&file.username) {
            return Err("Invalid username or password.".into());
        }
        let hashed = hash_password(&file.salt, password);
        if hashed != file.hash {
            return Err("Invalid username or password.".into());
        }
        let token = Uuid::new_v4().simple().to_string();
        let sess = Session {
            username: file.username,
            must_change: file.must_change,
            created: Instant::now(),
        };
        self.sessions.insert(token.clone(), sess.clone());
        Ok((token, sess))
    }

    pub fn change_password(&mut self, token: &str, current: &str, next: &str) -> Result<Session, String> {
        let next = next.trim();
        if next.chars().count() < 8 {
            return Err("New password must be at least 8 characters.".into());
        }
        if next == current {
            return Err("Choose a different password than the temporary one.".into());
        }
        let file = load_file()?;
        if hash_password(&file.salt, current) != file.hash {
            return Err("Current password is wrong.".into());
        }
        let salt = random_hex(16);
        save_file(&AuthFile {
            username: file.username.clone(),
            salt: salt.clone(),
            hash: hash_password(&salt, next),
            must_change: false,
        })?;
        if let Some(s) = self.sessions.get_mut(token) {
            s.must_change = false;
            s.created = Instant::now();
            return Ok(s.clone());
        }
        Err("Not signed in.".into())
    }

    pub fn logout(&mut self, token: &str) {
        self.sessions.remove(token);
    }

    pub fn set_cookie(token: &str) -> String {
        format!(
            "{COOKIE}={token}; Path=/; HttpOnly; SameSite=Lax; Max-Age={}",
            SESSION_TTL.as_secs()
        )
    }

    pub fn clear_cookie() -> String {
        format!("{COOKIE}=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_is_stable_for_same_salt() {
        let a = hash_password("abc", "secret");
        let b = hash_password("abc", "secret");
        let c = hash_password("abc", "other");
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert_eq!(a.len(), 64);
    }

    #[test]
    fn temp_password_is_12_unambiguous_chars() {
        let p = random_password();
        assert_eq!(p.len(), 12);
        assert!(p.chars().all(|c| c.is_ascii_alphanumeric()));
        assert!(!p.contains('0') && !p.contains('O') && !p.contains('l') && !p.contains('1'));
    }

    #[test]
    fn api_key_shape_and_bearer() {
        let k = random_api_key();
        assert!(k.starts_with("epgs_"));
        assert_eq!(k.len(), 5 + 32);
        assert_eq!(parse_bearer(Some("Bearer epgs_abc")), Some("epgs_abc".into()));
        assert_eq!(parse_bearer(Some("epgs_abc")), None);
        assert!(!verify_api_key("epgs_not_a_real_key_00000000000000"));
    }
}
