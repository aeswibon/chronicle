//! Build focus events from monitor samples (one event per tab session).

use crate::event_filter;
use crate::focus_context::{FocusContext, FocusSnapshot};
use crate::project;
use chronicle_core::tab_session::{TabSessionChange, TabSessionTracker};
use chronicle_core::{CanonicalEvent, EventCategory};

#[derive(Default)]
pub struct FocusEmitter {
    tracker: TabSessionTracker,
}

impl FocusEmitter {
    pub fn reset(&mut self) {
        self.tracker.reset();
    }

    #[allow(clippy::too_many_arguments)]
    pub fn sample_to_event(
        &mut self,
        name: &str,
        bundle_id: &str,
        pid: i32,
        window_title: Option<String>,
        title_source: &str,
        timestamp_ms: i64,
        capture_relay: Option<&str>,
    ) -> Option<CanonicalEvent> {
        if event_filter::is_transient_focus_app(name, bundle_id) {
            return None;
        }

        let change = self
            .tracker
            .observe(name, bundle_id, window_title.as_deref())?;

        let mut event = CanonicalEvent::new(name, EventCategory::Os, "process.focus");
        event.timestamp = timestamp_ms;

        if let Some((project, root)) = window_title
            .as_deref()
            .and_then(project::detect_project_from_title)
        {
            event = event.with_project(&project);
            let meta = event.metadata.as_object_mut().unwrap();
            meta.insert("project_path".into(), root.to_string_lossy().into());
        }

        attach_focus_meta(
            &mut event,
            name,
            bundle_id,
            pid,
            &change,
            window_title,
            title_source,
            capture_relay,
        );
        Some(event)
    }

    pub fn update_focus_context(
        &self,
        focus: &FocusContext,
        name: &str,
        bundle_id: &str,
        pid: i32,
        window_title: Option<String>,
    ) {
        if event_filter::is_transient_focus_app(name, bundle_id) {
            return;
        }
        let detected_project = window_title
            .as_ref()
            .and_then(|title| project::detect_project_from_title(title).map(|(p, _)| p));
        focus.set(FocusSnapshot::from_app_info(
            name,
            bundle_id,
            pid,
            window_title,
            detected_project,
        ));
    }
}

#[allow(clippy::too_many_arguments)]
pub fn attach_focus_meta(
    event: &mut CanonicalEvent,
    name: &str,
    bundle_id: &str,
    pid: i32,
    change: &TabSessionChange,
    window_title: Option<String>,
    title_source: &str,
    capture_relay: Option<&str>,
) {
    let meta = event.metadata.as_object_mut().unwrap();
    meta.insert("app_name".into(), name.into());
    meta.insert("bundle_id".into(), bundle_id.into());
    meta.insert("pid".into(), pid.into());
    meta.insert("title_source".into(), title_source.into());
    meta.insert("capture_backend".into(), "nsworkspace".into());
    meta.insert(
        "tab_session_key".into(),
        change.tab_session_key.clone().into(),
    );
    meta.insert("tab_title".into(), change.tab_title.clone().into());
    meta.insert(
        "focus_kind".into(),
        if change.app_changed { "app" } else { "tab" }.into(),
    );
    if let Some(relay) = capture_relay {
        meta.insert("capture_relay".into(), relay.into());
    }
    if let Some(title) = window_title {
        meta.insert("window_title".into(), title.into());
    }
}

/// Backfill tab identity when focus events omit it (legacy relay or app-only metadata).
pub fn ensure_tab_session_meta(event: &mut CanonicalEvent) {
    if event.category != EventCategory::Os || event.r#type != "process.focus" {
        return;
    }
    let Some(obj) = event.metadata.as_object_mut() else {
        return;
    };
    if obj
        .get("tab_session_key")
        .and_then(|v| v.as_str())
        .is_some_and(|s| !s.is_empty())
    {
        return;
    }
    let app = obj
        .get("app_name")
        .and_then(|v| v.as_str())
        .unwrap_or(&event.source);
    let bundle = obj.get("bundle_id").and_then(|v| v.as_str()).unwrap_or("");
    let window_title = obj
        .get("window_title")
        .or_else(|| obj.get("tab_title"))
        .and_then(|v| v.as_str());
    let key = chronicle_core::tab_session::tab_session_key(app, bundle, window_title);
    let tab_title = window_title
        .map(chronicle_core::tab_session::normalize_tab_title)
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| app.to_string());
    obj.insert("tab_session_key".into(), key.into());
    obj.insert("tab_title".into(), tab_title.into());
    if !obj.contains_key("focus_kind") {
        obj.insert("focus_kind".into(), "app".into());
    }
}

#[cfg(test)]
mod ensure_meta_tests {
    use super::*;
    use chronicle_core::{CanonicalEvent, EventCategory};

    #[test]
    fn backfills_tab_session_key_from_app_metadata() {
        let mut event = CanonicalEvent::new("Ghostty", EventCategory::Os, "process.focus");
        event.metadata = serde_json::json!({
            "app_name": "Ghostty",
            "bundle_id": "com.mitchellh.ghostty",
            "activity_label": "terminal"
        });
        ensure_tab_session_meta(&mut event);
        assert!(event
            .metadata
            .get("tab_session_key")
            .and_then(|v| v.as_str())
            .is_some());
        assert_eq!(
            event.metadata.get("tab_title").and_then(|v| v.as_str()),
            Some("Ghostty")
        );
    }
}
