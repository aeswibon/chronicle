use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
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
}
