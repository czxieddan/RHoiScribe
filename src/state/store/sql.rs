//------------------------------------------------------------------------------------
// state/store/sql.rs -- Part of RHoiScribe
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

use crate::state::{StateScope, StoredPreferenceRecord, StoredToolLogFilter, StoredToolLogRecord};

pub(super) fn preference_insert_sql(record: &StoredPreferenceRecord) -> Result<String, String> {
    let updated_at = sql_i64(record.updated_at_unix_seconds, "preference timestamp")?;
    Ok(format!(
        "INSERT INTO agent_preferences (record_key, scope_kind, scope_key, mod_root, preference_key, value_json, updated_at_unix_seconds) VALUES ({}, {}, {}, {}, {}, {}, {updated_at});",
        sql_text(&record.record_key),
        sql_text(&record.scope_kind),
        sql_text(&record.scope_key),
        sql_optional_text(record.mod_root.as_deref()),
        sql_text(&record.preference_key),
        sql_text(&record.value_json),
    ))
}

pub(super) fn log_insert_sql(record: &StoredToolLogRecord) -> Result<String, String> {
    let sequence = sql_i64(record.sequence, "tool log sequence")?;
    let timestamp = sql_i64(record.timestamp_unix_seconds, "tool log timestamp")?;
    let success = if record.success { "TRUE" } else { "FALSE" };
    let search_text = log_search_text(record);
    Ok(format!(
        "INSERT INTO tool_logs (sequence, timestamp_unix_seconds, scope_kind, scope_key, mod_root, tool_name, arguments_json, success, result_json, error_text, search_text) VALUES ({sequence}, {timestamp}, {}, {}, {}, {}, {}, {success}, {}, {}, {});",
        sql_text(&record.scope_kind),
        sql_text(&record.scope_key),
        sql_optional_text(record.mod_root.as_deref()),
        sql_text(&record.tool_name),
        sql_text(&record.arguments_json),
        sql_optional_text(record.result_json.as_deref()),
        sql_optional_text(record.error_text.as_deref()),
        sql_text(&search_text),
    ))
}

pub(super) fn log_search_text(record: &StoredToolLogRecord) -> String {
    format!(
        "{}\n{}\n{}\n{}\n{}\n{}",
        record.tool_name,
        record.scope_kind,
        record.mod_root.as_deref().unwrap_or(&record.scope_key),
        record.arguments_json,
        record.result_json.as_deref().unwrap_or(""),
        record.error_text.as_deref().unwrap_or("")
    )
}

pub(super) fn log_filter_where_clause(filter: &StoredToolLogFilter) -> Result<String, String> {
    let predicates = [
        log_scope_predicate(filter.scope.as_ref()),
        log_text_predicate("tool_name", filter.tool_name.as_deref()),
        log_success_predicate(filter.success),
        log_time_predicate(">=", filter.since_unix_seconds)?,
        log_time_predicate("<=", filter.until_unix_seconds)?,
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();
    Ok(where_clause(predicates))
}

fn log_scope_predicate(scope: Option<&StateScope>) -> Option<String> {
    scope.map(|scope| {
        format!(
            "(scope_kind = {} AND scope_key = {})",
            sql_text(scope.kind()),
            sql_text(scope.key())
        )
    })
}

fn log_text_predicate(column: &str, value: Option<&str>) -> Option<String> {
    value.map(|value| format!("{column} = {}", sql_text(value)))
}

fn log_success_predicate(success: Option<bool>) -> Option<String> {
    success.map(|success| format!("success = {}", if success { "TRUE" } else { "FALSE" }))
}

fn log_time_predicate(operator: &str, value: Option<u64>) -> Result<Option<String>, String> {
    value
        .map(|value| {
            sql_i64(value, "tool log filter timestamp")
                .map(|value| format!("timestamp_unix_seconds {operator} {value}"))
        })
        .transpose()
}

fn where_clause(predicates: Vec<String>) -> String {
    if predicates.is_empty() {
        String::new()
    } else {
        format!(" WHERE {}", predicates.join(" AND "))
    }
}

pub(super) fn sql_i64(value: u64, label: &str) -> Result<i64, String> {
    i64::try_from(value).map_err(|_| format!("{label} exceeds RNMDB INT64 range"))
}

pub(super) fn sql_usize(value: usize, label: &str) -> Result<i64, String> {
    let value = u64::try_from(value).map_err(|_| format!("{label} exceeds u64 range"))?;
    sql_i64(value, label)
}

fn sql_optional_text(value: Option<&str>) -> String {
    value.map(sql_text).unwrap_or_else(|| "NULL".to_string())
}

pub(super) fn sql_text(value: &str) -> String {
    let escaped = value.replace('\'', "''");
    format!("'{escaped}'")
}
