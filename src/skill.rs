//------------------------------------------------------------------------------------
// skill.rs -- Part of RHoiScribe
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

use std::{error::Error, fmt, sync::Arc};

use rmcp::model::JsonObject;
use serde_json::{Map, Value, json};

use crate::{
    RhoiScribeRuntime, prompts::PromptCatalog, resources::ResourceCatalog, tools::ToolCatalog,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkillCommand {
    ListTools,
    ListResources,
    ListPrompts,
    ReadResource {
        uri: String,
    },
    GetPrompt {
        name: String,
        arguments_json: String,
    },
    CallTool {
        name: String,
        arguments_json: String,
    },
}

#[derive(Debug)]
pub enum SkillError {
    InvalidJson {
        command: &'static str,
        source: serde_json::Error,
    },
    InvalidArguments {
        command: &'static str,
    },
    Prompt(String),
    Resource(String),
    Tool(String),
    Serialize(serde_json::Error),
}

pub fn execute_skill_command(command: SkillCommand) -> Result<String, SkillError> {
    execute_skill_command_with_runtime(command, Arc::new(RhoiScribeRuntime::new()))
}

pub fn execute_skill_command_with_runtime(
    command: SkillCommand,
    runtime: Arc<RhoiScribeRuntime>,
) -> Result<String, SkillError> {
    match command {
        SkillCommand::ListTools => list_tools_json(),
        SkillCommand::ListResources => list_resources_json(),
        SkillCommand::ListPrompts => list_prompts_json(),
        SkillCommand::ReadResource { uri } => read_resource_json(&uri),
        SkillCommand::GetPrompt {
            name,
            arguments_json,
        } => get_prompt_json(&name, &arguments_json),
        SkillCommand::CallTool {
            name,
            arguments_json,
        } => call_tool_json(runtime, &name, &arguments_json),
    }
}

fn list_tools_json() -> Result<String, SkillError> {
    serialize(json!({
        "tools": ToolCatalog::builtin().to_mcp_tools()
    }))
}

fn list_resources_json() -> Result<String, SkillError> {
    let catalog = embedded_resource_catalog()?;
    serialize(json!({
        "resources": catalog.to_mcp_resources()
    }))
}

fn list_prompts_json() -> Result<String, SkillError> {
    serialize(json!({
        "prompts": PromptCatalog::builtin().to_mcp_prompts()
    }))
}

fn read_resource_json(uri: &str) -> Result<String, SkillError> {
    let catalog = embedded_resource_catalog()?;
    let result = catalog
        .read_mcp_resource(uri)
        .map_err(|error| SkillError::Resource(error.to_string()))?;
    serialize(result)
}

fn get_prompt_json(name: &str, arguments_json: &str) -> Result<String, SkillError> {
    let arguments = parse_object("get-prompt", arguments_json)?;
    let result = PromptCatalog::builtin()
        .render(name, &arguments)
        .map_err(|error| SkillError::Prompt(error.to_string()))?;
    serialize(result)
}

fn call_tool_json(
    runtime: Arc<RhoiScribeRuntime>,
    name: &str,
    arguments_json: &str,
) -> Result<String, SkillError> {
    let arguments = parse_object("call-tool", arguments_json)?;
    let result = ToolCatalog::builtin()
        .call_with_runtime(runtime, name, arguments)
        .map_err(|error| SkillError::Tool(error.to_string()))?;
    serialize(result)
}

fn embedded_resource_catalog() -> Result<ResourceCatalog, SkillError> {
    ResourceCatalog::load_embedded().map_err(|error| SkillError::Resource(error.to_string()))
}

impl fmt::Display for SkillError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SkillError::InvalidJson { command, source } => {
                write!(
                    formatter,
                    "{} arguments must be valid JSON: {}",
                    command, source
                )
            }
            SkillError::InvalidArguments { command } => {
                write!(formatter, "{} arguments must be a JSON object", command)
            }
            SkillError::Prompt(error) => write!(formatter, "prompt error: {}", error),
            SkillError::Resource(error) => write!(formatter, "resource error: {}", error),
            SkillError::Tool(error) => write!(formatter, "tool error: {}", error),
            SkillError::Serialize(error) => write!(formatter, "serialization error: {}", error),
        }
    }
}

impl Error for SkillError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            SkillError::InvalidJson { source, .. } => Some(source),
            SkillError::Serialize(error) => Some(error),
            _ => None,
        }
    }
}

fn parse_object(command: &'static str, json_text: &str) -> Result<JsonObject, SkillError> {
    let value = serde_json::from_str::<Value>(json_text)
        .map_err(|source| SkillError::InvalidJson { command, source })?;

    let Value::Object(object) = value else {
        return Err(SkillError::InvalidArguments { command });
    };

    Ok(Map::from_iter(object))
}

fn serialize<T>(value: T) -> Result<String, SkillError>
where
    T: serde::Serialize,
{
    serde_json::to_string_pretty(&value).map_err(SkillError::Serialize)
}
