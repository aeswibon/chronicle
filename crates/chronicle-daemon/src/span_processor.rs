use chronicle_core::{CanonicalEvent, EventCategory, Span, SpanType};
use std::collections::HashMap;
use std::time::{Duration, Instant};

const SESSION_TIMEOUT: Duration = Duration::from_secs(15 * 60);

pub struct SpanProcessor {
    active: HashMap<Option<String>, ActiveSpan>,
}

struct ActiveSpan {
    id: uuid::Uuid,
    trace_id: uuid::Uuid,
    span_type: SpanType,
    started_at: i64,
    last_activity: Instant,
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
        let now = Instant::now();

        let span_type = category_to_span_type(&event.category);

        if let Some(active) = self.active.get_mut(&key) {
            if active.last_activity.elapsed() > SESSION_TIMEOUT
                || active.span_type != span_type
            {
                let mut span = Span::new(active.span_type.clone(), active.project.clone());
                span.id = active.id;
                span.trace_id = active.trace_id;
                span.started_at = active.started_at;
                span.ended_at = Some(event.timestamp);
                span.duration_ms = Some((event.timestamp - active.started_at) as u64);
                span.event_count = active.event_count;
                closed_span = Some(span);

                *active = ActiveSpan::new(event.timestamp, span_type, key.clone());
            } else {
                active.last_activity = now;
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
            last_activity: Instant::now(),
            event_count: 1,
            project,
        }
    }
}

fn category_to_span_type(category: &EventCategory) -> SpanType {
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
