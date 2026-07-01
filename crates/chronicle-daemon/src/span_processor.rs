use crate::app_classify::{
    is_agent_app, is_browser_app, is_finder_app, is_ide_app, is_terminal_app,
};
use chronicle_core::{CanonicalEvent, EventCategory, Span, SpanType};

const SESSION_TIMEOUT_MS: i64 = 15 * 60 * 1000;

pub struct SpanProcessor {
    active: Option<ActiveSpan>,
}

struct ActiveSpan {
    tab_session_key: Option<String>,
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
        Self { active: None }
    }

    pub fn process(&mut self, event: &CanonicalEvent) -> Option<Span> {
        let project = event.project.clone();
        let span_type = category_to_span_type(&event.category, event);
        let mut closed_span = None;

        if let Some(active) = &self.active {
            let gap = event.timestamp.saturating_sub(active.last_event_ts);
            let project_changed = active.project != project;
            let tab_changed = tab_session_key(event)
                .is_some_and(|k| active.tab_session_key.as_deref() != Some(k.as_str()));
            if gap > SESSION_TIMEOUT_MS
                || active.span_type != span_type
                || project_changed
                || tab_changed
            {
                closed_span = Some(active.clone().into_closed_span(event.timestamp));
                self.active = Some(ActiveSpan::new(
                    event.timestamp,
                    span_type,
                    project,
                    tab_session_key(event),
                ));
            } else {
                let active = self.active.as_mut().unwrap();
                active.last_event_ts = event.timestamp;
                active.event_count += 1;
            }
        } else {
            self.active = Some(ActiveSpan::new(
                event.timestamp,
                span_type,
                project,
                tab_session_key(event),
            ));
        }

        closed_span
    }

    /// Close every open span (e.g. when the user switches apps).
    pub fn close_all(&mut self, ended_at: i64) -> Vec<Span> {
        self.active
            .take()
            .map(|a| a.into_closed_span(ended_at))
            .into_iter()
            .collect()
    }

    /// Open spans still in progress (not yet persisted).
    pub fn active_spans(&self) -> Vec<Span> {
        self.active
            .as_ref()
            .map(ActiveSpan::to_open_span)
            .into_iter()
            .collect()
    }
}

impl ActiveSpan {
    fn new(
        timestamp: i64,
        span_type: SpanType,
        project: Option<String>,
        tab_session_key: Option<String>,
    ) -> Self {
        Self {
            tab_session_key,
            id: uuid::Uuid::new_v4(),
            trace_id: uuid::Uuid::new_v4(),
            span_type,
            started_at: timestamp,
            last_event_ts: timestamp,
            event_count: 1,
            project,
        }
    }

    fn to_open_span(&self) -> Span {
        let mut span = Span::new(self.span_type.clone(), self.project.clone());
        span.id = self.id;
        span.trace_id = self.trace_id;
        span.started_at = self.started_at;
        span.ended_at = None;
        span.duration_ms = Some((self.last_event_ts - self.started_at).max(0) as u64);
        span.event_count = self.event_count;
        if let Some(obj) = span.metadata.as_object_mut() {
            obj.insert(
                "last_event_at".into(),
                serde_json::json!(self.last_event_ts),
            );
            if let Some(ref key) = self.tab_session_key {
                obj.insert("tab_session_key".into(), key.clone().into());
            }
        }
        span
    }

    fn into_closed_span(self, ended_at: i64) -> Span {
        let mut span = self.to_open_span();
        span.ended_at = Some(ended_at);
        span.duration_ms = Some((ended_at - self.started_at).max(0) as u64);
        span
    }
}

impl Clone for ActiveSpan {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            trace_id: self.trace_id,
            span_type: self.span_type.clone(),
            started_at: self.started_at,
            last_event_ts: self.last_event_ts,
            event_count: self.event_count,
            project: self.project.clone(),
            tab_session_key: self.tab_session_key.clone(),
        }
    }
}

fn tab_session_key(event: &CanonicalEvent) -> Option<String> {
    if let Some(key) = event
        .metadata
        .get("tab_session_key")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
    {
        return Some(key.to_string());
    }
    if event.category != EventCategory::Os || event.r#type != "process.focus" {
        return None;
    }
    let meta = event.metadata.as_object()?;
    let app = meta
        .get("app_name")
        .and_then(|v| v.as_str())
        .unwrap_or(&event.source);
    let bundle = meta.get("bundle_id").and_then(|v| v.as_str()).unwrap_or("");
    let title = meta
        .get("window_title")
        .or_else(|| meta.get("tab_title"))
        .and_then(|v| v.as_str());
    Some(chronicle_core::tab_session::tab_session_key(
        app, bundle, title,
    ))
}

fn category_to_span_type(category: &EventCategory, event: &CanonicalEvent) -> SpanType {
    if *category == EventCategory::Os {
        if is_agent_app(event) {
            return SpanType::AiAssistant;
        }
        if is_terminal_app(event) {
            return SpanType::Terminal;
        }
        if is_ide_app(event) {
            return SpanType::Coding;
        }
        if is_browser_app(event) {
            return SpanType::Documentation;
        }
        if is_finder_app(event) {
            return SpanType::Documentation;
        }
        return SpanType::Idle;
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
#[cfg(test)]
mod tests {
    use super::*;
    use chronicle_core::{CanonicalEvent, EventCategory};

    #[test]
    fn exposes_open_span_before_close() {
        let mut p = SpanProcessor::new();
        let mut e = CanonicalEvent::new("Cursor", EventCategory::Os, "process.focus");
        e.metadata = serde_json::json!({"app_name": "Cursor"});
        e.timestamp = 1_000;
        assert!(p.process(&e).is_none());
        let active = p.active_spans();
        assert_eq!(active.len(), 1);
        assert!(active[0].ended_at.is_none());
        assert_eq!(active[0].span_type, SpanType::AiAssistant);
    }

    #[test]
    fn ghostty_maps_to_terminal_span() {
        let mut p = SpanProcessor::new();
        let mut e = CanonicalEvent::new("Ghostty", EventCategory::Os, "process.focus");
        e.metadata = serde_json::json!({"app_name": "Ghostty"});
        e.timestamp = 1_000;
        p.process(&e);
        assert_eq!(p.active_spans()[0].span_type, SpanType::Terminal);
    }

    #[test]
    fn safari_tab_change_closes_span() {
        let mut p = SpanProcessor::new();
        let mut a = CanonicalEvent::new("Safari", EventCategory::Os, "process.focus");
        a.metadata = serde_json::json!({
            "app_name": "Safari",
            "bundle_id": "com.apple.Safari",
            "window_title": "Rust Book",
            "tab_session_key": "safari|com.apple.safari|Rust Book"
        });
        a.timestamp = 1_000;
        p.process(&a);

        let mut b = CanonicalEvent::new("Safari", EventCategory::Os, "process.focus");
        b.metadata = serde_json::json!({
            "app_name": "Safari",
            "bundle_id": "com.apple.Safari",
            "window_title": "GitHub",
            "tab_session_key": "safari|com.apple.safari|GitHub"
        });
        b.timestamp = 2_000;
        let closed = p.process(&b).expect("tab change closes span");
        assert_eq!(closed.span_type, SpanType::Documentation);
        assert_eq!(p.active_spans().len(), 1);
    }

    #[test]
    fn only_one_active_span_at_a_time() {
        let mut p = SpanProcessor::new();
        let mut a = CanonicalEvent::new("Cursor", EventCategory::Os, "process.focus");
        a.metadata = serde_json::json!({"app_name": "Cursor"});
        a.project = Some("chronicle".into());
        a.timestamp = 1_000;
        p.process(&a);

        let mut b = CanonicalEvent::new("git", EventCategory::Git, "commit");
        b.project = Some("other".into());
        b.timestamp = 2_000;
        let closed = p.process(&b).expect("project change closes prior span");
        assert_eq!(closed.project.as_deref(), Some("chronicle"));
        assert_eq!(p.active_spans().len(), 1);
        assert_eq!(p.active_spans()[0].project.as_deref(), Some("other"));
    }
}
