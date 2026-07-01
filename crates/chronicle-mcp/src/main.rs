use anyhow::Context;
use chronicle_ipc::{Client, DaemonRequest, DaemonResponse, SearchMode};
use clap::Parser;
use rmcp::{
    handler::server::tool::ToolRouter, handler::server::wrapper::Parameters, model::*, tool,
    tool_handler, tool_router, transport::stdio, ErrorData as McpError, ServerHandler, ServiceExt,
};
use std::sync::Arc;

#[derive(Parser)]
#[command(
    name = "chronicle-mcp",
    about = "MCP server for Chronicle developer activity"
)]
struct Cli {
    #[arg(long, default_value = "~/.chronicle/chronicle.sock")]
    socket: String,
}

#[derive(Clone)]
struct ChronicleMcp {
    socket: Arc<String>,
    tool_router: ToolRouter<Self>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct SearchParams {
    query: String,
    #[serde(default = "default_limit")]
    limit: u32,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct LimitParams {
    #[serde(default = "default_limit")]
    limit: u32,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct TimelineParams {
    /// Milliseconds before now (default 24h)
    #[serde(default = "default_since_ms")]
    since_ms: i64,
    #[serde(default = "default_limit")]
    limit: u32,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct ProjectParams {
    project: String,
    #[serde(default = "default_since_ms")]
    since_ms: i64,
    #[serde(default = "default_limit")]
    limit: u32,
}

fn default_limit() -> u32 {
    25
}

fn default_since_ms() -> i64 {
    86_400_000
}

fn since_timestamp(since_ms: i64) -> i64 {
    chrono::Utc::now().timestamp_millis() - since_ms
}

async fn connect(socket: &str) -> anyhow::Result<Client> {
    Client::connect(socket)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))
        .with_context(|| format!("connect to chronicle daemon at {socket}"))
}

fn text_result(body: String) -> Result<CallToolResult, McpError> {
    Ok(CallToolResult::success(vec![Content::text(body)]))
}

fn error_result(message: impl std::fmt::Display) -> Result<CallToolResult, McpError> {
    Ok(CallToolResult::error(vec![Content::text(
        message.to_string(),
    )]))
}

#[tool_router]
impl ChronicleMcp {
    fn new(socket: String) -> Self {
        Self {
            socket: Arc::new(socket),
            tool_router: Self::tool_router(),
        }
    }

    #[tool(description = "Daemon health: version, uptime, and total events recorded")]
    async fn chronicle_status(&self) -> Result<CallToolResult, McpError> {
        match connect(&self.socket).await {
            Ok(mut client) => match client.request(DaemonRequest::GetStatus).await {
                Ok(DaemonResponse::Status {
                    uptime_secs,
                    events_count,
                    version,
                }) => text_result(
                    serde_json::to_string_pretty(&serde_json::json!({
                        "version": version,
                        "uptime_secs": uptime_secs,
                        "events_count": events_count,
                    }))
                    .unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}")),
                ),
                Ok(DaemonResponse::Error { message, .. }) => error_result(message),
                Ok(_) => error_result("unexpected response"),
                Err(e) => error_result(e),
            },
            Err(e) => error_result(e),
        }
    }

    #[tool(description = "Full-text search over recorded developer activity events")]
    async fn search_events(
        &self,
        Parameters(SearchParams { query, limit }): Parameters<SearchParams>,
    ) -> Result<CallToolResult, McpError> {
        match connect(&self.socket).await {
            Ok(mut client) => match client
                .request(DaemonRequest::Search {
                    query,
                    mode: SearchMode::Keyword,
                    limit,
                })
                .await
            {
                Ok(DaemonResponse::TimelineEvents { events }) => text_result(
                    serde_json::to_string_pretty(&events)
                        .unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}")),
                ),
                Ok(DaemonResponse::Error { message, .. }) => error_result(message),
                Ok(_) => error_result("unexpected response"),
                Err(e) => error_result(e),
            },
            Err(e) => error_result(e),
        }
    }

    #[tool(description = "List git/cargo projects detected from activity, newest first")]
    async fn list_projects(
        &self,
        Parameters(LimitParams { limit }): Parameters<LimitParams>,
    ) -> Result<CallToolResult, McpError> {
        match connect(&self.socket).await {
            Ok(mut client) => match client.request(DaemonRequest::ListProjects { limit }).await {
                Ok(DaemonResponse::Projects { projects }) => text_result(
                    serde_json::to_string_pretty(&projects)
                        .unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}")),
                ),
                Ok(DaemonResponse::Error { message, .. }) => error_result(message),
                Ok(_) => error_result("unexpected response"),
                Err(e) => error_result(e),
            },
            Err(e) => error_result(e),
        }
    }

    #[tool(description = "Recent focus/coding sessions (spans) from the activity timeline")]
    async fn get_timeline(
        &self,
        Parameters(TimelineParams { since_ms, limit }): Parameters<TimelineParams>,
    ) -> Result<CallToolResult, McpError> {
        let since = since_timestamp(since_ms);
        match connect(&self.socket).await {
            Ok(mut client) => match client
                .request(DaemonRequest::GetTimeline {
                    since,
                    until: None,
                    limit,
                })
                .await
            {
                Ok(DaemonResponse::Timeline { spans }) => text_result(
                    serde_json::to_string_pretty(&spans)
                        .unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}")),
                ),
                Ok(DaemonResponse::Error { message, .. }) => error_result(message),
                Ok(_) => error_result("unexpected response"),
                Err(e) => error_result(e),
            },
            Err(e) => error_result(e),
        }
    }

    #[tool(description = "Project detail: metadata, sessions, and recent activity for one project")]
    async fn get_project_context(
        &self,
        Parameters(ProjectParams {
            project,
            since_ms,
            limit,
        }): Parameters<ProjectParams>,
    ) -> Result<CallToolResult, McpError> {
        let since = since_timestamp(since_ms);
        match connect(&self.socket).await {
            Ok(mut client) => match client
                .request(DaemonRequest::GetProjectContext {
                    project,
                    since,
                    limit,
                })
                .await
            {
                Ok(DaemonResponse::ProjectContext {
                    project,
                    spans,
                    events,
                }) => text_result(
                    serde_json::to_string_pretty(&serde_json::json!({
                        "project": project,
                        "spans": spans,
                        "events": events,
                    }))
                    .unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}")),
                ),
                Ok(DaemonResponse::Error { message, .. }) => error_result(message),
                Ok(_) => error_result("unexpected response"),
                Err(e) => error_result(e),
            },
            Err(e) => error_result(e),
        }
    }

    #[tool(description = "Recent failed shell commands and other error-class events")]
    async fn get_recent_errors(
        &self,
        Parameters(TimelineParams { since_ms, limit }): Parameters<TimelineParams>,
    ) -> Result<CallToolResult, McpError> {
        let since = since_timestamp(since_ms);
        match connect(&self.socket).await {
            Ok(mut client) => match client
                .request(DaemonRequest::GetErrors { since, limit })
                .await
            {
                Ok(DaemonResponse::TimelineEvents { events }) => text_result(
                    serde_json::to_string_pretty(&events)
                        .unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}")),
                ),
                Ok(DaemonResponse::Error { message, .. }) => error_result(message),
                Ok(_) => error_result("unexpected response"),
                Err(e) => error_result(e),
            },
            Err(e) => error_result(e),
        }
    }

    #[tool(description = "AI rollup sessions persisted in the sessions table")]
    async fn get_sessions(
        &self,
        Parameters(TimelineParams { since_ms, limit: _ }): Parameters<TimelineParams>,
    ) -> Result<CallToolResult, McpError> {
        let since = since_timestamp(since_ms);
        match connect(&self.socket).await {
            Ok(mut client) => match client
                .request(DaemonRequest::GetSessions { since, until: None })
                .await
            {
                Ok(DaemonResponse::Sessions { sessions }) => text_result(
                    serde_json::to_string_pretty(&sessions)
                        .unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}")),
                ),
                Ok(DaemonResponse::Error { message, .. }) => error_result(message),
                Ok(_) => error_result("unexpected response"),
                Err(e) => error_result(e),
            },
            Err(e) => error_result(e),
        }
    }

    #[tool(
        description = "Generate and store a daily work summary from spans and events (AI when enabled, else rules)"
    )]
    async fn get_daily_summary(
        &self,
        Parameters(TimelineParams { since_ms, limit: _ }): Parameters<TimelineParams>,
    ) -> Result<CallToolResult, McpError> {
        let since = since_timestamp(since_ms);
        match connect(&self.socket).await {
            Ok(mut client) => match client
                .request(DaemonRequest::SummarizeDay { since, until: None })
                .await
            {
                Ok(DaemonResponse::DailySummary {
                    summary,
                    session,
                    source,
                }) => text_result(
                    serde_json::to_string_pretty(&serde_json::json!({
                        "summary": summary,
                        "source": source.unwrap_or_else(|| "rules".into()),
                        "session": session,
                    }))
                    .unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}")),
                ),
                Ok(DaemonResponse::Error { message, .. }) => error_result(message),
                Ok(_) => error_result("unexpected response"),
                Err(e) => error_result(e),
            },
            Err(e) => error_result(e),
        }
    }

    #[tool(description = "Installed plugin manifests from ~/.chronicle/plugins")]
    async fn list_plugins(&self) -> Result<CallToolResult, McpError> {
        match connect(&self.socket).await {
            Ok(mut client) => match client.request(DaemonRequest::ListPlugins).await {
                Ok(DaemonResponse::Plugins { plugins }) => text_result(
                    serde_json::to_string_pretty(&plugins)
                        .unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}")),
                ),
                Ok(DaemonResponse::Error { message, .. }) => error_result(message),
                Ok(_) => error_result("unexpected response"),
                Err(e) => error_result(e),
            },
            Err(e) => error_result(e),
        }
    }
}

#[tool_handler]
impl ServerHandler for ChronicleMcp {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some(
                "Query local Chronicle developer activity: search events, list projects, and read session timelines.".into(),
            ),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..Default::default()
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();

    let cli = Cli::parse();
    let service = ChronicleMcp::new(cli.socket).serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
