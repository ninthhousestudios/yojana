use std::sync::Arc;

use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{ServerCapabilities, ServerInfo};
use rmcp::{ErrorData, ServerHandler, tool, tool_handler, tool_router};

use crate::config::Config;
use crate::db::Db;
use crate::error::YojanaError;
use crate::tools;
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
        description = "Create, get, list, or update projects. Actions: create (requires slug, title), get (requires id or slug), list (optional status filter), update (requires id or slug, plus fields to change)."
    )]
    pub async fn yojana_project(
        &self,
        Parameters(args): Parameters<ProjectArgs>,
    ) -> Result<String, ErrorData> {
        let out = tools::project::handle(&self.db, args).map_err(err_to_rmcp)?;
        serde_json::to_string_pretty(&out).map_err(json_to_rmcp)
    }

    #[tool(
        description = "Create, get, or update tasks. Actions: create (requires project, title), get (requires id — UUID or 'project-slug/N'), update (requires id, plus fields to change). Supports acceptance_criteria, decisions, context_refs as JSON arrays."
    )]
    pub async fn yojana_task(
        &self,
        Parameters(args): Parameters<TaskArgs>,
    ) -> Result<String, ErrorData> {
        let out = tools::task::handle(&self.db, args).map_err(err_to_rmcp)?;
        serde_json::to_string_pretty(&out).map_err(json_to_rmcp)
    }

    #[tool(
        description = "Create, delete, or list task edges. Actions: create (requires source, target, edge_type — 'depends_on', 'relates_to', 'supersedes', 'refines', 'motivated_by'), delete (requires id), list (requires task — UUID or 'project-slug/N'). Cycle detection on depends_on edges."
    )]
    pub async fn yojana_edge(
        &self,
        Parameters(args): Parameters<EdgeArgs>,
    ) -> Result<String, ErrorData> {
        let out = tools::edge::handle(&self.db, args).map_err(err_to_rmcp)?;
        serde_json::to_string_pretty(&out).map_err(json_to_rmcp)
    }

    #[tool(
        description = "Query tasks with filters. Optional: project (id or slug), status, category, slice_type, tag. Omit project for cross-project query. Each result includes ready/blocked flags computed from the dependency graph."
    )]
    pub async fn yojana_query(
        &self,
        Parameters(args): Parameters<QueryArgs>,
    ) -> Result<String, ErrorData> {
        let out = tools::query::handle(&self.db, args).map_err(err_to_rmcp)?;
        serde_json::to_string_pretty(&out).map_err(json_to_rmcp)
    }

    #[tool(
        description = "Find tasks ready to start — status is ready-for-agent or ready-for-human with all depends_on targets done. Optional: project (id or slug) to scope to one project; omit for cross-project."
    )]
    pub async fn yojana_ready(
        &self,
        Parameters(args): Parameters<ReadyArgs>,
    ) -> Result<String, ErrorData> {
        let out = tools::ready::handle(&self.db, args).map_err(err_to_rmcp)?;
        serde_json::to_string_pretty(&out).map_err(json_to_rmcp)
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for YojanaServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_instructions("yojana v0.1.0 — task graph server for the manas ecosystem. Tracks projects, tasks, dependencies, and context shapes.")
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
