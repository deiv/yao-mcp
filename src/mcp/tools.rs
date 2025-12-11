use crate::vault::vault::{Vault};
use rmcp::{
    ErrorData as McpError, RoleServer, ServerHandler,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::*,
    schemars,
    service::RequestContext,
    tool, tool_handler, tool_router,
};
use std::sync::Arc;
use tracing::instrument;

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ReadNoteRequest {
    #[schemars(description = "the path to the note")]
    pub path: String,
}

#[derive(Clone, Debug)]
pub struct ObsidianMCP {
    tool_router: ToolRouter<ObsidianMCP>,
    vault_operations: Arc<Vault>,
}

#[tool_router]
impl ObsidianMCP {

    pub fn new(vault_operations: Arc<Vault>) -> Self {
        Self {
            tool_router: Self::tool_router(),
            vault_operations: vault_operations,
        }
    }

    #[tool(description = "Read a note from the current vault")]
    #[instrument()]
    async fn read_note(
        &self,
        Parameters(ReadNoteRequest { path }): Parameters<ReadNoteRequest>,
    ) -> Result<CallToolResult, McpError> {
        if path.is_empty() {
            return Err(McpError::invalid_request("path cannot be empty", None));
        }

        match self.vault_operations.read_note(&path).await {
            Ok(content) => return Ok(CallToolResult::success(vec![Content::text(content)])),
            Err(err) => return Err(McpError::internal_error(err.to_string(), None)),
        }
    }
}

#[tool_handler()]
impl ServerHandler for ObsidianMCP {
    async fn initialize(
        &self,
        _request: InitializeRequestParam,
        context: RequestContext<RoleServer>,
    ) -> Result<InitializeResult, McpError> {
        if let Some(http_request_part) = context.extensions.get::<axum::http::request::Parts>() {
            let initialize_headers = &http_request_part.headers;
            let initialize_uri = &http_request_part.uri;
            tracing::info!(?initialize_headers, %initialize_uri, "initialize from http server");
        }
        Ok(self.get_info())
    }

    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::V_2024_11_05,
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            server_info: Implementation::from_build_env(),
            instructions: Some(
                "This server provides Obsidian Vault mcp. Tools: read_note.".to_string(),
            ),
        }
    }
}

#[cfg(test)]
mod tests {}
