use std::sync::Arc;

use anyhow::Context;
use axum::routing::{any_service, get};
use clap::Parser;
use rmcp::transport::streamable_http_server::{
    session::local::LocalSessionManager,
    tower::{StreamableHttpServerConfig, StreamableHttpService},
};
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;
use tracing_subscriber::EnvFilter;

use yojana::config::Config;
use yojana::db::{CreateTaskParams, Db, TaskQueryFilter, TaskUpdates, TERMINAL_STATUSES};
use yojana::display::{self, EdgeDirection, EdgeDisplay, format_dependency_tree};
use yojana::graph::build_dependency_forest;
use yojana::mcp::YojanaServer;

const DONE_RECENT_WINDOW_MS: i64 = 24 * 60 * 60 * 1000;

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
        /// Include all done/wontfix tasks (default: only those completed in last 24h)
        #[arg(long)]
        all: bool,
    },
    /// Mark a task done, optionally recording the commit it shipped in.
    Done {
        /// Task identifier (UUID or slug/N)
        id: String,
        /// Commit SHA to record as a context_ref on the task
        #[arg(long)]
        commit: Option<String>,
    },
    /// Show dependency tree for a project
    Tree {
        /// Project slug
        slug: String,
        /// Include done/wontfix tasks
        #[arg(long)]
        all: bool,
    },
    /// Quickly capture a task in a project (status: needs-triage)
    Todo {
        /// Project slug (supports nested form, e.g. `chitta/research`)
        project: String,
        /// Task title
        title: String,
        /// Task body. If omitted and stdin is piped, stdin is read as the body.
        #[arg(short = 'm', long = "message")]
        message: Option<String>,
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
            all,
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
                let edges = resolve_edges_for_display(&db, &task)?;
                println!("{}", display::format_task_detail(&task, &edges));
                return Ok(());
            }
            let project = db
                .get_project(None, Some(&identifier))?
                .ok_or_else(|| anyhow::anyhow!("project '{}' not found", identifier))?;
            let project_ids = db.project_ids_with_descendants(&project.id)?;
            // If user asked for a specific status, honor it and skip default-hide.
            let cutoff = if all || status.is_some() {
                None
            } else {
                Some(chrono::Utc::now().timestamp_millis() - DONE_RECENT_WINDOW_MS)
            };
            let filter = TaskQueryFilter {
                project_ids: Some(project_ids),
                status,
                category,
                include_terminal_after: cutoff,
                ..Default::default()
            };
            let tasks = db.list_tasks(&filter)?;
            let (active, recent_terminal): (Vec<_>, Vec<_>) = tasks
                .into_iter()
                .partition(|t| !TERMINAL_STATUSES.contains(&t.status.as_str()));
            if active.is_empty() && recent_terminal.is_empty() {
                println!("No tasks found.");
            } else {
                if !active.is_empty() || recent_terminal.is_empty() {
                    println!("Active");
                    println!("{}", display::format_tasks_list(&active));
                }
                if !recent_terminal.is_empty() {
                    if !active.is_empty() {
                        println!();
                    }
                    let header = if all {
                        "Done / wontfix"
                    } else {
                        "Recently done (last 24h)"
                    };
                    println!("{header}");
                    println!("{}", display::format_tasks_list(&recent_terminal));
                }
            }
            Ok(())
        }
        Command::Todo { project, title, message } => {
            let config = Config::from_env();
            let db = Db::open(&config).context("opening database")?;
            let proj = db
                .get_project(None, Some(&project))?
                .ok_or_else(|| anyhow::anyhow!("project '{}' not found", project))?;

            let description = match message {
                Some(m) => m,
                None => {
                    use std::io::{IsTerminal, Read};
                    let stdin = std::io::stdin();
                    if !stdin.is_terminal() {
                        let mut buf = String::new();
                        stdin.lock().read_to_string(&mut buf)?;
                        buf.trim_end_matches('\n').to_string()
                    } else {
                        String::new()
                    }
                }
            };

            let params = CreateTaskParams {
                project_id: proj.id,
                project_slug: proj.slug.clone(),
                title,
                description,
                category: None,
                slice_type: None,
                acceptance_criteria: "[]".to_string(),
                decisions: "[]".to_string(),
                context_refs: "[]".to_string(),
                files: "[]".to_string(),
                tags: "[]".to_string(),
                implementation_plan: None,
                execution_record: None,
                reproduction: None,
                root_cause: None,
            };
            let row = db.create_task(params)?;
            println!("{}/{}", row.project_slug, row.sequence_number);
            Ok(())
        }
        Command::Tree { slug, all } => {
            let config = Config::from_env();
            let db = Db::open(&config).context("opening database")?;
            let project = db
                .get_project(None, Some(&slug))?
                .ok_or_else(|| anyhow::anyhow!("project '{}' not found", slug))?;
            let project_ids = db.project_ids_with_descendants(&project.id)?;
            let filter = TaskQueryFilter {
                project_ids: Some(project_ids),
                ..Default::default()
            };
            let tasks = db.list_tasks(&filter)?;
            let task_ids: Vec<_> = tasks.iter().map(|t| t.id).collect();
            let edges = db.list_edges_by_type_for_tasks(&task_ids, "depends_on")?;
            let (mut forest, mut standalone) = build_dependency_forest(&task_ids, &edges);
            let task_map: std::collections::HashMap<uuid::Uuid, &yojana::db::TaskRow> =
                tasks.iter().map(|t| (t.id, t)).collect();
            if !all {
                let cutoff = chrono::Utc::now().timestamp_millis() - DONE_RECENT_WINDOW_MS;
                forest.retain(|root| !tree_all_stale(root, &task_map, cutoff));
                standalone.retain(|id| {
                    task_map
                        .get(id)
                        .map(|t| !is_stale_terminal(t, cutoff))
                        .unwrap_or(false)
                });
            }
            print!("{}", format_dependency_tree(&forest, &standalone, &task_map));
            Ok(())
        }
        Command::Done { id, commit } => {
            let config = Config::from_env();
            let db = Db::open(&config).context("opening database")?;
            let task = db
                .get_task(&id)?
                .ok_or_else(|| anyhow::anyhow!("task '{}' not found", id))?;

            let mut context_refs: Vec<serde_json::Value> =
                serde_json::from_str(&task.context_refs).unwrap_or_default();
            let refs_changed = if let Some(ref sha) = commit {
                context_refs.push(serde_json::json!({"type": "git:commit", "value": sha}));
                true
            } else {
                false
            };

            let updates = TaskUpdates {
                status: Some("done".to_string()),
                context_refs: if refs_changed {
                    Some(serde_json::to_string(&context_refs)?)
                } else {
                    None
                },
                ..Default::default()
            };
            let updated = db.update_task(&id, updates)?;
            let human_id = format!("{}/{}", updated.project_slug, updated.sequence_number);
            match commit {
                Some(sha) => println!("{} → done (commit {})", human_id, sha),
                None => println!("{} → done", human_id),
            }
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
    let app = axum::Router::new()
        .route("/mcp", any_service(mcp_service))
        .route("/health", get(|| async { axum::Json(serde_json::json!({"status": "ok"})) }));
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    cancel.cancel();
    let _ = std::fs::remove_file(&pid_path);
    Ok(())
}

fn is_stale_terminal(t: &yojana::db::TaskRow, cutoff: i64) -> bool {
    TERMINAL_STATUSES.contains(&t.status.as_str())
        && t.completed_at.map(|c| c < cutoff).unwrap_or(true)
}

fn tree_all_stale(
    node: &yojana::graph::ForestNode,
    tasks: &std::collections::HashMap<uuid::Uuid, &yojana::db::TaskRow>,
    cutoff: i64,
) -> bool {
    let self_stale = tasks
        .get(&node.task_id)
        .map(|t| is_stale_terminal(t, cutoff))
        .unwrap_or(true);
    self_stale && node.children.iter().all(|c| tree_all_stale(c, tasks, cutoff))
}

fn resolve_edges_for_display(
    db: &Db,
    task: &yojana::db::TaskRow,
) -> anyhow::Result<Vec<EdgeDisplay>> {
    let edges = db.list_edges_for_task(&task.id)?;
    let mut out = Vec::with_capacity(edges.len());
    for e in edges {
        let (other_id, direction) = if e.source_task_id == task.id {
            (e.target_task_id, EdgeDirection::Outgoing)
        } else {
            (e.source_task_id, EdgeDirection::Incoming)
        };
        let other = match db.get_task(&other_id.to_string())? {
            Some(t) => t,
            None => continue,
        };
        out.push(EdgeDisplay {
            direction,
            edge_type: e.edge_type,
            other_human_id: format!("{}/{}", other.project_slug, other.sequence_number),
            other_title: other.title,
        });
    }
    Ok(out)
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
