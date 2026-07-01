//! Tab-session identity: one capture unit per app tab/window (not per title animation tick).

/// Stable tab/window label with dynamic suffixes stripped (spinners, dirty markers, loading).
pub fn normalize_tab_title(title: &str) -> String {
    let mut t = title.trim().to_string();
    if t.is_empty() {
        return t;
    }

    // Agent / terminal status suffixes: "project — ⏳ Working .··"
    for marker in [" - ⏳", " — ⏳", " – ⏳", " ⏳"] {
        if let Some(idx) = t.find(marker) {
            t.truncate(idx);
            break;
        }
    }

    // Leading dirty / unsaved markers (Cursor, VS Code, Xcode, etc.)
    loop {
        let trimmed = t.trim_start();
        let stripped = trimmed
            .strip_prefix('●')
            .or_else(|| trimmed.strip_prefix('•'))
            .or_else(|| trimmed.strip_prefix('◦'))
            .or_else(|| trimmed.strip_prefix('*'))
            .or_else(|| trimmed.strip_prefix('○'))
            .map(str::trim_start);
        match stripped {
            Some(rest) if rest.len() < trimmed.len() => t = rest.to_string(),
            _ => break,
        }
    }

    // Trailing dirty markers and loading tails
    loop {
        let trimmed = t.trim_end();
        let mut changed = false;
        let without_marker = trimmed
            .strip_suffix('●')
            .or_else(|| trimmed.strip_suffix('•'))
            .or_else(|| trimmed.strip_suffix('◦'))
            .or_else(|| trimmed.strip_suffix('*'))
            .or_else(|| trimmed.strip_suffix('○'))
            .map(str::trim_end);
        if let Some(rest) = without_marker {
            if rest.len() < trimmed.len() {
                t = rest.to_string();
                changed = true;
            }
        }
        let lower = t.to_ascii_lowercase();
        for suffix in [
            " (unsaved)",
            " — unsaved",
            " - unsaved",
            " (modified)",
            " — modified",
            " - modified",
            " — loading",
            " - loading",
            " …",
        ] {
            if lower.ends_with(suffix) {
                t.truncate(t.len().saturating_sub(suffix.len()));
                t = t.trim_end().to_string();
                changed = true;
                break;
            }
        }
        if !changed {
            break;
        }
    }

    // Trailing progress / spinner noise
    while t.ends_with('.') || t.ends_with('·') || t.ends_with(' ') || t.ends_with('…') {
        t.pop();
    }

    t.trim().to_string()
}

/// Identity for a tab/window session: app + bundle + normalized title.
pub fn tab_session_key(app_name: &str, bundle_id: &str, window_title: Option<&str>) -> String {
    let app = app_name.to_lowercase();
    let bundle = bundle_id.to_lowercase();
    let tab = window_title
        .map(normalize_tab_title)
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| "_default".into());
    format!("{app}|{bundle}|{tab}")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TabSessionChange {
    pub tab_session_key: String,
    pub tab_title: String,
    pub app_changed: bool,
}

#[derive(Debug, Default)]
pub struct TabSessionTracker {
    last_key: Option<String>,
    last_app: Option<String>,
}

impl TabSessionTracker {
    pub fn observe(
        &mut self,
        app_name: &str,
        bundle_id: &str,
        window_title: Option<&str>,
    ) -> Option<TabSessionChange> {
        let key = tab_session_key(app_name, bundle_id, window_title);
        if self.last_key.as_deref() == Some(key.as_str()) {
            return None;
        }
        let app_lower = app_name.to_lowercase();
        let app_changed = self.last_app.as_deref() != Some(app_lower.as_str());
        self.last_key = Some(key.clone());
        self.last_app = Some(app_lower);
        let tab_title = window_title
            .map(normalize_tab_title)
            .filter(|t| !t.is_empty())
            .unwrap_or_else(|| app_name.to_string());
        Some(TabSessionChange {
            tab_session_key: key,
            tab_title,
            app_changed,
        })
    }

    pub fn reset(&mut self) {
        self.last_key = None;
        self.last_app = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_ghostty_spinner_suffix() {
        assert_eq!(
            normalize_tab_title("Build UI Better - ⏳ Working .··"),
            "Build UI Better"
        );
    }

    #[test]
    fn strips_ide_dirty_marker() {
        assert_eq!(
            normalize_tab_title("● lib.rs — chronicle"),
            "lib.rs — chronicle"
        );
        assert_eq!(
            normalize_tab_title("Settings.tsx — chronicle ●"),
            "Settings.tsx — chronicle"
        );
        assert_eq!(normalize_tab_title("main.rs (unsaved)"), "main.rs");
    }

    #[test]
    fn same_tab_despite_spinner_change() {
        let mut t = TabSessionTracker::default();
        let a = t.observe(
            "Ghostty",
            "com.mitchellh.ghostty",
            Some("Build UI Better - ⏳ Working .··"),
        );
        assert!(a.is_some());
        let b = t.observe(
            "Ghostty",
            "com.mitchellh.ghostty",
            Some("Build UI Better - ⏳ Working ..."),
        );
        assert!(b.is_none());
    }

    #[test]
    fn same_tab_despite_dirty_toggle() {
        let mut t = TabSessionTracker::default();
        t.observe("Cursor", "com.todesktop.cursor", Some("lib.rs — chronicle"));
        assert!(t
            .observe(
                "Cursor",
                "com.todesktop.cursor",
                Some("● lib.rs — chronicle")
            )
            .is_none());
    }

    #[test]
    fn new_tab_when_title_changes() {
        let mut t = TabSessionTracker::default();
        t.observe("Cursor", "com.cursor", Some("chronicle — lib.rs"));
        let next = t.observe("Cursor", "com.cursor", Some("other — main.rs"));
        assert!(next.is_some());
        assert!(!next.unwrap().app_changed);
    }

    #[test]
    fn browser_tab_sessions() {
        let mut t = TabSessionTracker::default();
        t.observe(
            "Safari",
            "com.apple.Safari",
            Some("Rust Book - The Rust Programming Language"),
        );
        let next = t.observe(
            "Safari",
            "com.apple.Safari",
            Some("Chronicle README - GitHub"),
        );
        assert!(next.is_some());
        assert!(!next.unwrap().app_changed);
    }

    #[test]
    fn finder_window_sessions() {
        let mut t = TabSessionTracker::default();
        t.observe("Finder", "com.apple.finder", Some("developer"));
        let next = t.observe("Finder", "com.apple.finder", Some("Downloads"));
        assert!(next.is_some());
    }
}
