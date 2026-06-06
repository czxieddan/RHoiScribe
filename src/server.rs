use std::{future, future::Future};

use rmcp::{
    ErrorData as McpError, RoleServer, ServerHandler, ServiceExt,
    model::{
        GetPromptRequestParams, GetPromptResult, Implementation, ListPromptsResult,
        PaginatedRequestParams, ServerCapabilities, ServerInfo,
    },
    service::{MaybeSendFuture, RequestContext},
    transport::stdio,
};

use crate::prompts::PromptCatalog;

pub const SERVER_NAME: &str = "rhoiscribe";
pub const SERVER_TITLE: &str = "RHoiScribe";
pub const SERVER_INSTRUCTIONS: &str = "RHoiScribe provides local MCP prompts, resources, and batch tools for HOI4 Modding agents. Use it to reduce web lookups and keep generated mod files aligned with Hearts of Iron IV script conventions.";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerMetadata {
    pub name: &'static str,
    pub title: &'static str,
    pub version: &'static str,
    pub instructions: &'static str,
}

#[derive(Debug, Clone, Default)]
pub struct RhoiScribeServer;

impl RhoiScribeServer {
    pub fn new() -> Self {
        Self
    }

    pub fn metadata(&self) -> ServerMetadata {
        ServerMetadata {
            name: SERVER_NAME,
            title: SERVER_TITLE,
            version: env!("CARGO_PKG_VERSION"),
            instructions: SERVER_INSTRUCTIONS,
        }
    }

    pub fn server_info(&self) -> ServerInfo {
        let metadata = self.metadata();

        ServerInfo::new(
            ServerCapabilities::builder()
                .enable_prompts()
                .enable_resources()
                .enable_tools()
                .build(),
        )
        .with_server_info(
            Implementation::new(metadata.name, metadata.version)
                .with_title(metadata.title)
                .with_description("Local MCP server for HOI4 Modding agent workflows"),
        )
        .with_instructions(metadata.instructions)
    }
}

impl ServerHandler for RhoiScribeServer {
    fn get_info(&self) -> ServerInfo {
        self.server_info()
    }

    fn list_prompts(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ListPromptsResult, McpError>> + MaybeSendFuture + '_ {
        future::ready(Ok(ListPromptsResult::with_all_items(
            PromptCatalog::builtin().to_mcp_prompts(),
        )))
    }

    fn get_prompt(
        &self,
        request: GetPromptRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<GetPromptResult, McpError>> + MaybeSendFuture + '_ {
        let arguments = request.arguments.unwrap_or_default();
        future::ready(
            PromptCatalog::builtin()
                .render(&request.name, &arguments)
                .map_err(|error| McpError::invalid_params(error.to_string(), None)),
        )
    }
}

pub async fn run_stdio_server() -> anyhow::Result<()> {
    RhoiScribeServer::new()
        .serve(stdio())
        .await?
        .waiting()
        .await?;

    Ok(())
}
