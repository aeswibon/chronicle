pub mod client;
pub mod server;

pub use client::Client;
pub use server::Server;

use serde::{Deserialize, Serialize};
pub use chronicle_core::CanonicalEvent;

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DaemonRequest {
    Subscribe { event_types: Vec<String> },
    GetTimeline { since: i64, until: Option<i64>, limit: u32 },
    GetProjectContext { project: String },
    Search { query: String, mode: SearchMode },
    GetErrors { since: i64, limit: u32 },
    GetSessions { since: i64, until: Option<i64> },
    GetStatus,
    EmitEvent { event: CanonicalEvent },
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SearchMode {
    Keyword,
    Semantic,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DaemonResponse {
    Event { event: CanonicalEvent },
    Timeline { spans: Vec<CanonicalEvent> },
    Status { uptime_secs: u64, events_count: u64, version: String },
    Ack { event_id: String },
    Error { code: u32, message: String },
}
