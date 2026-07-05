//------------------------------------------------------------------------------------
// tool_logs.rs -- Part of RHoiScribe
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
    fs,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

use regex::Regex;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Map, Value, json};

use super::{
    preferences::state_database_error,
    preferences::{PreferenceMutationLock, open_preference_store, preference_store_path},
    rnmdb_store::{RnmdbSingleFilePageStore, clean_display_path},
};

pub(crate) const MAX_TOOL_LOG_ENTRIES: usize = 32767;

const TOOL_LOG_INDEX_PAGE_ID: u64 = 2;
const TOOL_LOG_DATA_START_PAGE_ID: u64 = 3;
const TOOL_LOG_SCHEMA_VERSION: u32 = 1;
const DEFAULT_TOOL_LOG_LIMIT: usize = 100;
const MAX_LOGGED_JSON_CHARS: usize = 8192;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolLogEntry {
    pub sequence: u64,
    pub timestamp_unix_seconds: u64,
    pub tool_name: String,
    pub arguments: Value,
    pub success: bool,
    pub result: Option<Value>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ToolLogQueryRequest {
    pub store_path: Option<String>,
    pub pattern: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolLogExportRequest {
    pub store_path: Option<String>,
    pub output_path: String,
    pub pattern: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolLogQueryResult {
    pub store_path: String,
    pub backend: String,
    pub total_entries: usize,
    pub matched_entries: usize,
    pub entries: Vec<ToolLogEntry>,
    pub messages: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolLogExportResult {
    pub store_path: String,
    pub output_path: String,
    pub exported_entries: usize,
    pub messages: Vec<String>,
}

pub(crate) struct ToolLogAppend {
    pub(crate) tool_name: String,
    pub(crate) arguments: Value,
    pub(crate) success: bool,
    pub(crate) result: Option<Value>,
    pub(crate) error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct StoredToolLogIndex {
    schema_version: u32,
    byte_len: usize,
    page_count: u64,
}

impl StoredToolLogIndex {
    fn is_empty(&self) -> bool {
        self.byte_len == 0 || self.page_count == 0
    }
}

pub(crate) fn tool_log_store_path_from_arguments(arguments: &Map<String, Value>) -> Option<String> {
    arguments
        .get("store_path")
        .and_then(Value::as_str)
        .map(str::to_string)
}

pub(crate) fn record_tool_call(
    store_path: Option<&str>,
    append: ToolLogAppend,
) -> Result<(), String> {
    let store_path = preference_store_path(store_path);
    let _lock = PreferenceMutationLock::acquire(&store_path)
        .map_err(|error| state_database_error(&store_path, error))?;
    let store = open_preference_store(&store_path)?;
    let mut entries = read_tool_log_entries(&store, &store_path)?;
    let sequence = next_sequence(&entries);

    entries.push(ToolLogEntry {
        sequence,
        timestamp_unix_seconds: unix_timestamp_now(),
        tool_name: append.tool_name,
        arguments: compact_json_value(append.arguments),
        success: append.success,
        result: append.result.map(compact_json_value),
        error: append.error,
    });
    trim_old_entries(&mut entries);
    write_tool_log_entries(&store, &store_path, &entries)
}

pub fn query_tool_logs(request: ToolLogQueryRequest) -> Result<ToolLogQueryResult, String> {
    let store_path = preference_store_path(request.store_path.as_deref());
    let store = open_preference_store(&store_path)?;
    let entries = read_tool_log_entries(&store, &store_path)?;
    let matcher = compile_log_regex(request.pattern.as_deref())?;
    let limit = query_limit(request.limit, DEFAULT_TOOL_LOG_LIMIT);
    let total_entries = entries.len();
    let (matched_entries, entries) = filter_log_entries(&entries, matcher.as_ref(), limit)?;

    Ok(ToolLogQueryResult {
        store_path: clean_display_path(&store_path),
        backend: "RNMDB single-file page store".to_string(),
        total_entries,
        matched_entries,
        entries,
        messages: vec![
            "tool logs share the same RNMDB database as agent preferences".to_string(),
            format!("only the latest {MAX_TOOL_LOG_ENTRIES} entries are retained"),
        ],
    })
}

pub fn export_tool_logs(request: ToolLogExportRequest) -> Result<ToolLogExportResult, String> {
    let store_path = preference_store_path(request.store_path.as_deref());
    let limit = query_limit(request.limit, MAX_TOOL_LOG_ENTRIES);
    let entries = filtered_tool_log_entries(&store_path, request.pattern.as_deref(), limit)?;
    let output_path = Path::new(&request.output_path);

    write_tool_log_export(output_path, tool_log_export_payload(&store_path, &entries))?;

    Ok(ToolLogExportResult {
        store_path: clean_display_path(&store_path),
        output_path: clean_display_path(output_path),
        exported_entries: entries.len(),
        messages: vec!["matching tool logs exported as JSON".to_string()],
    })
}

fn filtered_tool_log_entries(
    store_path: &Path,
    pattern: Option<&str>,
    limit: usize,
) -> Result<Vec<ToolLogEntry>, String> {
    let store = open_preference_store(store_path)?;
    let entries = read_tool_log_entries(&store, store_path)?;
    let matcher = compile_log_regex(pattern)?;
    let (_, entries) = filter_log_entries(&entries, matcher.as_ref(), limit)?;
    Ok(entries)
}

fn write_tool_log_export(output_path: &Path, payload: Value) -> Result<(), String> {
    create_output_parent(output_path)?;
    fs::write(
        output_path,
        serde_json::to_vec_pretty(&payload).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())
}

fn create_output_parent(output_path: &Path) -> Result<(), String> {
    output_path
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .map(fs::create_dir_all)
        .transpose()
        .map(|_| ())
        .map_err(|error| error.to_string())
}

fn tool_log_export_payload(store_path: &Path, entries: &[ToolLogEntry]) -> Value {
    json!({
        "store_path": clean_display_path(store_path),
        "backend": "RNMDB single-file page store",
        "exported_entries": entries.len(),
        "entries": entries,
    })
}

fn read_tool_log_entries(
    store: &RnmdbSingleFilePageStore,
    path: &Path,
) -> Result<Vec<ToolLogEntry>, String> {
    match read_tool_log_index(store, path)? {
        Some(index) if !index.is_empty() => {
            decode_tool_log_entries(read_tool_log_bytes(store, path, &index)?).map_err(|error| {
                state_database_error(path, format!("failed to decode tool log entries: {error}"))
            })
        }
        _ => Ok(Vec::new()),
    }
}

fn read_tool_log_index(
    store: &RnmdbSingleFilePageStore,
    path: &Path,
) -> Result<Option<StoredToolLogIndex>, String> {
    store
        .read_payload_page(TOOL_LOG_INDEX_PAGE_ID)
        .map_err(|error| {
            state_database_error(path, format!("failed to read tool log index page: {error}"))
        })?
        .map(|payload| decode_length_prefixed_json::<StoredToolLogIndex>(&payload))
        .transpose()
        .map_err(|error| {
            state_database_error(
                path,
                format!("failed to decode tool log index page: {error}"),
            )
        })
}

fn read_tool_log_bytes(
    store: &RnmdbSingleFilePageStore,
    path: &Path,
    index: &StoredToolLogIndex,
) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::with_capacity(index.byte_len);
    for page_offset in 0..index.page_count {
        bytes.extend_from_slice(&read_tool_log_page(store, path, page_offset)?);
    }
    bytes.truncate(index.byte_len);
    Ok(bytes)
}

fn read_tool_log_page(
    store: &RnmdbSingleFilePageStore,
    path: &Path,
    page_offset: u64,
) -> Result<Vec<u8>, String> {
    let page_id = TOOL_LOG_DATA_START_PAGE_ID + page_offset;
    store
        .read_payload_page(page_id)
        .map_err(|error| {
            state_database_error(
                path,
                format!("failed to read tool log data page {page_id}: {error}"),
            )
        })?
        .ok_or_else(|| format!("stored tool log page {page_id} is missing"))
        .map_err(|error| state_database_error(path, error))
}

fn decode_tool_log_entries(bytes: Vec<u8>) -> Result<Vec<ToolLogEntry>, String> {
    serde_json::from_slice(&bytes).map_err(|error| error.to_string())
}

fn write_tool_log_entries(
    store: &RnmdbSingleFilePageStore,
    path: &Path,
    entries: &[ToolLogEntry],
) -> Result<(), String> {
    let encoded = serde_json::to_vec(entries).map_err(|error| {
        state_database_error(path, format!("failed to encode tool logs: {error}"))
    })?;
    let page_size = store.page_size_bytes();
    let page_count = encoded.len().div_ceil(page_size);

    for (index, chunk) in encoded.chunks(page_size).enumerate() {
        let mut payload = vec![0_u8; page_size];
        payload[..chunk.len()].copy_from_slice(chunk);
        let page_id = TOOL_LOG_DATA_START_PAGE_ID + index as u64;
        store
            .write_payload_page(page_id, payload)
            .map_err(|error| {
                state_database_error(
                    path,
                    format!("failed to write tool log data page {page_id}: {error}"),
                )
            })?;
    }

    let index = StoredToolLogIndex {
        schema_version: TOOL_LOG_SCHEMA_VERSION,
        byte_len: encoded.len(),
        page_count: page_count as u64,
    };
    let index_payload = encode_length_prefixed_json(&index, page_size).map_err(|error| {
        state_database_error(
            path,
            format!("failed to encode tool log index page: {error}"),
        )
    })?;
    store
        .write_payload_page(TOOL_LOG_INDEX_PAGE_ID, index_payload)
        .map_err(|error| {
            state_database_error(
                path,
                format!("failed to write tool log index page: {error}"),
            )
        })
}

fn decode_length_prefixed_json<T>(payload: &[u8]) -> Result<T, String>
where
    T: DeserializeOwned + Default,
{
    if payload.len() < 4 {
        return Ok(T::default());
    }
    let length = u32::from_be_bytes([payload[0], payload[1], payload[2], payload[3]]) as usize;
    if length == 0 {
        return Ok(T::default());
    }
    if length > payload.len().saturating_sub(4) {
        return Err("stored RNMDB tool log index has an invalid payload length".to_string());
    }
    serde_json::from_slice(&payload[4..4 + length]).map_err(|error| error.to_string())
}

fn encode_length_prefixed_json<T>(value: &T, page_size_bytes: usize) -> Result<Vec<u8>, String>
where
    T: Serialize,
{
    let encoded = serde_json::to_vec(value).map_err(|error| error.to_string())?;
    if encoded.len() + 4 > page_size_bytes {
        return Err(format!(
            "tool log index is too large for the RNMDB page: {} bytes > {} bytes",
            encoded.len() + 4,
            page_size_bytes
        ));
    }

    let mut payload = vec![0_u8; page_size_bytes];
    payload[..4].copy_from_slice(&(encoded.len() as u32).to_be_bytes());
    payload[4..4 + encoded.len()].copy_from_slice(&encoded);
    Ok(payload)
}

fn compile_log_regex(pattern: Option<&str>) -> Result<Option<Regex>, String> {
    pattern
        .filter(|pattern| !pattern.trim().is_empty())
        .map(Regex::new)
        .transpose()
        .map_err(|error| error.to_string())
}

fn filter_log_entries(
    entries: &[ToolLogEntry],
    matcher: Option<&Regex>,
    limit: usize,
) -> Result<(usize, Vec<ToolLogEntry>), String> {
    let mut matched_entries = 0;
    let mut filtered = Vec::new();

    for entry in entries.iter().rev() {
        if !matches_log_entry(entry, matcher)? {
            continue;
        }
        matched_entries += 1;
        if filtered.len() < limit {
            filtered.push(entry.clone());
        }
    }

    Ok((matched_entries, filtered))
}

fn matches_log_entry(entry: &ToolLogEntry, matcher: Option<&Regex>) -> Result<bool, String> {
    let Some(matcher) = matcher else {
        return Ok(true);
    };
    let text = serde_json::to_string(entry).map_err(|error| error.to_string())?;
    Ok(matcher.is_match(&text))
}

fn query_limit(limit: Option<usize>, default_limit: usize) -> usize {
    limit
        .unwrap_or(default_limit)
        .clamp(1, MAX_TOOL_LOG_ENTRIES)
}

fn next_sequence(entries: &[ToolLogEntry]) -> u64 {
    entries
        .last()
        .map(|entry| entry.sequence.saturating_add(1))
        .unwrap_or(1)
}

fn unix_timestamp_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn trim_old_entries(entries: &mut Vec<ToolLogEntry>) {
    let overflow = entries.len().saturating_sub(MAX_TOOL_LOG_ENTRIES);
    if overflow > 0 {
        entries.drain(..overflow);
    }
}

fn compact_json_value(value: Value) -> Value {
    let Ok(encoded) = serde_json::to_string(&value) else {
        return value;
    };
    if encoded.chars().count() <= MAX_LOGGED_JSON_CHARS {
        return value;
    }

    let preview = encoded
        .chars()
        .take(MAX_LOGGED_JSON_CHARS)
        .collect::<String>();
    json!({
        "truncated": true,
        "original_json_chars": encoded.chars().count(),
        "preview": preview,
    })
}
