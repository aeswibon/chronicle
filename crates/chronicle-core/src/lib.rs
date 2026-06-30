use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum EventCategory {
    Os,
    Shell,
    Git,
    Browser,
    Ide,
    Filesystem,
    Infrastructure,
    Build,
    Meeting,
    Documentation,
    Ai,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SpanType {
    Coding,
    Debugging,
    Documentation,
    Terminal,
    Meeting,
    AiAssistant,
    Deployment,
    Idle,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SessionType {
    Focus,
    Break,
    Meeting,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanonicalEvent {
    pub version: String,
    pub id: Uuid,
    pub timestamp: i64,
    pub source: String,
    pub category: EventCategory,
    pub r#type: String,
    pub project: Option<String>,
    pub workspace: Option<String>,
    pub duration_ms: Option<u64>,
    pub metadata: serde_json::Value,
}

impl CanonicalEvent {
    pub fn new(source: &str, category: EventCategory, r#type: &str) -> Self {
        Self {
            version: "1.0".into(),
            id: Uuid::new_v4(),
            timestamp: chrono::Utc::now().timestamp_millis(),
            source: source.into(),
            category,
            r#type: r#type.into(),
            project: None,
            workspace: None,
            duration_ms: None,
            metadata: serde_json::Value::Object(Default::default()),
        }
    }

    pub fn with_project(mut self, project: &str) -> Self {
        self.project = Some(project.into());
        self
    }

    pub fn with_duration(mut self, ms: u64) -> Self {
        self.duration_ms = Some(ms);
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Span {
    pub id: Uuid,
    pub trace_id: Uuid,
    pub parent_id: Option<Uuid>,
    pub span_type: SpanType,
    pub project: Option<String>,
    pub started_at: i64,
    pub ended_at: Option<i64>,
    pub duration_ms: Option<u64>,
    pub event_count: u32,
    pub metadata: serde_json::Value,
}

impl Span {
    pub fn new(span_type: SpanType, project: Option<String>) -> Self {
        let now = chrono::Utc::now().timestamp_millis();
        Self {
            id: Uuid::new_v4(),
            trace_id: Uuid::new_v4(),
            parent_id: None,
            span_type,
            project,
            started_at: now,
            ended_at: None,
            duration_ms: None,
            event_count: 0,
            metadata: serde_json::Value::Object(Default::default()),
        }
    }

    pub fn close(&mut self) {
        let now = chrono::Utc::now().timestamp_millis();
        self.ended_at = Some(now);
        self.duration_ms = Some((now - self.started_at) as u64);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: Uuid,
    pub session_type: SessionType,
    pub started_at: i64,
    pub ended_at: Option<i64>,
    pub duration_ms: Option<u64>,
    pub project: Option<String>,
    pub span_count: u32,
    pub event_count: u32,
    pub summary: Option<String>,
}

impl Session {
    pub fn new(session_type: SessionType) -> Self {
        Self {
            id: Uuid::new_v4(),
            session_type,
            started_at: chrono::Utc::now().timestamp_millis(),
            ended_at: None,
            duration_ms: None,
            project: None,
            span_count: 0,
            event_count: 0,
            summary: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_canonical_event_defaults() {
        let event = CanonicalEvent::new("vscode", EventCategory::Ide, "file.edited");
        assert_eq!(event.version, "1.0");
        assert_eq!(event.source, "vscode");
        assert_eq!(event.category, EventCategory::Ide);
        assert_eq!(event.r#type, "file.edited");
        assert!(event.project.is_none());
        assert!(event.duration_ms.is_none());
        assert!(event.metadata.is_object());
    }

    #[test]
    fn test_canonical_event_builder() {
        let event = CanonicalEvent::new("zsh", EventCategory::Shell, "command.executed")
            .with_project("chronicle")
            .with_duration(1500);
        assert_eq!(event.project.unwrap(), "chronicle");
        assert_eq!(event.duration_ms.unwrap(), 1500);
    }

    #[test]
    fn test_span_create_and_close() {
        let mut span = Span::new(SpanType::Coding, Some("chronicle".into()));
        assert_eq!(span.span_type, SpanType::Coding);
        assert_eq!(span.project.as_deref(), Some("chronicle"));
        assert!(span.ended_at.is_none());

        std::thread::sleep(std::time::Duration::from_millis(5));
        span.close();
        assert!(span.ended_at.is_some());
        assert!(span.duration_ms.unwrap() >= 5);
    }

    #[test]
    fn test_session_creation() {
        let session = Session::new(SessionType::Focus);
        assert_eq!(session.session_type, SessionType::Focus);
        assert!(session.ended_at.is_none());
        assert_eq!(session.span_count, 0);
    }

    #[test]
    fn test_serde_roundtrip() {
        let event = CanonicalEvent::new("test", EventCategory::Os, "process.focus");
        let json = serde_json::to_string(&event).unwrap();
        let deserialized: CanonicalEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(event.id, deserialized.id);
        assert_eq!(event.source, deserialized.source);
        assert_eq!(event.category, deserialized.category);
    }

    #[test]
    fn test_event_category_serde() {
        let cases = vec![
            (EventCategory::Os, "\"os\""),
            (EventCategory::Shell, "\"shell\""),
            (EventCategory::Git, "\"git\""),
            (EventCategory::Ide, "\"ide\""),
            (EventCategory::Ai, "\"ai\""),
        ];
        for (cat, expected) in cases {
            let json = serde_json::to_string(&cat).unwrap();
            assert_eq!(json, expected);
        }
    }
}
