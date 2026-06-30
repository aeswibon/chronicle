#[cfg(target_os = "macos")]
mod platform {
    use base64::{engine::general_purpose::STANDARD, Engine};
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::process::{Command, Stdio};
    use std::sync::{Mutex, OnceLock};

    const MAX_ICONS_PER_BATCH: usize = 24;

    static CACHE: OnceLock<Mutex<HashMap<String, Option<String>>>> = OnceLock::new();

    fn cache() -> &'static Mutex<HashMap<String, Option<String>>> {
        CACHE.get_or_init(|| Mutex::new(HashMap::new()))
    }

    fn helper_bin() -> PathBuf {
        PathBuf::from(env!("CHRONICLE_ICON_HELPER"))
    }

    fn cache_key_app(bundle_id: &str) -> String {
        format!("app:{bundle_id}")
    }

    fn cache_key_path(path: &str) -> String {
        format!("path:{path}")
    }

    fn get_cached(key: &str) -> Option<Option<String>> {
        cache().lock().ok()?.get(key).cloned()
    }

    fn set_cached(key: String, value: Option<String>) {
        if let Ok(mut guard) = cache().lock() {
            guard.insert(key, value);
        }
    }

    fn escape_mdfind(value: &str) -> String {
        value.replace('\\', "\\\\").replace('\'', "\\'")
    }

    fn app_path_for_bundle(bundle_id: &str) -> Option<String> {
        let id = escape_mdfind(bundle_id);
        let output = Command::new("/usr/bin/mdfind")
            .arg(format!("kMDItemCFBundleIdentifier == '{id}'"))
            .output()
            .ok()?;

        if !output.status.success() {
            return None;
        }

        String::from_utf8_lossy(&output.stdout)
            .lines()
            .find(|line| line.ends_with(".app"))
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(str::to_string)
    }

    fn fetch_png(path: &str) -> Option<Vec<u8>> {
        if !std::path::Path::new(path).exists() {
            return None;
        }

        let output = Command::new(helper_bin())
            .arg(path)
            .arg("32")
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .output()
            .ok()?;

        if !output.status.success() || output.stdout.is_empty() {
            return None;
        }

        Some(output.stdout)
    }

    fn png_to_data_url(png: Vec<u8>) -> String {
        format!("data:image/png;base64,{}", STANDARD.encode(png))
    }

    pub fn resolve_app_icon(
        bundle_id: Option<String>,
        _app_name: Option<String>,
    ) -> Option<String> {
        let bundle_id = bundle_id.filter(|s| !s.is_empty())?;
        let key = cache_key_app(&bundle_id);
        if let Some(hit) = get_cached(&key) {
            return hit;
        }

        let data_url = app_path_for_bundle(&bundle_id)
            .and_then(|path| fetch_png(&path))
            .map(png_to_data_url);

        set_cached(key, data_url.clone());
        data_url
    }

    pub fn resolve_path_icon(path: String) -> Option<String> {
        let trimmed = path.trim();
        if trimmed.is_empty() {
            return None;
        }

        let key = cache_key_path(trimmed);
        if let Some(hit) = get_cached(&key) {
            return hit;
        }

        let data_url = fetch_png(trimmed).map(png_to_data_url);
        set_cached(key, data_url.clone());
        data_url
    }

    pub fn resolve_path_icons(paths: Vec<String>) -> HashMap<String, String> {
        let mut out = HashMap::new();
        for path in paths.into_iter().take(MAX_ICONS_PER_BATCH) {
            if let Some(url) = resolve_path_icon(path.clone()) {
                out.insert(path, url);
            }
        }
        out
    }
}

#[cfg(not(target_os = "macos"))]
mod platform {
    use std::collections::HashMap;

    pub fn resolve_app_icon(
        _bundle_id: Option<String>,
        _app_name: Option<String>,
    ) -> Option<String> {
        None
    }

    pub fn resolve_path_icon(_path: String) -> Option<String> {
        None
    }

    pub fn resolve_path_icons(_paths: Vec<String>) -> HashMap<String, String> {
        HashMap::new()
    }
}

pub use platform::{resolve_app_icon, resolve_path_icon, resolve_path_icons};
