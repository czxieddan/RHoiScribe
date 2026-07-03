//------------------------------------------------------------------------------------
// preferences.rs -- Part of RHoiScribe
//
// Copyright (C) 2026 CzXieDdan. All rights reserved.
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.
//
// https://github.com/czxieddan/RHoiScribe
//------------------------------------------------------------------------------------

use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    path::{Path, PathBuf},
    thread,
    time::{Duration, SystemTime},
};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::rnmdb_store::{
    DEFAULT_RNMDB_PAGE_SIZE_BYTES, RnmdbSingleFilePageStore, clean_display_path,
    default_rhoiscribe_dir,
};

const PREFERENCES_PAGE_ID: u64 = 1;
const PREFERENCES_SCHEMA_VERSION: u32 = 1;
const PREFERENCE_LOCK_RETRY_COUNT: usize = 250;
const PREFERENCE_LOCK_RETRY_DELAY: Duration = Duration::from_millis(20);
const STALE_LOCK_AFTER: Duration = Duration::from_secs(30 * 60);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ListAgentPreferencesRequest {
    pub store_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SetAgentPreferenceRequest {
    pub key: String,
    pub value: Value,
    pub store_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DeleteAgentPreferenceRequest {
    pub key: String,
    pub store_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentPreferenceItem {
    pub key: String,
    pub value: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentPreferencesResult {
    pub store_path: String,
    pub backend: String,
    pub preferences: Vec<AgentPreferenceItem>,
    pub messages: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentPreferenceMutationResult {
    pub store_path: String,
    pub backend: String,
    pub key: String,
    pub removed: bool,
    pub value: Option<Value>,
    pub preferences: Vec<AgentPreferenceItem>,
    pub messages: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
struct StoredPreferences {
    schema_version: u32,
    preferences: BTreeMap<String, Value>,
}

pub fn list_agent_preferences(
    request: ListAgentPreferencesRequest,
) -> Result<AgentPreferencesResult, String> {
    let store_path = preference_store_path(request.store_path.as_deref());
    let store = open_preference_store(&store_path)?;
    let snapshot = read_preferences(&store)?;
    let preferences = preference_items(&snapshot.preferences);

    Ok(AgentPreferencesResult {
        store_path: clean_display_path(&store_path),
        backend: "RNMDB single-file page store".to_string(),
        preferences,
        messages: vec![
            "preferences are shared across MCP clients and IDEs through .rhoiscribe".to_string(),
        ],
    })
}

pub fn set_agent_preference(
    request: SetAgentPreferenceRequest,
) -> Result<AgentPreferenceMutationResult, String> {
    let key = normalized_preference_key(&request.key)?;
    let store_path = preference_store_path(request.store_path.as_deref());
    let _lock = PreferenceMutationLock::acquire(&store_path)?;
    let store = open_preference_store(&store_path)?;
    let mut snapshot = read_preferences(&store)?;
    snapshot
        .preferences
        .insert(key.clone(), request.value.clone());
    write_preferences(&store, &snapshot)?;

    Ok(AgentPreferenceMutationResult {
        store_path: clean_display_path(&store_path),
        backend: "RNMDB single-file page store".to_string(),
        key,
        removed: false,
        value: Some(request.value),
        preferences: preference_items(&snapshot.preferences),
        messages: vec!["preference stored in RNMDB-backed .rhoiscribe state".to_string()],
    })
}

pub fn delete_agent_preference(
    request: DeleteAgentPreferenceRequest,
) -> Result<AgentPreferenceMutationResult, String> {
    let key = normalized_preference_key(&request.key)?;
    let store_path = preference_store_path(request.store_path.as_deref());
    let _lock = PreferenceMutationLock::acquire(&store_path)?;
    let store = open_preference_store(&store_path)?;
    let mut snapshot = read_preferences(&store)?;
    let value = snapshot.preferences.remove(&key);
    write_preferences(&store, &snapshot)?;

    Ok(AgentPreferenceMutationResult {
        store_path: clean_display_path(&store_path),
        backend: "RNMDB single-file page store".to_string(),
        key,
        removed: value.is_some(),
        value,
        preferences: preference_items(&snapshot.preferences),
        messages: vec!["preference state updated in RNMDB".to_string()],
    })
}

pub(crate) fn preference_store_path(store_path: Option<&str>) -> PathBuf {
    store_path
        .map(|path| PathBuf::from(path.trim().trim_matches('"')))
        .unwrap_or_else(|| default_rhoiscribe_dir().join("preferences.rnmdb"))
}

pub(crate) struct PreferenceMutationLock {
    path: PathBuf,
    _file: File,
}

impl PreferenceMutationLock {
    pub(crate) fn acquire(store_path: &Path) -> Result<Self, String> {
        let path = preference_lock_path(store_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }

        for _ in 0..PREFERENCE_LOCK_RETRY_COUNT {
            remove_stale_lock(&path)?;
            match OpenOptions::new().write(true).create_new(true).open(&path) {
                Ok(file) => return Ok(Self { path, _file: file }),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                    thread::sleep(PREFERENCE_LOCK_RETRY_DELAY);
                }
                Err(error) => return Err(error.to_string()),
            }
        }

        Err(format!(
            "timed out waiting for preference store lock at {}",
            clean_display_path(&path)
        ))
    }
}

impl Drop for PreferenceMutationLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn preference_lock_path(store_path: &Path) -> PathBuf {
    let mut file_name = store_path
        .file_name()
        .unwrap_or_else(|| std::ffi::OsStr::new("preferences.rnmdb"))
        .to_os_string();
    file_name.push(".lock");
    store_path.with_file_name(file_name)
}

fn remove_stale_lock(path: &Path) -> Result<(), String> {
    let Ok(metadata) = fs::metadata(path) else {
        return Ok(());
    };
    let Ok(modified) = metadata.modified() else {
        return Ok(());
    };
    if SystemTime::now()
        .duration_since(modified)
        .is_ok_and(|age| age > STALE_LOCK_AFTER)
    {
        fs::remove_file(path).map_err(|error| error.to_string())?;
    }
    Ok(())
}

pub(crate) fn open_preference_store(path: &Path) -> Result<RnmdbSingleFilePageStore, String> {
    RnmdbSingleFilePageStore::open_or_create(path, DEFAULT_RNMDB_PAGE_SIZE_BYTES)
}

fn read_preferences(store: &RnmdbSingleFilePageStore) -> Result<StoredPreferences, String> {
    let Some(payload) = store.read_payload_page(PREFERENCES_PAGE_ID)? else {
        return Ok(default_preferences());
    };
    decode_preferences_payload(&payload)
}

fn write_preferences(
    store: &RnmdbSingleFilePageStore,
    preferences: &StoredPreferences,
) -> Result<(), String> {
    let payload = encode_preferences_payload(preferences, store.page_size_bytes())?;
    store.write_payload_page(PREFERENCES_PAGE_ID, payload)
}

fn default_preferences() -> StoredPreferences {
    StoredPreferences {
        schema_version: PREFERENCES_SCHEMA_VERSION,
        preferences: BTreeMap::new(),
    }
}

fn decode_preferences_payload(payload: &[u8]) -> Result<StoredPreferences, String> {
    if payload.len() < 4 {
        return Ok(default_preferences());
    }
    let length = u32::from_be_bytes([payload[0], payload[1], payload[2], payload[3]]) as usize;
    if length == 0 {
        return Ok(default_preferences());
    }
    if length > payload.len().saturating_sub(4) {
        return Err("stored RNMDB preference page has an invalid payload length".to_string());
    }
    let mut preferences = serde_json::from_slice::<StoredPreferences>(&payload[4..4 + length])
        .map_err(|error| error.to_string())?;
    if preferences.schema_version == 0 {
        preferences.schema_version = PREFERENCES_SCHEMA_VERSION;
    }
    Ok(preferences)
}

fn encode_preferences_payload(
    preferences: &StoredPreferences,
    page_size_bytes: usize,
) -> Result<Vec<u8>, String> {
    let encoded = serde_json::to_vec(preferences).map_err(|error| error.to_string())?;
    if encoded.len() + 4 > page_size_bytes {
        return Err(format!(
            "preference payload is too large for the RNMDB page: {} bytes > {} bytes",
            encoded.len() + 4,
            page_size_bytes
        ));
    }

    let mut payload = vec![0_u8; page_size_bytes];
    payload[..4].copy_from_slice(&(encoded.len() as u32).to_be_bytes());
    payload[4..4 + encoded.len()].copy_from_slice(&encoded);
    Ok(payload)
}

fn preference_items(preferences: &BTreeMap<String, Value>) -> Vec<AgentPreferenceItem> {
    preferences
        .iter()
        .map(|(key, value)| AgentPreferenceItem {
            key: key.clone(),
            value: value.clone(),
        })
        .collect()
}

fn normalized_preference_key(key: &str) -> Result<String, String> {
    let key = key.trim();
    if key.is_empty() {
        return Err("preference key is required".to_string());
    }
    if !key
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || matches!(character, '_' | '-' | '.'))
    {
        return Err(
            "preference key may contain only ASCII letters, digits, underscore, dash, or dot"
                .to_string(),
        );
    }
    Ok(key.to_ascii_lowercase())
}
