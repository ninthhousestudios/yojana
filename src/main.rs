use std::sync::Arc;

use anyhow::Context;
use axum::routing::any_service;
use clap::Parser;
use rmcp::transport::streamable_http_server::{
    session::local::LocalSessionManager,
    tower::{StreamableHttpServerConfig, StreamableHttpService},
};
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;
use tracing_subscriber::EnvFilter;

use yojana::config::Config;
use yojana::db::Db;
use yojana::mcp::YojanaServer;

#[derive(Parser)]
#[command(name = "yojana", version)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(clap::Subcommand)]
enum Command {
    /// Start the yojana server (default if no subcommand given)
    Serve {
        /// Run in stdio mode instead of HTTP
        #[arg(long)]
        stdio: bool,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    let cli = Cli::parse();
    let command = cli.command.unwrap_or(Command::Serve { stdio: false });

    match command {
        Command::Serve { stdio } => {
            if stdio {
                tracing_subscriber::fmt()
                    .with_env_filter(EnvFilter::from_default_env())
                    .with_writer(std::io::stderr)
                    .init();
                serve_stdio().await
            } else {
                tracing_subscriber::fmt()
                    .with_env_filter(EnvFilter::from_default_env())
                    .init();
                serve_http().await
            }
        }
    }
}

async fn serve_stdio() -> anyhow::Result<()> {
    use rmcp::ServiceExt;
    use rmcp::transport::stdio;

    let config = Arc::new(Config::from_env());
    let db = Arc::new(Db::open(&config).context("opening database")?);
    let server = YojanaServer::new(db, config);

    let (stdin, stdout) = stdio();
    let service = server.serve((stdin, stdout)).await?;
    tokio::select! {
        res = service.waiting() => { res?; }
        _ = shutdown_signal() => {}
    }
    Ok(())
}

async fn serve_http() -> anyhow::Result<()> {
    let config = Arc::new(Config::from_env());
    let addr: std::net::SocketAddr = format!("{}:{}", config.host, config.port).parse()?;

    if tokio::net::TcpStream::connect(&addr).await.is_ok() {
        eprintln!("yojana already running on {addr}");
        std::process::exit(0);
    }

    let db = Arc::new(Db::open(&config).context("opening database")?);

    let pid_path = config.pid_path();
    if let Some(parent) = pid_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&pid_path, std::process::id().to_string())?;

    tracing::info!("yojana serving on {addr}");

    let mut session_manager = LocalSessionManager::default();
    session_manager.session_config.keep_alive = Some(std::time::Duration::from_secs(900));
    let session_manager = Arc::new(session_manager);

    let cancel = CancellationToken::new();
    let shttp_config =
        StreamableHttpServerConfig::default().with_cancellation_token(cancel.clone());

    let mcp_service = StreamableHttpService::new(
        move || Ok(YojanaServer::new(db.clone(), config.clone())),
        session_manager,
        shttp_config,
    );

    #[allow(deprecated)]
    let app = axum::Router::new().route("/mcp", any_service(mcp_service));

    let listener = TcpListener::bind(&addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    let _ = std::fs::remove_file(&pid_path);
    Ok(())
}

async fn shutdown_signal() {
    if let Err(e) = tokio::signal::ctrl_c().await {
        tracing::error!("failed to install CTRL+C handler: {e}");
    }
}
