//------------------------------------------------------------------------------------
// state/store/records.rs -- Part of RHoiScribe
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

use rnmdb_cli::CommandOutput;
use rnmdb_types::SqlValue;
use serde_json::Value;

use super::sql::{log_search_text, sql_i64};
use crate::state::{
    StoredPreferenceRecord, StoredToolLogRecord, StoredToolLogSearchRow,
    scope::validate_stored_scope, stored_preference_record_key,
};

struct DecodedScope {
    scope_kind: String,
    scope_key: String,
    mod_root: Option<String>,
}

struct DecodedPreferenceIdentity {
    record_key: String,
    scope: DecodedScope,
}

struct DecodedLogIdentity {
    sequence: u64,
    timestamp_unix_seconds: u64,
    scope: DecodedScope,
}

struct DecodedLogPayload {
    tool_name: String,
    arguments_json: String,
    success: bool,
    result_json: Option<String>,
    error_text: Option<String>,
}

pub(super) fn validate_import(
    preferences: &[StoredPreferenceRecord],
    logs: &[StoredToolLogRecord],
) -> Result<(), String> {
    for preference in preferences {
        validate_preference(preference)?;
    }
    for log in logs {
        validate_log(log)?;
    }
    Ok(())
}

pub(super) fn validate_preference(record: &StoredPreferenceRecord) -> Result<(), String> {
    validate_preference_identity(record)?;
    validate_json(&record.value_json, "preference value")?;
    sql_i64(record.updated_at_unix_seconds, "preference timestamp").map(|_| ())
}

fn validate_preference_identity(record: &StoredPreferenceRecord) -> Result<(), String> {
    validate_stored_scope(
        &record.scope_kind,
        &record.scope_key,
        record.mod_root.as_deref(),
    )?;
    let expected = stored_preference_record_key(
        &record.scope_kind,
        &record.scope_key,
        &record.preference_key,
    );
    if record.record_key != expected {
        return Err("preference record_key does not match its scope and key".to_string());
    }
    Ok(())
}

pub(super) fn validate_log(record: &StoredToolLogRecord) -> Result<(), String> {
    validate_stored_scope(
        &record.scope_kind,
        &record.scope_key,
        record.mod_root.as_deref(),
    )?;
    if record.tool_name.trim().is_empty() {
        return Err("tool log name must not be empty".to_string());
    }
    validate_json(&record.arguments_json, "tool log arguments")?;
    if let Some(result) = &record.result_json {
        validate_json(result, "tool log result")?;
    }
    sql_i64(record.sequence, "tool log sequence")?;
    sql_i64(record.timestamp_unix_seconds, "tool log timestamp").map(|_| ())
}

fn validate_json(value: &str, label: &str) -> Result<(), String> {
    serde_json::from_str::<Value>(value)
        .map(|_| ())
        .map_err(|error| format!("{label} is not valid JSON: {error}"))
}

pub(super) fn decode_preference(values: &[SqlValue]) -> Result<StoredPreferenceRecord, String> {
    let identity = decode_preference_identity(values)?;
    let record = StoredPreferenceRecord {
        record_key: identity.record_key,
        scope_kind: identity.scope.scope_kind,
        scope_key: identity.scope.scope_key,
        mod_root: identity.scope.mod_root,
        preference_key: required_text(values, 4, "agent_preferences.preference_key")?,
        value_json: required_json(values, 5, "agent_preferences.value_json")?,
        updated_at_unix_seconds: required_u64(
            values,
            6,
            "agent_preferences.updated_at_unix_seconds",
        )?,
    };
    validate_preference(&record)?;
    Ok(record)
}

fn decode_preference_identity(values: &[SqlValue]) -> Result<DecodedPreferenceIdentity, String> {
    Ok(DecodedPreferenceIdentity {
        record_key: required_text(values, 0, "agent_preferences.record_key")?,
        scope: decode_scope(values, 1, 2, 3, "agent_preferences")?,
    })
}

fn decode_log(values: &[SqlValue]) -> Result<StoredToolLogRecord, String> {
    let identity = decode_log_identity(values)?;
    let payload = decode_log_payload(values)?;
    let record = StoredToolLogRecord {
        sequence: identity.sequence,
        timestamp_unix_seconds: identity.timestamp_unix_seconds,
        scope_kind: identity.scope.scope_kind,
        scope_key: identity.scope.scope_key,
        mod_root: identity.scope.mod_root,
        tool_name: payload.tool_name,
        arguments_json: payload.arguments_json,
        success: payload.success,
        result_json: payload.result_json,
        error_text: payload.error_text,
    };
    validate_log(&record)?;
    Ok(record)
}

fn decode_log_identity(values: &[SqlValue]) -> Result<DecodedLogIdentity, String> {
    Ok(DecodedLogIdentity {
        sequence: required_u64(values, 0, "tool_logs.sequence")?,
        timestamp_unix_seconds: required_u64(values, 1, "tool_logs.timestamp_unix_seconds")?,
        scope: decode_scope(values, 2, 3, 4, "tool_logs")?,
    })
}

fn decode_log_payload(values: &[SqlValue]) -> Result<DecodedLogPayload, String> {
    Ok(DecodedLogPayload {
        tool_name: required_text(values, 5, "tool_logs.tool_name")?,
        arguments_json: required_json(values, 6, "tool_logs.arguments_json")?,
        success: required_bool(values, 7, "tool_logs.success")?,
        result_json: optional_json(values, 8, "tool_logs.result_json")?,
        error_text: optional_text(values, 9, "tool_logs.error_text")?,
    })
}

fn decode_scope(
    values: &[SqlValue],
    kind_index: usize,
    key_index: usize,
    root_index: usize,
    table: &str,
) -> Result<DecodedScope, String> {
    Ok(DecodedScope {
        scope_kind: required_text(values, kind_index, &format!("{table}.scope_kind"))?,
        scope_key: required_text(values, key_index, &format!("{table}.scope_key"))?,
        mod_root: optional_text(values, root_index, &format!("{table}.mod_root"))?,
    })
}

pub(super) fn decode_log_search_row(values: &[SqlValue]) -> Result<StoredToolLogSearchRow, String> {
    let record = decode_log(values)?;
    let search_text = optional_text(values, 10, "tool_logs.search_text")?
        .filter(|text| !text.trim().is_empty())
        .unwrap_or_else(|| log_search_text(&record));
    Ok(StoredToolLogSearchRow {
        record,
        search_text,
    })
}

pub(super) fn required_text(
    values: &[SqlValue],
    index: usize,
    label: &str,
) -> Result<String, String> {
    match values.get(index) {
        Some(SqlValue::Text(value)) => Ok(value.clone()),
        _ => Err(format!("{label} is missing or not TEXT")),
    }
}

fn optional_text(values: &[SqlValue], index: usize, label: &str) -> Result<Option<String>, String> {
    match values.get(index) {
        Some(SqlValue::Null) => Ok(None),
        Some(SqlValue::Text(value)) => Ok(Some(value.clone())),
        _ => Err(format!("{label} is missing or not nullable TEXT")),
    }
}

fn required_json(values: &[SqlValue], index: usize, label: &str) -> Result<String, String> {
    let value = required_text(values, index, label)?;
    validate_json(&value, label)?;
    Ok(value)
}

fn optional_json(values: &[SqlValue], index: usize, label: &str) -> Result<Option<String>, String> {
    let value = optional_text(values, index, label)?;
    if let Some(json) = &value {
        validate_json(json, label)?;
    }
    Ok(value)
}

pub(super) fn required_u64(values: &[SqlValue], index: usize, label: &str) -> Result<u64, String> {
    match values.get(index) {
        Some(SqlValue::Int64(value)) => {
            u64::try_from(*value).map_err(|_| format!("{label} must not be negative"))
        }
        _ => Err(format!("{label} is missing or not INT64")),
    }
}

pub(super) fn optional_u64(
    values: &[SqlValue],
    index: usize,
    label: &str,
) -> Result<Option<u64>, String> {
    match values.get(index) {
        Some(SqlValue::Null) => Ok(None),
        Some(SqlValue::Int64(value)) => u64::try_from(*value)
            .map(Some)
            .map_err(|_| format!("{label} must not be negative")),
        _ => Err(format!("{label} is missing or not nullable INT64")),
    }
}

fn required_bool(values: &[SqlValue], index: usize, label: &str) -> Result<bool, String> {
    match values.get(index) {
        Some(SqlValue::Bool(value)) => Ok(*value),
        _ => Err(format!("{label} is missing or not BOOL")),
    }
}

pub(super) fn rows_affected(output: CommandOutput) -> Option<u64> {
    match output {
        CommandOutput::RowsAffected(count) => Some(count),
        _ => None,
    }
}

pub(super) fn row_count(output: CommandOutput) -> Option<usize> {
    match output {
        CommandOutput::Rows(batch) => Some(batch.rows().len()),
        _ => None,
    }
}

pub(super) fn decode_count(output: CommandOutput, label: &str) -> Result<usize, String> {
    let CommandOutput::Rows(batch) = output else {
        return Err(format!("{label} returned an unexpected command result"));
    };
    let value = batch
        .rows()
        .first()
        .ok_or_else(|| format!("{label} returned no row"))
        .and_then(|row| required_u64(row.values(), 0, label))?;
    usize::try_from(value).map_err(|_| format!("{label} exceeds usize range"))
}
