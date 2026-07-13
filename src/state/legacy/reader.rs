//------------------------------------------------------------------------------------
// state/legacy/reader.rs -- Part of RHoiScribe
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
    fs,
    time::{SystemTime, UNIX_EPOCH},
};

use rnmdb_common::ids::PageId;
use rnmdb_storage::{PageCryptoKey, SingleFileBackend, StorageBackend};
use serde::Deserialize;
use serde_json::Value;

use super::super::{
    GLOBAL_SCOPE_KEY, GLOBAL_SCOPE_KIND, StoredPreferenceRecord, StoredToolLogRecord,
    global_record_key, state_database_error,
};

const PREFERENCES_PAGE_ID: u64 = 1;
const TOOL_LOG_INDEX_PAGE_ID: u64 = 2;
const TOOL_LOG_DATA_START_PAGE_ID: u64 = 3;
const SQL_FRAME_MAGIC: &[u8; 8] = b"RNOVSI01";
const LEGACY_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Deserialize)]
struct LegacyPreferences {
    schema_version: u32,
    preferences: BTreeMap<String, Value>,
}

#[derive(Debug, Deserialize)]
struct LegacyToolLogIndex {
    schema_version: u32,
    byte_len: usize,
    page_count: u64,
}

impl LegacyToolLogIndex {
    fn is_empty(&self) -> bool {
        self.byte_len == 0 && self.page_count == 0
    }
}

#[derive(Debug, Deserialize)]
struct LegacyToolLogEntry {
    sequence: u64,
    timestamp_unix_seconds: u64,
    tool_name: String,
    arguments: Value,
    success: bool,
    result: Option<Value>,
    error: Option<String>,
}

pub(super) struct LegacySnapshot {
    pub(super) preferences: Vec<StoredPreferenceRecord>,
    pub(super) logs: Vec<StoredToolLogRecord>,
}

pub(super) enum ExistingLayout {
    Sql,
    Legacy(LegacySnapshot),
}

pub(super) fn inspect_existing_layout(
    readable_path: &std::path::Path,
    canonical_path: &std::path::Path,
    key: PageCryptoKey,
) -> Result<ExistingLayout, String> {
    let backend = SingleFileBackend::open_with_key(readable_path, key)
        .map_err(|error| state_database_error(canonical_path, "open", error.to_string()))?;
    if backend.catalog_root().is_some() {
        return Ok(ExistingLayout::Sql);
    }
    if legacy_root_is_sql_frame(&backend, canonical_path)? {
        return Ok(ExistingLayout::Sql);
    }
    read_legacy_snapshot(&backend, canonical_path)
        .map(ExistingLayout::Legacy)
        .map_err(|error| state_database_error(canonical_path, "migrate", error))
}

fn legacy_root_is_sql_frame(
    backend: &SingleFileBackend,
    canonical_path: &std::path::Path,
) -> Result<bool, String> {
    let page = read_page(backend, canonical_path, PREFERENCES_PAGE_ID)?;
    Ok(page.is_some_and(|payload| payload.starts_with(SQL_FRAME_MAGIC)))
}

fn read_legacy_snapshot(
    backend: &SingleFileBackend,
    canonical_path: &std::path::Path,
) -> Result<LegacySnapshot, String> {
    let preferences = read_legacy_preferences(backend, canonical_path)?;
    let logs = read_legacy_logs(backend, canonical_path)?;
    Ok(LegacySnapshot { preferences, logs })
}

fn read_legacy_preferences(
    backend: &SingleFileBackend,
    canonical_path: &std::path::Path,
) -> Result<Vec<StoredPreferenceRecord>, String> {
    let Some(payload) = read_page(backend, canonical_path, PREFERENCES_PAGE_ID)? else {
        return Ok(Vec::new());
    };
    let Some(preferences) = decode_length_prefixed::<LegacyPreferences>(&payload, "preferences")?
    else {
        return Ok(Vec::new());
    };
    validate_legacy_schema(preferences.schema_version, "preferences")?;
    let updated_at = unix_timestamp_now();
    preferences
        .preferences
        .into_iter()
        .map(|(key, value)| legacy_preference_record(key, value, updated_at))
        .collect()
}

fn legacy_preference_record(
    preference_key: String,
    value: Value,
    updated_at_unix_seconds: u64,
) -> Result<StoredPreferenceRecord, String> {
    let value_json = serde_json::to_string(&value).map_err(|error| error.to_string())?;
    Ok(StoredPreferenceRecord {
        record_key: global_record_key(&preference_key),
        scope_kind: GLOBAL_SCOPE_KIND.to_string(),
        scope_key: GLOBAL_SCOPE_KEY.to_string(),
        mod_root: None,
        preference_key,
        value_json,
        updated_at_unix_seconds,
    })
}

fn read_legacy_logs(
    backend: &SingleFileBackend,
    canonical_path: &std::path::Path,
) -> Result<Vec<StoredToolLogRecord>, String> {
    let Some(payload) = read_page(backend, canonical_path, TOOL_LOG_INDEX_PAGE_ID)? else {
        return Ok(Vec::new());
    };
    let Some(index) = decode_length_prefixed::<LegacyToolLogIndex>(&payload, "tool log index")?
    else {
        return Ok(Vec::new());
    };
    validate_legacy_schema(index.schema_version, "tool log index")?;
    if index.is_empty() {
        return Ok(Vec::new());
    }
    validate_log_index(&index, backend)?;
    let bytes = read_legacy_log_bytes(backend, canonical_path, &index)?;
    let entries = serde_json::from_slice::<Vec<LegacyToolLogEntry>>(&bytes)
        .map_err(|error| format!("failed to decode legacy tool logs: {error}"))?;
    entries.into_iter().map(legacy_log_record).collect()
}

fn validate_log_index(
    index: &LegacyToolLogIndex,
    backend: &SingleFileBackend,
) -> Result<(), String> {
    let page_size = backend.page_size().bytes();
    let expected = index.byte_len.div_ceil(page_size);
    let actual = usize::try_from(index.page_count)
        .map_err(|_| "legacy tool log page count does not fit this platform".to_string())?;
    if actual != expected {
        return Err(format!(
            "legacy tool log index page count {actual} does not match byte length {}",
            index.byte_len
        ));
    }
    let file_len = fs::metadata(backend.path())
        .map_err(|error| format!("failed to inspect legacy state database size: {error}"))?
        .len();
    let byte_len = u64::try_from(index.byte_len)
        .map_err(|_| "legacy tool log byte length does not fit RNMDB limits".to_string())?;
    if byte_len > file_len {
        return Err("legacy tool log byte length exceeds the database file size".to_string());
    }
    Ok(())
}

fn read_legacy_log_bytes(
    backend: &SingleFileBackend,
    canonical_path: &std::path::Path,
    index: &LegacyToolLogIndex,
) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(index.byte_len)
        .map_err(|error| format!("legacy tool log allocation failed: {error}"))?;
    for offset in 0..index.page_count {
        let page_id = TOOL_LOG_DATA_START_PAGE_ID.saturating_add(offset);
        let payload = read_page(backend, canonical_path, page_id)?
            .ok_or_else(|| format!("legacy tool log page {page_id} is missing"))?;
        bytes.extend_from_slice(&payload);
    }
    bytes.truncate(index.byte_len);
    Ok(bytes)
}

fn legacy_log_record(entry: LegacyToolLogEntry) -> Result<StoredToolLogRecord, String> {
    let arguments_json =
        serde_json::to_string(&entry.arguments).map_err(|error| error.to_string())?;
    let result_json = entry
        .result
        .map(|value| serde_json::to_string(&value))
        .transpose()
        .map_err(|error| error.to_string())?;
    Ok(StoredToolLogRecord {
        sequence: entry.sequence,
        timestamp_unix_seconds: entry.timestamp_unix_seconds,
        scope_kind: GLOBAL_SCOPE_KIND.to_string(),
        scope_key: GLOBAL_SCOPE_KEY.to_string(),
        mod_root: None,
        tool_name: entry.tool_name,
        arguments_json,
        success: entry.success,
        result_json,
        error_text: entry.error,
    })
}

fn read_page(
    backend: &SingleFileBackend,
    canonical_path: &std::path::Path,
    page_id: u64,
) -> Result<Option<Vec<u8>>, String> {
    backend
        .read_page(PageId::new(page_id))
        .map(|page| page.map(|page| page.payload().to_vec()))
        .map_err(|error| {
            state_database_error(
                canonical_path,
                "migrate",
                format!("failed to read legacy page {page_id}: {error}"),
            )
        })
}

fn decode_length_prefixed<T>(payload: &[u8], label: &str) -> Result<Option<T>, String>
where
    T: for<'de> Deserialize<'de>,
{
    if payload.len() < 4 {
        return Ok(None);
    }
    let length = u32::from_be_bytes([payload[0], payload[1], payload[2], payload[3]]) as usize;
    if length == 0 {
        return Ok(None);
    }
    if length > payload.len().saturating_sub(4) {
        return Err(format!("legacy {label} page has an invalid payload length"));
    }
    serde_json::from_slice(&payload[4..4 + length])
        .map(Some)
        .map_err(|error| format!("failed to decode legacy {label}: {error}"))
}

pub(crate) fn is_legacy_state_page(page_id: u64, payload: &[u8]) -> bool {
    let Ok(Some(Value::Object(object))) =
        decode_length_prefixed::<Value>(payload, "state classifier")
    else {
        return false;
    };
    let has_schema = object.get("schema_version").is_some_and(Value::is_u64);
    match page_id {
        PREFERENCES_PAGE_ID => {
            has_schema && object.get("preferences").is_some_and(Value::is_object)
        }
        TOOL_LOG_INDEX_PAGE_ID => {
            has_schema
                && object.get("byte_len").is_some_and(Value::is_u64)
                && object.get("page_count").is_some_and(Value::is_u64)
        }
        _ => false,
    }
}

fn validate_legacy_schema(version: u32, label: &str) -> Result<(), String> {
    if version <= LEGACY_SCHEMA_VERSION {
        return Ok(());
    }
    Err(format!(
        "legacy {label} schema version {version} is newer than supported version {LEGACY_SCHEMA_VERSION}"
    ))
}

fn unix_timestamp_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}
