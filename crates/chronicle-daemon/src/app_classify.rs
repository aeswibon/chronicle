//! Shared front-app classification for spans, rules, and focus gating.

use chronicle_core::CanonicalEvent;

pub fn app_name_bundle(event: &CanonicalEvent) -> (String, String) {
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
    (app, bundle)
}

pub fn is_agent_app_name(app: &str, bundle: &str) -> bool {
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
        .any(|n| app.contains(n) || bundle.contains(n))
        || bundle.contains("todesktop")
}

pub fn is_terminal_app_name(app: &str, _bundle: &str) -> bool {
    [
        "terminal",
        "iterm",
        "warp",
        "ghostty",
        "alacritty",
        "kitty",
        "wezterm",
    ]
    .iter()
    .any(|n| app.contains(n))
}

pub fn is_ide_app_name(app: &str, _bundle: &str) -> bool {
    app.contains("xcode")
        || app.contains("intellij")
        || app.contains("android studio")
        || (app.contains("code") && !app.contains("cursor"))
}

pub fn is_browser_app_name(app: &str, bundle: &str) -> bool {
    [
        "safari", "chrome", "firefox", "brave", "arc", "edge", "opera", "vivaldi", "chromium",
    ]
    .iter()
    .any(|n| app.contains(n) || bundle.contains(n))
}

pub fn is_finder_app_name(app: &str, bundle: &str) -> bool {
    app.contains("finder") || bundle.contains("com.apple.finder")
}

pub fn is_agent_app(event: &CanonicalEvent) -> bool {
    let (app, bundle) = app_name_bundle(event);
    is_agent_app_name(&app, &bundle)
}

pub fn is_terminal_app(event: &CanonicalEvent) -> bool {
    let (app, bundle) = app_name_bundle(event);
    is_terminal_app_name(&app, &bundle)
}

pub fn is_ide_app(event: &CanonicalEvent) -> bool {
    let (app, bundle) = app_name_bundle(event);
    is_ide_app_name(&app, &bundle)
}

pub fn is_browser_app(event: &CanonicalEvent) -> bool {
    let (app, bundle) = app_name_bundle(event);
    is_browser_app_name(&app, &bundle)
}

pub fn is_finder_app(event: &CanonicalEvent) -> bool {
    let (app, bundle) = app_name_bundle(event);
    is_finder_app_name(&app, &bundle)
}
