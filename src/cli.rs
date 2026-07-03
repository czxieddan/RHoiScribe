//------------------------------------------------------------------------------------
// cli.rs -- Part of RHoiScribe
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
    error::Error,
    fmt, io,
    path::{Path, PathBuf},
};

pub type SkillCliCommand = crate::skill::SkillCommand;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CliCommand {
    Serve,
    Help,
    Version,
    PrintCommand,
    Logs {
        pattern: Option<String>,
    },
    ExportLogs {
        output_path: String,
        pattern: Option<String>,
    },
    Skill(SkillCliCommand),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliError {
    argument: String,
}

pub fn parse_args<I, S>(args: I) -> Result<CliCommand, CliError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let rest = args
        .into_iter()
        .skip(1)
        .map(|argument| argument.as_ref().to_string())
        .collect::<Vec<_>>();

    parse_rest(&rest)
}

fn parse_rest(rest: &[String]) -> Result<CliCommand, CliError> {
    match rest {
        [] => Ok(CliCommand::Serve),
        [flag] if flag == "--logs" => Ok(CliCommand::Logs { pattern: None }),
        [flag, pattern] if flag == "--logs" => Ok(CliCommand::Logs {
            pattern: Some(pattern.clone()),
        }),
        [flag, output_path] if flag == "--export-logs" => Ok(CliCommand::ExportLogs {
            output_path: output_path.clone(),
            pattern: None,
        }),
        [flag, output_path, pattern] if flag == "--export-logs" => Ok(CliCommand::ExportLogs {
            output_path: output_path.clone(),
            pattern: Some(pattern.clone()),
        }),
        [flag] => parse_single_flag(flag),
        [flag, skill_args @ ..] if flag == "--skill" => parse_skill_args(skill_args),
        [argument, ..] => Err(CliError {
            argument: argument.clone(),
        }),
    }
}

fn parse_single_flag(flag: &str) -> Result<CliCommand, CliError> {
    match flag {
        "--help" | "-h" => Ok(CliCommand::Help),
        "--version" | "-V" => Ok(CliCommand::Version),
        "--print-command" | "--mcp-command" => Ok(CliCommand::PrintCommand),
        argument => Err(CliError {
            argument: argument.to_string(),
        }),
    }
}

fn parse_skill_args(args: &[String]) -> Result<CliCommand, CliError> {
    match args {
        [command] => parse_skill_command(command, None, None),
        [command, value] => parse_skill_command(command, Some(value), None),
        [command, value, arguments_json] => {
            parse_skill_command(command, Some(value), Some(arguments_json))
        }
        [argument, ..] => Err(unknown_skill_argument(argument)),
        [] => Err(unknown_skill_argument("--skill")),
    }
}

fn parse_skill_command(
    command: &str,
    value: Option<&String>,
    arguments_json: Option<&String>,
) -> Result<CliCommand, CliError> {
    if let Some(command) = fixed_skill_command(command) {
        return Ok(CliCommand::Skill(command));
    }

    match command {
        "read-resource" => skill_resource_command(command, value),
        "get-prompt" => skill_prompt_command(command, value, arguments_json),
        "call-tool" => skill_tool_command(command, value, arguments_json),
        argument => Err(CliError {
            argument: argument.to_string(),
        }),
    }
}

fn fixed_skill_command(command: &str) -> Option<SkillCliCommand> {
    match command {
        "list-tools" => Some(SkillCliCommand::ListTools),
        "list-resources" => Some(SkillCliCommand::ListResources),
        "list-prompts" => Some(SkillCliCommand::ListPrompts),
        _ => None,
    }
}

fn skill_resource_command(command: &str, uri: Option<&String>) -> Result<CliCommand, CliError> {
    uri.map(|uri| CliCommand::Skill(SkillCliCommand::ReadResource { uri: uri.clone() }))
        .ok_or_else(|| unknown_skill_argument(command))
}

fn skill_prompt_command(
    command: &str,
    name: Option<&String>,
    arguments_json: Option<&String>,
) -> Result<CliCommand, CliError> {
    name.map(|name| {
        CliCommand::Skill(SkillCliCommand::GetPrompt {
            name: name.clone(),
            arguments_json: arguments_json.cloned().unwrap_or_else(|| "{}".to_string()),
        })
    })
    .ok_or_else(|| unknown_skill_argument(command))
}

fn skill_tool_command(
    command: &str,
    name: Option<&String>,
    arguments_json: Option<&String>,
) -> Result<CliCommand, CliError> {
    name.map(|name| {
        CliCommand::Skill(SkillCliCommand::CallTool {
            name: name.clone(),
            arguments_json: arguments_json.cloned().unwrap_or_else(|| "{}".to_string()),
        })
    })
    .ok_or_else(|| unknown_skill_argument(command))
}

fn unknown_skill_argument(argument: &str) -> CliError {
    CliError {
        argument: argument.to_string(),
    }
}

pub fn version_text() -> String {
    format!("rhoiscribe {}", env!("CARGO_PKG_VERSION"))
}

pub fn command_path() -> io::Result<PathBuf> {
    std::env::current_exe()
}

pub fn command_path_for_mcp_json() -> io::Result<String> {
    command_path().map(|path| path_for_mcp_json(&path))
}

pub fn path_for_mcp_json(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "\\\\")
}

pub fn help_text() -> &'static str {
    "RHoiScribe - local MCP server for HOI4 Modding agents\n\n\
Usage:\n\
  rhoiscribe                  Run the MCP server over stdio\n\
  rhoiscribe --print-command  Print the absolute command path for MCP config\n\
  rhoiscribe --mcp-command    Alias for --print-command\n\
  rhoiscribe --logs [REGEX]   Print recent tool logs as JSON\n\
  rhoiscribe --export-logs <PATH> [REGEX]\n\
                              Export matching tool logs as JSON\n\
  rhoiscribe --skill list-tools\n\
  rhoiscribe --skill list-resources\n\
  rhoiscribe --skill list-prompts\n\
  rhoiscribe --skill read-resource <URI>\n\
  rhoiscribe --skill get-prompt <NAME> <JSON_ARGUMENTS>\n\
  rhoiscribe --skill call-tool <NAME> <JSON_ARGUMENTS>\n\
  rhoiscribe --help           Show this help text\n\
  rhoiscribe --version        Show version information\n\n\
MCP clients should launch this binary as a local stdio server. Skill clients can use --skill commands for direct JSON output without MCP setup.\n"
}

impl fmt::Display for CliError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "unknown argument `{}`", self.argument)
    }
}

impl Error for CliError {}
