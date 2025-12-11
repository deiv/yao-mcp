use std::fmt::{Display, Formatter};
use std::io;
use std::path::PathBuf;
use std::sync::Arc;
use hyper_util::{
    rt::{TokioExecutor, TokioIo},
    server::conn::auto::Builder,
    service::TowerToHyperService,
};
use rmcp::transport::streamable_http_server::{
    StreamableHttpService, session::local::LocalSessionManager,
};
use clap::Parser;
use rmcp::service::{QuitReason, ServerInitializeError};
#[cfg(feature = "stdio")]
use rmcp::ServiceExt;
#[cfg(feature = "stdio")]
use rmcp::transport::stdio;
use tracing::log;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::{fmt, Registry};
use tracing_subscriber::util::{SubscriberInitExt, TryInitError};
use yao_mcp::mcp::tools::ObsidianMCP;
use yao_mcp::vault::vault::Vault;

#[derive(Debug)]
pub enum AppError {
    /// Vault Path Error
    VaultPathNotExist(io::Error),

    /// Argument Error
    ArgumentError { reason: String },

    /// UnexpectedError
    UnexpectedError { reason: String }
}

impl Display for AppError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            AppError::VaultPathNotExist(err) => {
                f.write_str(err.to_string().as_str())
            }

            AppError::ArgumentError { reason } |
            AppError::UnexpectedError { reason } => {
                f.write_str(reason)
            }
        }
    }
}

impl From<TryInitError> for AppError {
    fn from(err: TryInitError) -> Self {
        AppError::UnexpectedError {
            reason: err.to_string(),
        }
    }
}

impl From<ServerInitializeError> for AppError {
    fn from(err: ServerInitializeError) -> Self {
        AppError::UnexpectedError {
            reason: err.to_string(),
        }
    }
}

impl From<std::io::Error> for AppError {
    fn from(err: std::io::Error) -> Self {
        AppError::UnexpectedError {
            reason: err.to_string(),
        }
    }
}

impl AppError {
    pub fn vault_path_error(err: io::Error) -> Self {
        AppError::VaultPathNotExist(err)
    }

    pub fn argument_error(reason: impl Into<String>) -> Self {
        AppError::ArgumentError {
            reason: reason.into(),
        }
    }

    pub fn unexpected_error(reason: impl Into<String>) -> Self {
        AppError::UnexpectedError {
            reason: reason.into(),
        }
    }
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct ApplicationArgs {
    /// Path to the Obsidian vault
    #[arg(short, long, env = "OBSIDIAN_VAULT_PATH", required = true)]
    vault_path: Option<PathBuf>,

    /// MCP Transport mode (stdio, http)
    #[arg(short, long, default_value = "http", env = "MCP_TRANSPORT_MODE", required = true)]
    transport: String,

    /// MCP HTTP server port (for http transport)
    #[arg(long, default_value = "3000", env = "MCP_HTTP_TRANSPORT_PORT")]
    port: u16,
}

#[cfg(feature = "stdio")]
async fn start_stdio_server(vault : Arc<Vault>) -> Result<QuitReason, AppError> {
    log::info!("Starting MCP server in STDIO mode. Use Ctrl+C to exit.");
    let service=
        ObsidianMCP::new(vault)
            .serve(stdio())
            .await
            .inspect_err(|e| { tracing::error!("serving error: {:?}", e); })?;

    service.waiting().await.map_err(|e| AppError::unexpected_error(e.to_string()))
}

#[cfg(feature = "http")]
async fn start_http_server(vault : Arc<Vault>, port : u16) -> Result<QuitReason, AppError> {
    let addr = format!("0.0.0.0:{}", port);
    log::info!("Starting MCP server in HTTP mode with addr: {}. Use Ctrl+C to exit.", addr);

    let service = TowerToHyperService::new(
        StreamableHttpService::new(
            move || Ok(ObsidianMCP::new(vault.clone())),
            LocalSessionManager::default().into(),
            Default::default(),
    ));

    let listener = tokio::net::TcpListener::bind(addr).await?;

    loop {
        let io = tokio::select! {
            _ = tokio::signal::ctrl_c() => break,
            accept = listener.accept() => {
                TokioIo::new(accept?.0)
            }
        };

        let service = service.clone();
        tokio::spawn(async move {
            let _result = Builder::new(TokioExecutor::default())
                .serve_connection(io, service)
                .await;
        });
    }

    Ok(QuitReason::Cancelled)
}

fn handle_transport_arg_error(transport: &str) -> Result<(), AppError> {

    #[cfg(not(feature = "stdio"))]
    if transport == "stdio" {
        return Err(AppError::argument_error("STDIO transport not enabled. Rebuild with --features stdio"));
    }

    #[cfg(not(feature = "http"))]
    if transport == "http" {
        return Err(anyhow!("HTTP transport not enabled. Rebuild with --features http"));
    }

    let mut enabled_transports = Vec::new();

    if cfg!(feature = "stdio") {
        enabled_transports.push("stdio");
    }

    if cfg!(feature = "http") {
        enabled_transports.push("http");
    }

    Err(AppError::argument_error(
        format!(
            "Unknown transport '{}'. Valid options: {}",
            transport,
            enabled_transports.join(","),
    )))
}

#[tokio::main]
async fn main() -> Result<(), AppError> {

    let args = ApplicationArgs::parse();

    Registry::default()
        .with(
            fmt::layer()
                .with_writer(std::io::stderr)
                .with_target(true)
                .with_thread_ids(true)
                .with_file(true)
                .with_line_number(true)
                //.json(),
        )
        .try_init()?;

   let vault_path = match args.vault_path {
        Some(path) => {
            let canonicalized_vault_path = path.canonicalize();

            match canonicalized_vault_path {
                Ok(path) => { path }
                Err(err) => { return Err(AppError::vault_path_error(err)) }
            }
        }
        None => { return Err(AppError::unexpected_error("vault path is required")) }
    };

    let vault = Arc::new(Vault::new(vault_path).unwrap());

    match args.transport.as_str() {
        #[cfg(feature = "stdio")]
        "stdio" => {
            start_stdio_server(vault).await?;
        }
        #[cfg(feature = "http")]
        "http" => {
            start_http_server(vault, args.port).await?;
        }
        transport => {
            return handle_transport_arg_error(transport);
        }
    }

    Ok(())
}
