//------------------------------------------------------------------------------------
// cwt_file_validation.rs -- Part of RHoiScribe
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
    path::{Component, Path, PathBuf},
    sync::{Arc, OnceLock},
};

use serde::{Deserialize, Serialize};

use crate::{
    RhoiScribeRuntime,
    cwt::rules::{
        HOI4_CWT_CONFIG_CONTENT_SHA256, HOI4_CWT_CONFIG_REVISION, LoadedCwtRules,
        load_embedded_hoi4_cwt_rules,
    },
};

use super::{
    ToolError,
    cwt_common::{normalize_path, path_to_string, workspace_snapshot_from_handle},
    cwt_diagnostics::{Hoi4Diagnostic, validate_content},
};

static EMBEDDED_FILE_VALIDATION_RULES: OnceLock<Result<Arc<LoadedCwtRules>, String>> =
    OnceLock::new();

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ValidateHoi4FileRequest {
    pub handle_id: Option<String>,
    pub workspace_root: Option<String>,
    pub path: Option<String>,
    pub content: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ValidateHoi4FileResult {
    pub path: String,
    pub handle_id: Option<String>,
    pub diagnostics: Vec<Hoi4Diagnostic>,
    pub status: String,
    pub rule_revision: String,
    pub rule_content_sha256: String,
    pub runtime_disk_entities: bool,
}

struct FileValidationContext {
    rules: Arc<LoadedCwtRules>,
    handle_id: Option<String>,
    workspace_root: Option<PathBuf>,
}

pub fn validate_file(
    runtime: Arc<RhoiScribeRuntime>,
    request: ValidateHoi4FileRequest,
) -> Result<ValidateHoi4FileResult, ToolError> {
    let validation_context = rules_for_file_validation(&runtime, &request)?;
    let path = validation_path(&request)?;
    let content = file_content(
        &request,
        validation_context.workspace_root.as_deref(),
        &path,
    )?;
    let diagnostics = validate_content(&validation_context.rules, &path, &content);

    Ok(ValidateHoi4FileResult {
        path: normalize_path(&path),
        handle_id: validation_context.handle_id,
        status: diagnostics_status(&diagnostics),
        diagnostics,
        rule_revision: HOI4_CWT_CONFIG_REVISION.to_string(),
        rule_content_sha256: HOI4_CWT_CONFIG_CONTENT_SHA256.to_string(),
        runtime_disk_entities: false,
    })
}

fn rules_for_file_validation(
    runtime: &Arc<RhoiScribeRuntime>,
    request: &ValidateHoi4FileRequest,
) -> Result<FileValidationContext, ToolError> {
    if let Some(handle_id) = &request.handle_id {
        let snapshot = workspace_snapshot_from_handle(runtime, handle_id)?;
        return Ok(FileValidationContext {
            rules: Arc::clone(&snapshot.rules),
            handle_id: Some(handle_id.clone()),
            workspace_root: Some(snapshot.workspace_root.clone()),
        });
    }

    Ok(FileValidationContext {
        rules: embedded_file_validation_rules()?,
        handle_id: request.handle_id.clone(),
        workspace_root: request.workspace_root.as_ref().map(PathBuf::from),
    })
}

fn embedded_file_validation_rules() -> Result<Arc<LoadedCwtRules>, ToolError> {
    EMBEDDED_FILE_VALIDATION_RULES
        .get_or_init(|| {
            load_embedded_hoi4_cwt_rules()
                .map(Arc::new)
                .map_err(|error| error.to_string())
        })
        .clone()
        .map_err(ToolError::InvalidRequest)
}

fn file_content(
    request: &ValidateHoi4FileRequest,
    workspace_root: Option<&Path>,
    path: &str,
) -> Result<String, ToolError> {
    if let Some(content) = &request.content {
        return Ok(content.clone());
    }

    let read_path = readable_validation_path(workspace_root, path)?;
    fs::read_to_string(&read_path).map_err(|error| {
        ToolError::InvalidRequest(format!(
            "failed to read CWT validation file `{}`: {}",
            path_to_string(&read_path),
            error
        ))
    })
}

fn readable_validation_path(
    workspace_root: Option<&Path>,
    path: &str,
) -> Result<PathBuf, ToolError> {
    let Some(root) = workspace_root else {
        return Ok(PathBuf::from(path));
    };
    let relative_path = Path::new(path);
    if relative_path.is_absolute() || has_escaping_component(relative_path) {
        return Err(ToolError::InvalidRequest(
            "CWT validation path must stay inside workspace_root".to_string(),
        ));
    }
    Ok(join_relative_path(root, path))
}

fn has_escaping_component(path: &Path) -> bool {
    path.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::Prefix(_) | Component::RootDir
        )
    })
}

fn validation_path(request: &ValidateHoi4FileRequest) -> Result<String, ToolError> {
    if let Some(path) = request
        .path
        .as_deref()
        .map(str::trim)
        .filter(|path| !path.is_empty())
    {
        return Ok(path.to_string());
    }

    let Some(content) = request.content.as_deref() else {
        return Err(ToolError::InvalidRequest(
            "path is required when content is omitted".to_string(),
        ));
    };

    Ok(conversation_virtual_path(content).to_string())
}

fn conversation_virtual_path(content: &str) -> &'static str {
    conversation_content_kind(content).virtual_path()
}

fn conversation_content_kind(content: &str) -> ConversationContentKind {
    let normalized = content.to_ascii_lowercase();
    if contains_any(
        &normalized,
        &[
            "country_event",
            "news_event",
            "state_event",
            "add_namespace",
        ],
    ) {
        ConversationContentKind::Event
    } else if contains_any(&normalized, &["focus_tree", "focus ="]) {
        ConversationContentKind::Focus
    } else if contains_any(
        &normalized,
        &["spritetype", "containerwindowtype", "quadtexturesprite"],
    ) {
        ConversationContentKind::Interface
    } else {
        ConversationContentKind::ScriptedEffect
    }
}

enum ConversationContentKind {
    Event,
    Focus,
    Interface,
    ScriptedEffect,
}

impl ConversationContentKind {
    fn virtual_path(self) -> &'static str {
        match self {
            Self::Event => "events/rhoiscribe_conversation.txt",
            Self::Focus => "common/national_focus/rhoiscribe_conversation.txt",
            Self::Interface => "interface/rhoiscribe_conversation.gui",
            Self::ScriptedEffect => "common/scripted_effects/rhoiscribe_conversation.txt",
        }
    }
}

fn contains_any(value: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| value.contains(needle))
}

fn diagnostics_status(diagnostics: &[Hoi4Diagnostic]) -> String {
    if diagnostics
        .iter()
        .any(|diagnostic| diagnostic.status == "red")
    {
        "red".to_string()
    } else if diagnostics
        .iter()
        .any(|diagnostic| diagnostic.status == "yellow")
    {
        "yellow".to_string()
    } else {
        "green".to_string()
    }
}

fn join_relative_path(root: &Path, path: &str) -> PathBuf {
    path.split('/')
        .filter(|part| !part.is_empty())
        .fold(root.to_path_buf(), |current, part| current.join(part))
}
