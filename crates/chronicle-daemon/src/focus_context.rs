//! Tracks the frontmost app and gates capture to that context only.

use chronicle_core::{CanonicalEvent, EventCategory};
use std::path::Path;
use std::sync::RwLock;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FocusSnapshot {
    pub focused_at: i64,
    pub app_name: String,
    pub bundle_id: String,
    pub pid: i32,
    pub window_title: Option<String>,
    pub folder_path: Option<String>,
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

    /// Learn repo context from recorded work while terminal/IDE focus lacks a project.
    pub fn note_project(&self, project: &str) {
        if project.is_empty() {
            return;
        }
        if let Ok(mut guard) = self.current.write() {
            if let Some(snap) = guard.as_mut() {
                if snap.is_work_surface() && snap.project.is_none() {
                    snap.project = Some(project.to_string());
                }
            }
        }
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
        let folder_path = meta
            .get("folder_path")
            .and_then(|v| v.as_str())
            .map(str::to_string);
        let mut project = event.project.clone();
        if project.is_none()
            && (crate::app_classify::is_terminal_app_name(&app_name.to_lowercase(), &bundle_id.to_lowercase())
                || crate::app_classify::is_agent_app_name(&app_name.to_lowercase(), &bundle_id.to_lowercase()))
        {
            if let Some(title) = window_title.as_deref() {
                if let Some((name, _)) = crate::project::detect_project_from_title(title) {
                    project = Some(name);
                }
            }
        }

        self.set(FocusSnapshot {
            focused_at: event.timestamp,
            app_name,
            bundle_id,
            pid,
            window_title,
            folder_path,
            project,
        });
    }
}

/// Returns true when this event belongs to what the user is focused on right now.
pub fn applies_to_focus(event: &CanonicalEvent, focus: Option<&FocusSnapshot>) -> bool {
    if event.category == EventCategory::Os {
        return true;
    }

    let Some(focus) = focus else {
        return has_project_work_signal(event);
    };

    match event.category {
        EventCategory::Shell | EventCategory::Git | EventCategory::Build | EventCategory::Infrastructure => {
            work_surface_matches(event, focus)
        }
        EventCategory::Filesystem => {
            if focus.is_finder() {
                return filesystem_applies_to_finder_focus(event, focus);
            }
            work_surface_matches(event, focus)
        }
        EventCategory::Ide => focus.is_ide() && work_surface_matches(event, focus),
        EventCategory::Browser => focus.is_browser(),
        EventCategory::Ai => focus.is_agent(),
        _ => false,
    }
}

fn has_project_work_signal(event: &CanonicalEvent) -> bool {
    event.project.is_some()
        && matches!(
            event.category,
            EventCategory::Shell
                | EventCategory::Git
                | EventCategory::Filesystem
                | EventCategory::Build
                | EventCategory::Infrastructure
        )
}

fn filesystem_applies_to_finder_focus(event: &CanonicalEvent, focus: &FocusSnapshot) -> bool {
    let Some(path) = event
        .metadata
        .get("path")
        .and_then(|v| v.as_str())
        .map(Path::new)
    else {
        return false;
    };

    if let Some(folder) = focus.folder_path.as_deref().map(Path::new) {
        return path.starts_with(folder);
    }

    // Accessibility off: filesystem events are already scoped to watch dirs.
    true
}

fn work_surface_matches(event: &CanonicalEvent, focus: &FocusSnapshot) -> bool {
    if !focus.is_work_surface() {
        return false;
    }
    let Some(event_project) = event.project.as_deref() else {
        return false;
    };
    match focus.project.as_deref() {
        None => true,
        Some(focus_project) => event_project == focus_project,
    }
}

impl FocusSnapshot {
    pub fn from_app_info(
        name: &str,
        bundle_id: &str,
        pid: i32,
        window_title: Option<String>,
        folder_path: Option<String>,
        project: Option<String>,
    ) -> Self {
        Self {
            focused_at: chrono::Utc::now().timestamp_millis(),
            app_name: name.to_string(),
            bundle_id: bundle_id.to_string(),
            pid,
            window_title,
            folder_path,
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
            folder_path: None,
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
            folder_path: None,
            project: Some("chronicle".into()),
        };
        let mut git = CanonicalEvent::new("git", EventCategory::Git, "commit");
        git.project = Some("chronicle".into());
        assert!(applies_to_focus(&git, Some(&focus)));
    }

    #[test]
    fn allows_filesystem_when_finder_focused() {
        let focus = FocusSnapshot {
            focused_at: 0,
            app_name: "Finder".into(),
            bundle_id: "com.apple.finder".into(),
            pid: 1,
            window_title: Some("developer".into()),
            folder_path: Some("/Volumes/Seagate/developer".into()),
            project: None,
        };
        let mut fs = CanonicalEvent::new("fs", EventCategory::Filesystem, "file.created");
        fs.metadata = serde_json::json!({
            "path": "/Volumes/Seagate/developer/personal/notes.txt"
        });
        assert!(applies_to_focus(&fs, Some(&focus)));
    }

    #[test]
    fn blocks_filesystem_outside_finder_folder() {
        let focus = FocusSnapshot {
            focused_at: 0,
            app_name: "Finder".into(),
            bundle_id: "com.apple.finder".into(),
            pid: 1,
            window_title: Some("Downloads".into()),
            folder_path: Some("/Users/me/Downloads".into()),
            project: None,
        };
        let mut fs = CanonicalEvent::new("fs", EventCategory::Filesystem, "file.created");
        fs.metadata = serde_json::json!({
            "path": "/Volumes/Seagate/developer/personal/notes.txt"
        });
        assert!(!applies_to_focus(&fs, Some(&focus)));
    }

    #[test]
    fn allows_filesystem_without_folder_path_when_finder_focused() {
        let focus = FocusSnapshot {
            focused_at: 0,
            app_name: "Finder".into(),
            bundle_id: "com.apple.finder".into(),
            pid: 1,
            window_title: Some("developer".into()),
            folder_path: None,
            project: None,
        };
        let mut fs = CanonicalEvent::new("fs", EventCategory::Filesystem, "file.created");
        fs.metadata = serde_json::json!({
            "path": "/Volumes/Seagate/developer/personal/notes.txt"
        });
        assert!(applies_to_focus(&fs, Some(&focus)));
    }

    #[test]
    fn allows_shell_when_terminal_focused_without_project() {
        let focus = FocusSnapshot {
            focused_at: 0,
            app_name: "Ghostty".into(),
            bundle_id: "com.mitchellh.ghostty".into(),
            pid: 1,
            window_title: Some("chronicle — zsh".into()),
            folder_path: None,
            project: None,
        };
        let mut shell = CanonicalEvent::new("zsh", EventCategory::Shell, "command.completed");
        shell.project = Some("chronicle".into());
        assert!(applies_to_focus(&shell, Some(&focus)));
    }

    #[test]
    fn blocks_shell_when_terminal_focused_on_different_project() {
        let focus = FocusSnapshot {
            focused_at: 0,
            app_name: "Ghostty".into(),
            bundle_id: "com.mitchellh.ghostty".into(),
            pid: 1,
            window_title: None,
            folder_path: None,
            project: Some("other-repo".into()),
        };
        let mut shell = CanonicalEvent::new("zsh", EventCategory::Shell, "command.completed");
        shell.project = Some("chronicle".into());
        assert!(!applies_to_focus(&shell, Some(&focus)));
    }

    #[test]
    fn blocks_shell_when_finder_focused() {
        let focus = FocusSnapshot {
            focused_at: 0,
            app_name: "Finder".into(),
            bundle_id: "com.apple.finder".into(),
            pid: 1,
            window_title: None,
            folder_path: None,
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
    } else if snap.is_browser() || snap.is_finder() {
        SpanType::Documentation
    } else {
        SpanType::Idle
    };
    let now = chrono::Utc::now().timestamp_millis();
    let started = if snap.focused_at > 0 {
        snap.focused_at
    } else {
        now
    };
    let project = snap.project.clone().or_else(|| {
        snap.window_title
            .as_deref()
            .and_then(crate::project::detect_project_from_title)
            .map(|(name, _)| name)
    });
    let mut span = Span::new(span_type, project);
    span.started_at = started;
    span.ended_at = None;
    span.duration_ms = Some((now - started).max(0) as u64);
    span.event_count = 0;
    if let Some(obj) = span.metadata.as_object_mut() {
        obj.insert("from_focus".into(), true.into());
        obj.insert("app_name".into(), snap.app_name.clone().into());
        if let Some(path) = &snap.folder_path {
            obj.insert("folder_path".into(), path.clone().into());
        }
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
                    snap.folder_path.as_deref(),
                )
                .into(),
            );
        }
    }
    Some(span)
}
