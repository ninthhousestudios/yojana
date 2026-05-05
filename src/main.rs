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
use yojana::db::{Db, TaskQueryFilter};
use yojana::display;
use yojana::mcp::YojanaServer;

#[derive(Parser)]
#[command(name = "yojana", version)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(clap::Subcommand)]
enum Command {
    /// Start the yojana server
    Serve {
        /// Run in stdio mode instead of HTTP
        #[arg(long)]
        stdio: bool,
    },
    /// List projects, or show one project by slug
    Projects {
        /// Project slug to show detail for
        slug: Option<String>,
        /// Include paused and archived projects
        #[arg(long)]
        all: bool,
    },
    /// List tasks for a project, or show one task by slug/N
    Tasks {
        /// Project slug (list) or slug/N (detail)
        identifier: String,
        /// Filter by status
        #[arg(long)]
        status: Option<String>,
        /// Filter by category
        #[arg(long)]
        category: Option<String>,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    let cli = Cli::parse();

    match cli.command {
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
        Command::Projects { slug, all } => {
            let config = Config::from_env();
            let db = Db::open(&config).context("opening database")?;
            match slug {
                Some(s) => {
                    let project = db
                        .get_project(None, Some(&s))?
                        .ok_or_else(|| anyhow::anyhow!("project '{}' not found", s))?;
                    let children = db.list_projects(None, Some(Some(&project.id)), None, None)?;
                    println!("{}", display::format_project_detail(&project));
                    if !children.is_empty() {
                        println!("\nWorkstreams:");
                        println!("{}", display::format_projects_list(&children));
                    }
                }
                None => {
                    let status = if all { None } else { Some("active") };
                    let projects = db.list_projects(status, Some(None), None, None)?;
                    println!("{}", display::format_projects_list(&projects));
                }
            }
            Ok(())
        }
        Command::Tasks {
            identifier,
            status,
            category,
        } => {
            let config = Config::from_env();
            let db = Db::open(&config).context("opening database")?;
            let is_task_id = identifier
                .rsplit_once('/')
                .map(|(_, last)| last.parse::<i64>().is_ok())
                .unwrap_or(false);
            if is_task_id {
                let task = db
                    .get_task(&identifier)?
                    .ok_or_else(|| anyhow::anyhow!("task '{}' not found", identifier))?;
                println!("{}", display::format_task_detail(&task));
                return Ok(());
            }
            let project = db
                .get_project(None, Some(&identifier))?
                .ok_or_else(|| anyhow::anyhow!("project '{}' not found", identifier))?;
            let project_ids = db.project_ids_with_descendants(&project.id)?;
            let filter = TaskQueryFilter {
                project_ids: Some(project_ids),
                status,
                category,
                ..Default::default()
            };
            let tasks = db.list_tasks(&filter)?;
            println!("{}", display::format_tasks_list(&tasks));
            Ok(())
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

    let db = Arc::new(Db::open(&config).context("opening database")?);

    let listener = match TcpListener::bind(&addr).await {
        Ok(l) => l,
        Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {
            eprintln!("yojana already running on {addr}");
            std::process::exit(0);
        }
        Err(e) => return Err(e.into()),
    };

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
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    cancel.cancel();
    let _ = std::fs::remove_file(&pid_path);
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = tokio::signal::ctrl_c();
    let mut term =
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()).unwrap();
    tokio::select! {
        _ = ctrl_c => {},
        _ = term.recv() => {},
    }
}
