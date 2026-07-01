//! Tracks the frontmost app and gates capture to that context only.

use chronicle_core::{CanonicalEvent, EventCategory};
use std::sync::RwLock;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FocusSnapshot {
    pub focused_at: i64,
    pub app_name: String,
    pub bundle_id: String,
    pub pid: i32,
    pub window_title: Option<String>,
    pub project: Option<String>,
}

#[derive(Default)]
pub struct FocusContext {
    current: RwLock<Option<FocusSnapshot>>,
}

impl FocusContext {
    pub fn set(&self, snapshot: FocusSnapshot) {
        if let Ok(mut guard) = self.current.write() {
            *guard = Some(snapshot);
        }
    }

    pub fn snapshot(&self) -> Option<FocusSnapshot> {
        self.current.read().ok().and_then(|g| g.clone())
    }

    pub fn update_from_event(&self, event: &CanonicalEvent) {
        if event.category != EventCategory::Os {
            return;
        }
        if !matches!(event.r#type.as_str(), "process.focus" | "window.focus") {
            return;
        }
        let meta = &event.metadata;
        let app_name = meta
            .get("app_name")
            .and_then(|v| v.as_str())
            .unwrap_or(&event.source)
            .to_string();
        let bundle_id = meta
            .get("bundle_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let pid = meta.get("pid").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
        let window_title = meta
            .get("window_title")
            .and_then(|v| v.as_str())
            .map(str::to_string);
        self.set(FocusSnapshot {
            focused_at: event.timestamp,
            app_name,
            bundle_id,
            pid,
            window_title,
            project: event.project.clone(),
        });
    }
}

/// Returns true when this event belongs to what the user is focused on right now.
pub fn applies_to_focus(event: &CanonicalEvent, focus: Option<&FocusSnapshot>) -> bool {
    if event.category == EventCategory::Os {
        return true;
    }

    let Some(focus) = focus else {
        return false;
    };

    match event.category {
        EventCategory::Shell => focus.is_work_surface() && project_matches(event, focus),
        EventCategory::Git | EventCategory::Filesystem => {
            focus.is_work_surface() && project_matches(event, focus)
        }
        EventCategory::Ide => focus.is_ide() && project_matches(event, focus),
        EventCategory::Browser => focus.is_browser(),
        EventCategory::Build | EventCategory::Infrastructure => {
            focus.is_work_surface() && project_matches(event, focus)
        }
        EventCategory::Ai => focus.is_agent(),
        _ => false,
    }
}

fn project_matches(event: &CanonicalEvent, focus: &FocusSnapshot) -> bool {
    let Some(event_project) = event.project.as_deref() else {
        return false;
    };
    let Some(focus_project) = focus.project.as_deref() else {
        return false;
    };
    event_project == focus_project
}

impl FocusSnapshot {
    pub fn from_app_info(
        name: &str,
        bundle_id: &str,
        pid: i32,
        window_title: Option<String>,
        project: Option<String>,
    ) -> Self {
        Self {
            focused_at: chrono::Utc::now().timestamp_millis(),
            app_name: name.to_string(),
            bundle_id: bundle_id.to_string(),
            pid,
            window_title,
            project,
        }
    }

    fn app_lower(&self) -> String {
        self.app_name.to_lowercase()
    }

    fn bundle_lower(&self) -> String {
        self.bundle_id.to_lowercase()
    }

    pub fn is_agent(&self) -> bool {
        crate::app_classify::is_agent_app_name(&self.app_lower(), &self.bundle_lower())
    }

    pub fn is_terminal(&self) -> bool {
        crate::app_classify::is_terminal_app_name(&self.app_lower(), &self.bundle_lower())
    }

    pub fn is_ide(&self) -> bool {
        crate::app_classify::is_ide_app_name(&self.app_lower(), &self.bundle_lower())
    }

    pub fn is_browser(&self) -> bool {
        crate::app_classify::is_browser_app_name(&self.app_lower(), &self.bundle_lower())
    }

    pub fn is_finder(&self) -> bool {
        crate::app_classify::is_finder_app_name(&self.app_lower(), &self.bundle_lower())
    }

    pub fn is_work_surface(&self) -> bool {
        self.is_agent() || self.is_terminal() || self.is_ide()
    }

    pub fn is_window_surface(&self) -> bool {
        self.is_work_surface() || self.is_browser() || self.is_finder()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chronicle_core::CanonicalEvent;

    #[test]
    fn blocks_background_git_when_focused_elsewhere() {
        let focus = FocusSnapshot {
            focused_at: 0,
            app_name: "Cursor".into(),
            bundle_id: "com.cursor".into(),
            pid: 1,
            window_title: Some("chronicle — lib.rs".into()),
            project: Some("chronicle".into()),
        };
        let mut git = CanonicalEvent::new("git", EventCategory::Git, "commit");
        git.project = Some("other-repo".into());
        assert!(!applies_to_focus(&git, Some(&focus)));
    }

    #[test]
    fn allows_git_for_focused_project() {
        let focus = FocusSnapshot {
            focused_at: 0,
            app_name: "Cursor".into(),
            bundle_id: "com.cursor".into(),
            pid: 1,
            window_title: None,
            project: Some("chronicle".into()),
        };
        let mut git = CanonicalEvent::new("git", EventCategory::Git, "commit");
        git.project = Some("chronicle".into());
        assert!(applies_to_focus(&git, Some(&focus)));
    }

    #[test]
    fn blocks_shell_when_finder_focused() {
        let focus = FocusSnapshot {
            focused_at: 0,
            app_name: "Finder".into(),
            bundle_id: "com.apple.finder".into(),
            pid: 1,
            window_title: None,
            project: None,
        };
        let mut shell = CanonicalEvent::new("bash", EventCategory::Shell, "command.completed");
        shell.project = Some("chronicle".into());
        assert!(!applies_to_focus(&shell, Some(&focus)));
    }
}

use chronicle_core::{Span, SpanType};

/// Live session chip when span processor has not caught up yet.
pub fn open_span_from_focus(snap: &FocusSnapshot) -> Option<Span> {
    if !snap.is_window_surface() {
        return None;
    }
    let span_type = if snap.is_agent() {
        SpanType::AiAssistant
    } else if snap.is_terminal() {
        SpanType::Terminal
    } else if snap.is_ide() {
        SpanType::Coding
    } else {
        SpanType::Documentation
    };
    let now = chrono::Utc::now().timestamp_millis();
    let started = if snap.focused_at > 0 {
        snap.focused_at
    } else {
        now
    };
    let mut span = Span::new(span_type, snap.project.clone());
    span.started_at = started;
    span.ended_at = None;
    span.duration_ms = Some((now - started).max(0) as u64);
    span.event_count = 0;
    if let Some(obj) = span.metadata.as_object_mut() {
        obj.insert("from_focus".into(), true.into());
        obj.insert("app_name".into(), snap.app_name.clone().into());
        if let Some(title) = &snap.window_title {
            let tab = chronicle_core::tab_session::normalize_tab_title(title);
            obj.insert("tab_title".into(), tab.clone().into());
            obj.insert("window_title".into(), title.clone().into());
            obj.insert(
                "tab_session_key".into(),
                chronicle_core::tab_session::tab_session_key(
                    &snap.app_name,
                    &snap.bundle_id,
                    Some(title.as_str()),
                )
                .into(),
            );
        }
    }
    Some(span)
}
