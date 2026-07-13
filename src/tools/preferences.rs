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
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::state::{
    GLOBAL_SCOPE_KIND, MOD_SCOPE_KIND, RnmdbStateStore, StateScope, StoredPreferenceRecord,
    clean_display_path, preference_record_key, state_database_error, state_store_path,
};

const BACKEND_NAME: &str = "RNMDB single-file page store";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ListAgentPreferencesRequest {
    pub store_path: Option<String>,
    pub mod_root: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SetAgentPreferenceRequest {
    pub key: String,
    pub value: Value,
    pub store_path: Option<String>,
    pub mod_root: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DeleteAgentPreferenceRequest {
    pub key: String,
    pub store_path: Option<String>,
    pub mod_root: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AgentPreferenceProvenance {
    Global,
    Mod,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentPreferenceItem {
    pub key: String,
    pub value: Value,
    pub scope_kind: String,
    pub mod_root: Option<String>,
    pub provenance: AgentPreferenceProvenance,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentPreferencesResult {
    pub store_path: String,
    pub backend: String,
    pub preferences: Vec<AgentPreferenceItem>,
    pub global_preferences: Vec<AgentPreferenceItem>,
    pub mod_preferences: Vec<AgentPreferenceItem>,
    pub mod_root: Option<String>,
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
    pub global_preferences: Vec<AgentPreferenceItem>,
    pub mod_preferences: Vec<AgentPreferenceItem>,
    pub mod_root: Option<String>,
    pub messages: Vec<String>,
}

struct PreferenceViews {
    preferences: Vec<AgentPreferenceItem>,
    global_preferences: Vec<AgentPreferenceItem>,
    mod_preferences: Vec<AgentPreferenceItem>,
    mod_root: Option<String>,
}

pub fn list_agent_preferences(
    request: ListAgentPreferencesRequest,
) -> Result<AgentPreferencesResult, String> {
    let scope = StateScope::from_mod_root(request.mod_root.as_deref())?;
    let store_path = preference_store_path(request.store_path.as_deref());
    let mut store = RnmdbStateStore::open(&store_path)?;
    let migration_message = take_migration_message(&mut store);
    let views = preference_views(&mut store, &scope, &store_path)?;
    Ok(AgentPreferencesResult {
        store_path: clean_display_path(&store_path),
        backend: BACKEND_NAME.to_string(),
        preferences: views.preferences,
        global_preferences: views.global_preferences,
        mod_preferences: views.mod_preferences,
        mod_root: views.mod_root,
        messages: result_messages(
            "preferences are shared across MCP clients and IDEs through .rhoiscribe",
            migration_message,
        ),
    })
}

pub fn set_agent_preference(
    request: SetAgentPreferenceRequest,
) -> Result<AgentPreferenceMutationResult, String> {
    let scope = StateScope::from_mod_root(request.mod_root.as_deref())?;
    let key = normalized_preference_key(&request.key)?;
    let store_path = preference_store_path(request.store_path.as_deref());
    let mut store = RnmdbStateStore::open(&store_path)?;
    let migration_message = take_migration_message(&mut store);
    let record = stored_preference(&scope, &key, &request.value, &store_path)?;
    store.upsert_preference(&record)?;
    let views = preference_views(&mut store, &scope, &store_path)?;
    Ok(AgentPreferenceMutationResult {
        store_path: clean_display_path(&store_path),
        backend: BACKEND_NAME.to_string(),
        key,
        removed: false,
        value: Some(request.value),
        preferences: views.preferences,
        global_preferences: views.global_preferences,
        mod_preferences: views.mod_preferences,
        mod_root: views.mod_root,
        messages: result_messages(
            "preference stored in RNMDB-backed .rhoiscribe state",
            migration_message,
        ),
    })
}

pub fn delete_agent_preference(
    request: DeleteAgentPreferenceRequest,
) -> Result<AgentPreferenceMutationResult, String> {
    let scope = StateScope::from_mod_root(request.mod_root.as_deref())?;
    let key = normalized_preference_key(&request.key)?;
    let store_path = preference_store_path(request.store_path.as_deref());
    let mut store = RnmdbStateStore::open(&store_path)?;
    let migration_message = take_migration_message(&mut store);
    let records = store.list_preferences(&scope)?;
    let value = preference_value(&records, &key, &store_path)?;
    let removed = store.delete_preference(&preference_record_key(&scope, &key))?;
    let views = preference_views(&mut store, &scope, &store_path)?;
    Ok(AgentPreferenceMutationResult {
        store_path: clean_display_path(&store_path),
        backend: BACKEND_NAME.to_string(),
        key,
        removed,
        value,
        preferences: views.preferences,
        global_preferences: views.global_preferences,
        mod_preferences: views.mod_preferences,
        mod_root: views.mod_root,
        messages: result_messages("preference state updated in RNMDB", migration_message),
    })
}

pub(crate) fn preference_store_path(store_path: Option<&str>) -> PathBuf {
    state_store_path(store_path)
}

pub(crate) fn is_state_database_error(error: &str) -> bool {
    crate::state::is_state_database_error(error)
}

fn take_migration_message(store: &mut RnmdbStateStore) -> Option<String> {
    store
        .take_migration_report()
        .map(|report| report.retained_backup_message())
}

fn result_messages(primary: &str, migration: Option<String>) -> Vec<String> {
    let mut messages = vec![primary.to_string()];
    if let Some(message) = migration {
        messages.push(message);
    }
    messages
}

fn stored_preference(
    scope: &StateScope,
    key: &str,
    value: &Value,
    store_path: &std::path::Path,
) -> Result<StoredPreferenceRecord, String> {
    let value_json = serde_json::to_string(value)
        .map_err(|error| state_database_error(store_path, "query", error.to_string()))?;
    Ok(StoredPreferenceRecord {
        record_key: preference_record_key(scope, key),
        scope_kind: scope.kind().to_string(),
        scope_key: scope.key().to_string(),
        mod_root: scope.mod_root_text(),
        preference_key: key.to_string(),
        value_json,
        updated_at_unix_seconds: unix_timestamp_now(),
    })
}

fn preference_items(
    records: Vec<StoredPreferenceRecord>,
    store_path: &std::path::Path,
) -> Result<Vec<AgentPreferenceItem>, String> {
    let mut items = records
        .into_iter()
        .map(|record| preference_item(record, store_path))
        .collect::<Result<Vec<_>, _>>()?;
    items.sort_by(|left, right| left.key.cmp(&right.key));
    Ok(items)
}

fn preference_item(
    record: StoredPreferenceRecord,
    store_path: &std::path::Path,
) -> Result<AgentPreferenceItem, String> {
    let value = decode_preference_value(&record.value_json, store_path)?;
    let provenance = preference_provenance(&record.scope_kind, store_path)?;
    Ok(AgentPreferenceItem {
        key: record.preference_key,
        value,
        scope_kind: record.scope_kind,
        mod_root: record.mod_root,
        provenance,
    })
}

fn preference_views(
    store: &mut RnmdbStateStore,
    scope: &StateScope,
    store_path: &std::path::Path,
) -> Result<PreferenceViews, String> {
    let global_preferences =
        preference_items(store.list_preferences(&StateScope::Global)?, store_path)?;
    let mod_preferences = match scope {
        StateScope::Global => Vec::new(),
        StateScope::Mod { .. } => preference_items(store.list_preferences(scope)?, store_path)?,
    };
    let preferences = effective_preferences(&global_preferences, &mod_preferences);
    Ok(PreferenceViews {
        preferences,
        global_preferences,
        mod_preferences,
        mod_root: scope.mod_root_text(),
    })
}

fn effective_preferences(
    global: &[AgentPreferenceItem],
    scoped: &[AgentPreferenceItem],
) -> Vec<AgentPreferenceItem> {
    let mut effective = BTreeMap::new();
    for item in global.iter().chain(scoped) {
        effective.insert(item.key.clone(), item.clone());
    }
    effective.into_values().collect()
}

fn preference_provenance(
    scope_kind: &str,
    store_path: &std::path::Path,
) -> Result<AgentPreferenceProvenance, String> {
    match scope_kind {
        GLOBAL_SCOPE_KIND => Ok(AgentPreferenceProvenance::Global),
        MOD_SCOPE_KIND => Ok(AgentPreferenceProvenance::Mod),
        _ => Err(state_database_error(
            store_path,
            "query",
            format!("unknown preference scope kind `{scope_kind}`"),
        )),
    }
}

fn preference_value(
    records: &[StoredPreferenceRecord],
    key: &str,
    store_path: &std::path::Path,
) -> Result<Option<Value>, String> {
    records
        .iter()
        .find(|record| record.preference_key == key)
        .map(|record| decode_preference_value(&record.value_json, store_path))
        .transpose()
}

fn decode_preference_value(value: &str, store_path: &std::path::Path) -> Result<Value, String> {
    serde_json::from_str(value)
        .map_err(|error| state_database_error(store_path, "query", error.to_string()))
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

fn unix_timestamp_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}
