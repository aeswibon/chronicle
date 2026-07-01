pub mod client;
pub mod server;

pub use client::{Client, EventStream};
pub use server::{Connection, Server};

use chronicle_core::Span;
pub use chronicle_core::{CanonicalEvent, ProjectRecord};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DaemonRequest {
    Subscribe {
        event_types: Vec<String>,
    },
    GetTimeline {
        since: i64,
        until: Option<i64>,
        limit: u32,
    },
    GetEvents {
        since: i64,
        until: Option<i64>,
        limit: u32,
    },
    GetProjectContext {
        project: String,
        since: i64,
        limit: u32,
    },
    Search {
        query: String,
        mode: SearchMode,
        limit: u32,
    },
    ListProjects {
        limit: u32,
    },
    GetErrors {
        since: i64,
        limit: u32,
    },
    GetSessions {
        since: i64,
        until: Option<i64>,
    },
    GetConfig,
    SetConfig {
        watch_dirs: Vec<String>,
        #[serde(default)]
        collectors: chronicle_config::CollectorsConfig,
    },
    InstallShellHook {
        shell: Option<String>,
    },
    GetSpan {
        id: String,
        event_limit: u32,
    },
    GetStatus,
    EmitEvent {
        event: CanonicalEvent,
    },
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
    Event {
        event: CanonicalEvent,
    },
    Timeline {
        spans: Vec<Span>,
    },
    TimelineEvents {
        events: Vec<CanonicalEvent>,
    },
    Projects {
        projects: Vec<ProjectRecord>,
    },
    ProjectContext {
        project: Option<ProjectRecord>,
        spans: Vec<Span>,
        events: Vec<CanonicalEvent>,
    },
    SpanDetail {
        span: Span,
        events: Vec<CanonicalEvent>,
    },
    Config {
        watch_dirs: Vec<String>,
        collectors: chronicle_config::CollectorsConfig,
    },
    Status {
        uptime_secs: u64,
        events_count: u64,
        version: String,
    },
    Ack {
        event_id: String,
    },
    Error {
        code: u32,
        message: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_socket_path() -> String {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let pid = std::process::id();
        let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
        format!("/tmp/chronicle-test-{pid}-{seq}.sock")
    }

    #[tokio::test]
    async fn test_request_response() {
        let path = test_socket_path();
        let server = Server::bind(&path).unwrap();

        tokio::spawn(async move {
            let mut conn = server.accept().await.unwrap();
            let req = conn.read_request().await.unwrap();
            match req {
                DaemonRequest::GetStatus => {
                    conn.send_response(DaemonResponse::Status {
                        uptime_secs: 42,
                        events_count: 100,
                        version: "0.1.0".into(),
                    })
                    .await
                    .unwrap();
                }
                _ => panic!("unexpected request"),
            }
        });

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        let mut client = Client::connect(&path).await.unwrap();
        let resp = client.request(DaemonRequest::GetStatus).await.unwrap();
        match resp {
            DaemonResponse::Status {
                uptime_secs,
                events_count,
                version,
            } => {
                assert_eq!(uptime_secs, 42);
                assert_eq!(events_count, 100);
                assert_eq!(version, "0.1.0");
            }
            _ => panic!("unexpected response"),
        }
    }

    #[tokio::test]
    async fn test_timeline_request() {
        let path = test_socket_path();
        let server = Server::bind(&path).unwrap();

        tokio::spawn(async move {
            let mut conn = server.accept().await.unwrap();
            let req = conn.read_request().await.unwrap();
            match req {
                DaemonRequest::GetTimeline { since, limit, .. } => {
                    assert_eq!(since, 1000);
                    assert_eq!(limit, 10);
                    conn.send_response(DaemonResponse::Timeline { spans: vec![] })
                        .await
                        .unwrap();
                }
                _ => panic!("unexpected request"),
            }
        });

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        let mut client = Client::connect(&path).await.unwrap();
        let resp = client
            .request(DaemonRequest::GetTimeline {
                since: 1000,
                until: None,
                limit: 10,
            })
            .await
            .unwrap();
        match resp {
            DaemonResponse::Timeline { spans } => {
                assert!(spans.is_empty());
            }
            _ => panic!("unexpected response"),
        }
    }

    #[tokio::test]
    async fn test_error_response() {
        let path = test_socket_path();
        let server = Server::bind(&path).unwrap();

        tokio::spawn(async move {
            let mut conn = server.accept().await.unwrap();
            conn.read_request().await.unwrap();
            conn.send_response(DaemonResponse::Error {
                code: 404,
                message: "not found".into(),
            })
            .await
            .unwrap();
        });

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        let mut client = Client::connect(&path).await.unwrap();
        let resp = client.request(DaemonRequest::GetStatus).await.unwrap();
        match resp {
            DaemonResponse::Error { code, message } => {
                assert_eq!(code, 404);
                assert_eq!(message, "not found");
            }
            _ => panic!("unexpected response"),
        }
    }

    #[tokio::test]
    async fn test_emit_event() {
        let path = test_socket_path();
        let server = Server::bind(&path).unwrap();
        let event = CanonicalEvent::new("test", chronicle_core::EventCategory::Os, "process.focus");

        let event_id = event.id.to_string();
        tokio::spawn(async move {
            let mut conn = server.accept().await.unwrap();
            let req = conn.read_request().await.unwrap();
            match req {
                DaemonRequest::EmitEvent { event: e } => {
                    conn.send_response(DaemonResponse::Ack {
                        event_id: e.id.to_string(),
                    })
                    .await
                    .unwrap();
                }
                _ => panic!("unexpected request"),
            }
        });

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        let mut client = Client::connect(&path).await.unwrap();
        let resp = client
            .request(DaemonRequest::EmitEvent { event })
            .await
            .unwrap();
        match resp {
            DaemonResponse::Ack { event_id: id } => {
                assert_eq!(id, event_id);
            }
            _ => panic!("unexpected response"),
        }
    }

    #[tokio::test]
    async fn test_connect_to_nonexistent_socket() {
        let result = Client::connect("/tmp/chronicle-nonexistent.sock").await;
        assert!(result.is_err());
    }
}
