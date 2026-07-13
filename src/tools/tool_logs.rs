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
use rnmdb_fts::{SimpleTokenizer, TextQuery, TextVectorBuilder};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

use crate::state::{
    RnmdbStateStore, StateScope, StoredToolLogFilter, StoredToolLogRecord, StoredToolLogSearchRow,
    clean_display_path, state_database_error,
};

use super::preferences::preference_store_path;

pub(crate) const MAX_TOOL_LOG_ENTRIES: usize = 32767;

const DEFAULT_TOOL_LOG_LIMIT: usize = 100;
const MAX_LOGGED_JSON_CHARS: usize = 8192;
const BACKEND_NAME: &str = "RNMDB single-file page store";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolLogEntry {
    pub sequence: u64,
    pub timestamp_unix_seconds: u64,
    pub scope_kind: String,
    pub mod_root: Option<String>,
    pub tool_name: String,
    pub arguments: Value,
    pub success: bool,
    pub result: Option<Value>,
    pub error: Option<String>,
    pub text_rank: Option<ToolLogTextRank>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolLogTextRank {
    pub score: u32,
    pub first_position: Option<u32>,
    pub matched_terms: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ToolLogQueryRequest {
    pub store_path: Option<String>,
    pub mod_root: Option<String>,
    pub tool_name: Option<String>,
    pub success: Option<bool>,
    pub since_unix_seconds: Option<u64>,
    pub until_unix_seconds: Option<u64>,
    pub text_query: Option<String>,
    pub pattern: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolLogExportRequest {
    pub store_path: Option<String>,
    pub output_path: String,
    pub mod_root: Option<String>,
    pub tool_name: Option<String>,
    pub success: Option<bool>,
    pub since_unix_seconds: Option<u64>,
    pub until_unix_seconds: Option<u64>,
    pub text_query: Option<String>,
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

struct ToolLogSelectionInput<'a> {
    store_path: Option<&'a str>,
    mod_root: Option<&'a str>,
    tool_name: Option<&'a str>,
    success: Option<bool>,
    since_unix_seconds: Option<u64>,
    until_unix_seconds: Option<u64>,
    text_query: Option<&'a str>,
    pattern: Option<&'a str>,
    limit: Option<usize>,
}

struct ValidatedLogSelection {
    filter: StoredToolLogFilter,
    matcher: Option<Regex>,
    text_query: Option<TextQuery>,
    limit: usize,
}

struct SelectedToolLogs {
    store_path: std::path::PathBuf,
    total_entries: usize,
    matched_entries: usize,
    entries: Vec<ToolLogEntry>,
    migration_message: Option<String>,
}

pub(crate) fn tool_log_store_path_from_arguments(
    tool_name: &str,
    arguments: &Map<String, Value>,
) -> Option<String> {
    if tool_name == "launch_hoi4_debug_with_rchadow" {
        return None;
    }
    arguments
        .get("store_path")
        .and_then(Value::as_str)
        .map(str::to_string)
}

pub(crate) fn record_tool_call(
    store_path: Option<&str>,
    append: ToolLogAppend,
) -> Result<Option<String>, String> {
    let store_path = preference_store_path(store_path);
    let record = stored_log_record(append, &store_path)?;
    let mut store = RnmdbStateStore::open(&store_path)?;
    store.append_log(record, MAX_TOOL_LOG_ENTRIES)?;
    Ok(store
        .take_migration_report()
        .map(|report| report.retained_backup_message()))
}

pub fn query_tool_logs(request: ToolLogQueryRequest) -> Result<ToolLogQueryResult, String> {
    let selected = select_tool_logs(query_selection_input(&request), DEFAULT_TOOL_LOG_LIMIT)?;
    let mut messages = vec![
        "tool logs share the same RNMDB database as agent preferences".to_string(),
        format!("only the latest {MAX_TOOL_LOG_ENTRIES} entries are retained"),
    ];
    append_migration_message(&mut messages, selected.migration_message);
    Ok(ToolLogQueryResult {
        store_path: clean_display_path(&selected.store_path),
        backend: BACKEND_NAME.to_string(),
        total_entries: selected.total_entries,
        matched_entries: selected.matched_entries,
        entries: selected.entries,
        messages,
    })
}

pub fn export_tool_logs(request: ToolLogExportRequest) -> Result<ToolLogExportResult, String> {
    let selected = select_tool_logs(export_selection_input(&request), MAX_TOOL_LOG_ENTRIES)?;
    let output_path = Path::new(&request.output_path);
    write_tool_log_export(
        output_path,
        tool_log_export_payload(&selected.store_path, &selected.entries),
    )?;
    let mut messages = vec!["matching tool logs exported as JSON".to_string()];
    append_migration_message(&mut messages, selected.migration_message);
    Ok(ToolLogExportResult {
        store_path: clean_display_path(&selected.store_path),
        output_path: clean_display_path(output_path),
        exported_entries: selected.entries.len(),
        messages,
    })
}

fn query_selection_input(request: &ToolLogQueryRequest) -> ToolLogSelectionInput<'_> {
    ToolLogSelectionInput {
        store_path: request.store_path.as_deref(),
        mod_root: request.mod_root.as_deref(),
        tool_name: request.tool_name.as_deref(),
        success: request.success,
        since_unix_seconds: request.since_unix_seconds,
        until_unix_seconds: request.until_unix_seconds,
        text_query: request.text_query.as_deref(),
        pattern: request.pattern.as_deref(),
        limit: request.limit,
    }
}

fn export_selection_input(request: &ToolLogExportRequest) -> ToolLogSelectionInput<'_> {
    ToolLogSelectionInput {
        store_path: request.store_path.as_deref(),
        mod_root: request.mod_root.as_deref(),
        tool_name: request.tool_name.as_deref(),
        success: request.success,
        since_unix_seconds: request.since_unix_seconds,
        until_unix_seconds: request.until_unix_seconds,
        text_query: request.text_query.as_deref(),
        pattern: request.pattern.as_deref(),
        limit: request.limit,
    }
}

fn select_tool_logs(
    input: ToolLogSelectionInput<'_>,
    default_limit: usize,
) -> Result<SelectedToolLogs, String> {
    let validated = validate_log_selection(&input, default_limit)?;
    let store_path = preference_store_path(input.store_path);
    let mut store = RnmdbStateStore::open(&store_path)?;
    let migration_message = store
        .take_migration_report()
        .map(|report| report.retained_backup_message());
    let total_entries = store.count_logs()?;
    let rows = store.search_logs(&validated.filter)?;
    drop(store);
    let mut entries = filter_and_rank_logs(rows, &validated, &store_path)?;
    let matched_entries = entries.len();
    sort_log_entries(&mut entries, validated.text_query.is_some());
    entries.truncate(validated.limit);
    Ok(SelectedToolLogs {
        store_path,
        total_entries,
        matched_entries,
        entries,
        migration_message,
    })
}

fn validate_log_selection(
    input: &ToolLogSelectionInput<'_>,
    default_limit: usize,
) -> Result<ValidatedLogSelection, String> {
    validate_time_bounds(input.since_unix_seconds, input.until_unix_seconds)?;
    Ok(ValidatedLogSelection {
        filter: StoredToolLogFilter {
            scope: explicit_log_scope(input.mod_root)?,
            tool_name: validated_tool_name(input.tool_name)?,
            success: input.success,
            since_unix_seconds: input.since_unix_seconds,
            until_unix_seconds: input.until_unix_seconds,
        },
        matcher: compile_log_regex(input.pattern)?,
        text_query: parse_text_query(input.text_query)?,
        limit: query_limit(input.limit, default_limit),
    })
}

fn validate_time_bounds(since: Option<u64>, until: Option<u64>) -> Result<(), String> {
    validate_filter_timestamp(since, "since_unix_seconds")?;
    validate_filter_timestamp(until, "until_unix_seconds")?;
    if since.zip(until).is_some_and(|(since, until)| since > until) {
        Err("since_unix_seconds must be less than or equal to until_unix_seconds".to_string())
    } else {
        Ok(())
    }
}

fn validate_filter_timestamp(value: Option<u64>, name: &str) -> Result<(), String> {
    value
        .map(|value| {
            i64::try_from(value)
                .map(|_| ())
                .map_err(|_| format!("{name} exceeds RNMDB INT64 range"))
        })
        .unwrap_or(Ok(()))
}

fn explicit_log_scope(mod_root: Option<&str>) -> Result<Option<StateScope>, String> {
    mod_root
        .map(|root| StateScope::from_mod_root(Some(root)))
        .transpose()
}

fn validated_tool_name(tool_name: Option<&str>) -> Result<Option<String>, String> {
    tool_name
        .map(str::trim)
        .map(|name| {
            if name.is_empty() {
                Err("tool_name must not be empty".to_string())
            } else {
                Ok(name.to_string())
            }
        })
        .transpose()
}

fn parse_text_query(text_query: Option<&str>) -> Result<Option<TextQuery>, String> {
    text_query
        .map(TextQuery::parse)
        .transpose()
        .map_err(|error| format!("invalid text_query: {error}"))
}

fn filter_and_rank_logs(
    rows: Vec<StoredToolLogSearchRow>,
    selection: &ValidatedLogSelection,
    store_path: &Path,
) -> Result<Vec<ToolLogEntry>, String> {
    let builder = TextVectorBuilder::new(SimpleTokenizer::new());
    let mut entries = Vec::new();
    for row in rows {
        if let Some(entry) = select_log_entry(row, selection, &builder, store_path)? {
            entries.push(entry);
        }
    }
    Ok(entries)
}

fn select_log_entry(
    row: StoredToolLogSearchRow,
    selection: &ValidatedLogSelection,
    builder: &TextVectorBuilder<SimpleTokenizer>,
    store_path: &Path,
) -> Result<Option<ToolLogEntry>, String> {
    let mut entry = tool_log_entry(row.record, store_path)?;
    if !matches_log_entry(&entry, selection.matcher.as_ref())? {
        return Ok(None);
    }
    let Some(query) = selection.text_query.as_ref() else {
        return Ok(Some(entry));
    };
    let vector = builder
        .build(&row.search_text)
        .map_err(|error| state_database_error(store_path, "query", error.to_string()))?;
    let Some(rank) = query.rank(&vector) else {
        return Ok(None);
    };
    entry.text_rank = Some(ToolLogTextRank {
        score: rank.score(),
        first_position: rank.first_position(),
        matched_terms: rank.matched_terms().to_vec(),
    });
    Ok(Some(entry))
}

fn sort_log_entries(entries: &mut [ToolLogEntry], ranked: bool) {
    if ranked {
        entries.sort_by(compare_ranked_logs);
    } else {
        entries.sort_by_key(|entry| std::cmp::Reverse(entry.sequence));
    }
}

fn compare_ranked_logs(left: &ToolLogEntry, right: &ToolLogEntry) -> std::cmp::Ordering {
    let left_score = left.text_rank.as_ref().map(|rank| rank.score).unwrap_or(0);
    let right_score = right.text_rank.as_ref().map(|rank| rank.score).unwrap_or(0);
    right_score
        .cmp(&left_score)
        .then_with(|| right.sequence.cmp(&left.sequence))
}

fn append_migration_message(messages: &mut Vec<String>, migration: Option<String>) {
    if let Some(message) = migration {
        messages.push(message);
    }
}

fn stored_log_record(
    append: ToolLogAppend,
    store_path: &Path,
) -> Result<StoredToolLogRecord, String> {
    let scope = infer_log_scope(&append.arguments);
    let arguments = compact_json_value(append.arguments);
    let result = append.result.map(compact_json_value);
    Ok(StoredToolLogRecord {
        sequence: 0,
        timestamp_unix_seconds: unix_timestamp_now(),
        scope_kind: scope.kind().to_string(),
        scope_key: scope.key().to_string(),
        mod_root: scope.mod_root_text(),
        tool_name: append.tool_name,
        arguments_json: encode_json(&arguments, store_path)?,
        success: append.success,
        result_json: result
            .as_ref()
            .map(|value| encode_json(value, store_path))
            .transpose()?,
        error_text: append.error,
    })
}

fn infer_log_scope(arguments: &Value) -> StateScope {
    arguments
        .as_object()
        .and_then(inferred_mod_scope)
        .unwrap_or(StateScope::Global)
}

fn inferred_mod_scope(arguments: &Map<String, Value>) -> Option<StateScope> {
    argument_mod_scope(arguments, "mod_root")
        .or_else(|| argument_mod_scope(arguments, "workspace_root"))
        .or_else(|| argument_mod_scope(arguments, "workspace_mod_path"))
        .or_else(|| roots_mod_scope(arguments.get("roots")))
}

fn argument_mod_scope(arguments: &Map<String, Value>, name: &str) -> Option<StateScope> {
    arguments
        .get(name)
        .and_then(Value::as_str)
        .and_then(existing_mod_scope)
}

fn roots_mod_scope(roots: Option<&Value>) -> Option<StateScope> {
    roots
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_object)
        .filter(|root| is_mod_root(root))
        .filter_map(|root| root.get("path").and_then(Value::as_str))
        .find_map(existing_mod_scope)
}

fn is_mod_root(root: &Map<String, Value>) -> bool {
    ["role", "kind"]
        .into_iter()
        .filter_map(|name| root.get(name).and_then(Value::as_str))
        .any(|role| role == "mod")
}

fn existing_mod_scope(root: &str) -> Option<StateScope> {
    StateScope::from_mod_root(Some(root)).ok()
}

fn tool_log_entry(record: StoredToolLogRecord, store_path: &Path) -> Result<ToolLogEntry, String> {
    Ok(ToolLogEntry {
        sequence: record.sequence,
        timestamp_unix_seconds: record.timestamp_unix_seconds,
        scope_kind: record.scope_kind,
        mod_root: record.mod_root,
        tool_name: record.tool_name,
        arguments: decode_json(&record.arguments_json, store_path)?,
        success: record.success,
        result: record
            .result_json
            .as_deref()
            .map(|value| decode_json(value, store_path))
            .transpose()?,
        error: record.error_text,
        text_rank: None,
    })
}

fn encode_json(value: &Value, store_path: &Path) -> Result<String, String> {
    serde_json::to_string(value)
        .map_err(|error| state_database_error(store_path, "query", error.to_string()))
}

fn decode_json(value: &str, store_path: &Path) -> Result<Value, String> {
    serde_json::from_str(value)
        .map_err(|error| state_database_error(store_path, "query", error.to_string()))
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
        "backend": BACKEND_NAME,
        "exported_entries": entries.len(),
        "entries": entries,
    })
}

fn compile_log_regex(pattern: Option<&str>) -> Result<Option<Regex>, String> {
    pattern
        .filter(|pattern| !pattern.trim().is_empty())
        .map(Regex::new)
        .transpose()
        .map_err(|error| format!("invalid pattern: {error}"))
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

fn unix_timestamp_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
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
