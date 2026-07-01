use chronicle_core::{CanonicalEvent, EventCategory, Span, SpanType};
use std::collections::HashMap;

const SESSION_TIMEOUT_MS: i64 = 15 * 60 * 1000;

pub struct SpanProcessor {
    active: HashMap<Option<String>, ActiveSpan>,
}

struct ActiveSpan {
    id: uuid::Uuid,
    trace_id: uuid::Uuid,
    span_type: SpanType,
    started_at: i64,
    last_event_ts: i64,
    event_count: u32,
    project: Option<String>,
}

impl SpanProcessor {
    pub fn new() -> Self {
        Self {
            active: HashMap::new(),
        }
    }

    pub fn process(&mut self, event: &CanonicalEvent) -> Option<Span> {
        let mut closed_span = None;
        let key = event.project.clone();
        let span_type = category_to_span_type(&event.category, event);

        if let Some(active) = self.active.get_mut(&key) {
            let gap = event.timestamp.saturating_sub(active.last_event_ts);
            if gap > SESSION_TIMEOUT_MS || active.span_type != span_type {
                let mut span = Span::new(active.span_type.clone(), active.project.clone());
                span.id = active.id;
                span.trace_id = active.trace_id;
                span.started_at = active.started_at;
                span.ended_at = Some(event.timestamp);
                span.duration_ms = Some((event.timestamp - active.started_at).max(0) as u64);
                span.event_count = active.event_count;
                closed_span = Some(span);

                *active = ActiveSpan::new(event.timestamp, span_type, key.clone());
            } else {
                active.last_event_ts = event.timestamp;
                active.event_count += 1;
            }
        } else {
            self.active.insert(
                key.clone(),
                ActiveSpan::new(event.timestamp, span_type, key.clone()),
            );
        }

        closed_span
    }
}

impl ActiveSpan {
    fn new(timestamp: i64, span_type: SpanType, project: Option<String>) -> Self {
        Self {
            id: uuid::Uuid::new_v4(),
            trace_id: uuid::Uuid::new_v4(),
            span_type,
            started_at: timestamp,
            last_event_ts: timestamp,
            event_count: 1,
            project,
        }
    }
}

fn category_to_span_type(category: &EventCategory, event: &CanonicalEvent) -> SpanType {
    if *category == EventCategory::Os && is_agent_app(event) {
        return SpanType::AiAssistant;
    }
    match category {
        EventCategory::Os => SpanType::Idle,
        EventCategory::Shell => SpanType::Terminal,
        EventCategory::Git => SpanType::Coding,
        EventCategory::Browser => SpanType::Documentation,
        EventCategory::Ide => SpanType::Coding,
        EventCategory::Filesystem => SpanType::Coding,
        EventCategory::Infrastructure => SpanType::Deployment,
        EventCategory::Build => SpanType::Deployment,
        EventCategory::Meeting => SpanType::Meeting,
        EventCategory::Documentation => SpanType::Documentation,
        EventCategory::Ai => SpanType::AiAssistant,
    }
}

fn is_agent_app(event: &CanonicalEvent) -> bool {
    let app = event
        .metadata
        .get("app_name")
        .and_then(|v| v.as_str())
        .unwrap_or(&event.source)
        .to_lowercase();
    let bundle = event
        .metadata
        .get("bundle_id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_lowercase();
    const AGENT_APPS: &[&str] = &[
        "cursor",
        "claude",
        "codex",
        "gemini",
        "windsurf",
        "copilot",
        "aider",
        "opencode",
        "antigravity",
    ];
    AGENT_APPS
        .iter()
        .any(|needle| app.contains(needle) || bundle.contains(needle))
}
