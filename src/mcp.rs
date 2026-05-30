use std::sync::Arc;

use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{ServerCapabilities, ServerInfo};
use rmcp::{ErrorData, ServerHandler, tool, tool_handler, tool_router};

use crate::config::Config;
use crate::db::Db;
use crate::error::YojanaError;
use crate::tools;
use crate::tools::arc::ArcArgs;
use crate::tools::context::ContextArgs;
use crate::tools::edge::EdgeArgs;
use crate::tools::project::ProjectArgs;
use crate::tools::query::QueryArgs;
use crate::tools::ready::ReadyArgs;
use crate::tools::task::TaskArgs;

pub struct YojanaServer {
    db: Arc<Db>,
    #[allow(dead_code)]
    config: Arc<Config>,
    tool_router: ToolRouter<Self>,
}

impl YojanaServer {
    pub fn new(db: Arc<Db>, config: Arc<Config>) -> Self {
        Self {
            db,
            config,
            tool_router: Self::tool_router(),
        }
    }
}

impl Clone for YojanaServer {
    fn clone(&self) -> Self {
        Self {
            db: self.db.clone(),
            config: self.config.clone(),
            tool_router: Self::tool_router(),
        }
    }
}

#[tool_router(router = tool_router)]
impl YojanaServer {
    #[tool(
        description = "Create, get, list, or update projects. Supports nested projects (workstreams) via slash-separated slugs. Actions: create (requires slug, title — parent auto-inferred from slug prefix), get (requires id or slug — returns full detail including children, description, handoff, history), list (returns slug + title per project by default; set compact=false for full rows: id, status, parent_id, has_handoff, child_count, timestamps; optional status filter; optional parent to scope to children), update (requires id or slug, plus fields to change — supports handoff: set a string to write, empty string to clear)."
    )]
    pub async fn yojana_project(
        &self,
        Parameters(args): Parameters<ProjectArgs>,
    ) -> Result<String, ErrorData> {
        let out = tools::project::handle(&self.db, args).map_err(err_to_rmcp)?;
        serde_json::to_string(&out).map_err(json_to_rmcp)
    }

    #[tool(
        description = "Create, get, update, advance, or revert arcs (lifecycle containers for tasks). Actions: create (requires project, title, phases — array of {name, slice_type?, gate?}; first phase defaults to active, rest to pending; returns project/~N identifier), get (requires id — UUID or 'project-slug/~N'), update (requires id, plus fields to change — status: active/paused/completed/abandoned, title, description, tags, context_refs), advance (requires id; optional phase to target specific phase, skip=true to set skipped instead of completed, note for history), revert (requires id and phase; sets completed phase back to active, optional note for history)."
    )]
    pub async fn yojana_arc(
        &self,
        Parameters(args): Parameters<ArcArgs>,
    ) -> Result<String, ErrorData> {
        let out = tools::arc::handle(&self.db, args).map_err(err_to_rmcp)?;
        serde_json::to_string(&out).map_err(json_to_rmcp)
    }

    #[tool(
        description = "Create, get, update, or comment on tasks. Actions: create (requires project, title), get (requires id — UUID or 'project-slug/N'), update (requires id, plus fields to change), comment (requires id, text; optional author). create/update return a slim ack {id, human_id, status, title}; use action=get (or yojana_context) for full detail. acceptance_criteria, decisions, context_refs are JSON arrays. The string detail fields (category, slice_type, implementation_plan, execution_record, reproduction, root_cause) accept null to clear. arc_id + arc_phase must be provided together to assign a task to an arc phase. commit is a SHA shorthand that appends a git:commit context_ref (use with update, typically alongside status=done)."
    )]
    pub async fn yojana_task(
        &self,
        Parameters(args): Parameters<TaskArgs>,
    ) -> Result<String, ErrorData> {
        let out = tools::task::handle(&self.db, args).map_err(err_to_rmcp)?;
        serde_json::to_string(&out).map_err(json_to_rmcp)
    }

    #[tool(
        description = "Create, delete, or list task edges. Actions: create (requires source, target, edge_type — 'depends_on', 'relates_to', 'supersedes', 'refines', 'motivated_by'), delete (requires id), list (requires task — UUID or 'project-slug/N'). Cycle detection on depends_on edges."
    )]
    pub async fn yojana_edge(
        &self,
        Parameters(args): Parameters<EdgeArgs>,
    ) -> Result<String, ErrorData> {
        let out = tools::edge::handle(&self.db, args).map_err(err_to_rmcp)?;
        serde_json::to_string(&out).map_err(json_to_rmcp)
    }

    #[tool(
        description = "Query tasks with filters. Optional: project (id or slug — includes tasks from all descendant sub-projects), status, category, slice_type, tag. Omit project for cross-project query. Each result includes ready/blocked flags computed from the dependency graph."
    )]
    pub async fn yojana_query(
        &self,
        Parameters(args): Parameters<QueryArgs>,
    ) -> Result<String, ErrorData> {
        let out = tools::query::handle(&self.db, args).map_err(err_to_rmcp)?;
        serde_json::to_string(&out).map_err(json_to_rmcp)
    }

    #[tool(
        description = "Find tasks ready to start — status is ready-for-agent or ready-for-human with all depends_on targets done. Returns handoff notes (if any) before the ready list. Optional: project (id or slug) to scope to one project and its sub-projects; omit for cross-project (all active projects)."
    )]
    pub async fn yojana_ready(
        &self,
        Parameters(args): Parameters<ReadyArgs>,
    ) -> Result<String, ErrorData> {
        let out = tools::ready::handle(&self.db, args).map_err(err_to_rmcp)?;
        serde_json::to_string(&out).map_err(json_to_rmcp)
    }

    #[tool(
        description = "Get a context bundle for a task or arc. Shapes: 'summary' (title, status, slice_type, category, edge counts, last history entry), 'working' (acceptance_criteria, decisions, 1-hop neighbor summaries, recent conversation messages, context_refs; includes arc info when task belongs to an arc), 'planning' (like working + prior-phase decisions and execution_records from the same arc), 'agent' (acceptance_criteria, decisions, implementation_plan, arc position, prior-phase context — slim shape for agent execution), 'review' (description, acceptance_criteria, decisions, implementation_plan, git/doc refs separated, neighbor summaries). Requires: task (UUID or 'project-slug/N'), shape. For arc-level summary: arc (UUID or 'project-slug/~N'), shape='summary'."
    )]
    pub async fn yojana_context(
        &self,
        Parameters(args): Parameters<ContextArgs>,
    ) -> Result<String, ErrorData> {
        let out = tools::context::handle(&self.db, args).map_err(err_to_rmcp)?;
        serde_json::to_string(&out).map_err(json_to_rmcp)
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for YojanaServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_instructions(&format!("yojana v{} — task graph server for the manas ecosystem. Tracks projects, tasks, dependencies, arcs (lifecycle phases), and context shapes.", env!("CARGO_PKG_VERSION")))
    }
}

fn err_to_rmcp(e: YojanaError) -> ErrorData {
    ErrorData::new(
        rmcp::model::ErrorCode(e.code()),
        e.message(),
        None::<serde_json::Value>,
    )
}

fn json_to_rmcp(e: serde_json::Error) -> ErrorData {
    ErrorData::new(
        rmcp::model::ErrorCode::INTERNAL_ERROR,
        format!("json serialization failed: {e}"),
        None::<serde_json::Value>,
    )
}
