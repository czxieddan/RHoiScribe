//------------------------------------------------------------------------------------
// main.rs -- Part of RHoiScribe
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

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    match rhoiscribe::cli::parse_args(std::env::args())? {
        rhoiscribe::cli::CliCommand::Serve => rhoiscribe::server::run_stdio_server().await,
        rhoiscribe::cli::CliCommand::Help => {
            print!("{}", rhoiscribe::cli::help_text());
            Ok(())
        }
        rhoiscribe::cli::CliCommand::Version => {
            println!("{}", rhoiscribe::cli::version_text());
            Ok(())
        }
        rhoiscribe::cli::CliCommand::PrintCommand => {
            println!("{}", rhoiscribe::cli::command_path_for_mcp_json()?);
            Ok(())
        }
        rhoiscribe::cli::CliCommand::Logs { pattern } => {
            let result = rhoiscribe::tools::ToolEngine::query_tool_logs(
                rhoiscribe::tools::ToolLogQueryRequest {
                    store_path: None,
                    pattern,
                    limit: Some(100),
                },
            )?;
            println!("{}", serde_json::to_string_pretty(&result)?);
            Ok(())
        }
        rhoiscribe::cli::CliCommand::ExportLogs {
            output_path,
            pattern,
        } => {
            let result = rhoiscribe::tools::ToolEngine::export_tool_logs(
                rhoiscribe::tools::ToolLogExportRequest {
                    store_path: None,
                    output_path,
                    pattern,
                    limit: None,
                },
            )?;
            println!("{}", serde_json::to_string_pretty(&result)?);
            Ok(())
        }
        rhoiscribe::cli::CliCommand::Skill(command) => {
            println!("{}", rhoiscribe::skill::execute_skill_command(command)?);
            Ok(())
        }
    }
}
