//! Local / OpenAI-compatible LLM summaries (Ollama default).

use crate::context::DayReportContext;
use chronicle_config::AiConfig;

const SYSTEM_PROMPT: &str = "You are a technical work-log assistant. Write ONLY the final daily developer summary — no reasoning, planning, or meta commentary. Output 3-5 complete sentences for a human reader. Lead with what was accomplished (projects, commits, pushes, tests, builds). Mention focus time only briefly. Terminals (Ghostty, iTerm, Warp, etc.) are tools—not projects; describe work from git, shell, and file highlights instead of naming the terminal app. Ignore trivial terminal noise (cat, pbcopy, clipboard helpers, exploratory rm). Only mention failures if they blocked real work (build/test/deploy/git). Use the digest's local date and times. Do not invent facts. No bullet lists or markdown headers. Start with 'Today,' or the digest date.";

const COT_MARKERS: &[&str] = &[
    "let me think",
    "let me check",
    "let me craft",
    "let me put",
    "let me see",
    "to answer this",
    "i need to",
    "i'll start",
    "now, let me",
    "as i continue",
    "here's my attempt",
    "however, i need to rephrase",
    "wait a minute",
    "as i reflect",
    "first, i should",
    "carefully review",
    "extract the essential",
    "provided developer summary",
];

fn cot_marker_hits(text: &str) -> usize {
    let lower = text.to_lowercase();
    COT_MARKERS.iter().filter(|m| lower.contains(*m)).count()
}

/// True when output looks like a human-readable summary, not model reasoning.
pub fn is_acceptable_summary(text: &str) -> bool {
    let t = text.trim();
    if t.len() < 24 || t.len() > 1_200 {
        return false;
    }
    let hits = cot_marker_hits(t);
    if hits >= 1 {
        return false;
    }
    let lower = t.to_lowercase();
    !(lower.starts_with("to answer this")
        || lower.starts_with("let me think")
        || lower.starts_with("let me check")
        || lower.contains("carefully review"))
}

fn limit_sentences(text: &str, max: usize) -> String {
    let mut out = String::new();
    let mut count = 0usize;
    for part in text.split_inclusive(&['.', '!', '?'][..]) {
        let piece = part.trim();
        if piece.is_empty() {
            continue;
        }
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(piece);
        if !piece.ends_with('.') && !piece.ends_with('!') && !piece.ends_with('?') {
            out.push('.');
        }
        count += 1;
        if count >= max {
            break;
        }
    }
    out.trim().to_string()
}

fn extract_summary_body(text: &str) -> String {
    let mut s = text.trim().to_string();
    for stop in [
        "\n\nHowever,",
        "\n\nLet me ",
        "\n\nWait,",
        "\n\nNow, let me",
        "\n\nAs I ",
    ] {
        if let Some(idx) = s.find(stop) {
            s.truncate(idx);
        }
    }
    if cot_marker_hits(&s) >= 2 {
        if let Some(para) = s
            .split("\n\n")
            .map(str::trim)
            .filter(|p| cot_marker_hits(p) == 0 && p.len() > 40)
            .last()
        {
            s = para.to_string();
        }
    }
    limit_sentences(&s, 5)
}

/// Strip chain-of-thought leakage from reasoning models (e.g. smallthinker).
fn clean_summary(raw: &str) -> String {
    let text = raw.trim();
    if text.is_empty() {
        return String::new();
    }

    for marker in ["Here's my attempt:", "Overall,", "In summary,", "Summary:"] {
        if let Some(idx) = text.find(marker) {
            let tail = extract_summary_body(&text[idx + marker.len()..]);
            if is_acceptable_summary(&tail) {
                return tail;
            }
        }
    }

    if let Some(idx) = text.rfind("Today,").or_else(|| text.rfind("Today ")) {
        let tail = extract_summary_body(&text[idx..]);
        if is_acceptable_summary(&tail) {
            return tail;
        }
    }

    for marker in COT_MARKERS {
        if let Some(idx) = text.to_lowercase().find(marker) {
            if idx > 0 {
                let head = extract_summary_body(&text[..idx]);
                if is_acceptable_summary(&head) {
                    return head;
                }
            }
        }
    }

    extract_summary_body(text)
}

fn api_root(base_url: &str) -> String {
    base_url.trim_end_matches('/').to_string()
}

fn chat_completions_url(base_url: &str) -> String {
    let root = api_root(base_url);
    if root.ends_with("/v1/chat/completions") {
        root
    } else {
        format!("{root}/v1/chat/completions")
    }
}

fn ollama_tags_url(base_url: &str) -> String {
    let root = api_root(base_url);
    if root.ends_with("/v1/chat/completions") {
        root.trim_end_matches("/v1/chat/completions").to_string()
    } else {
        root
    }
    .trim_end_matches('/')
    .to_string()
        + "/api/tags"
}

fn resolve_api_key(config: &AiConfig) -> Option<String> {
    config
        .api_key_env
        .as_deref()
        .and_then(|var| std::env::var(var).ok())
        .filter(|v| !v.is_empty())
}

fn http_client(config: &AiConfig) -> anyhow::Result<reqwest::Client> {
    Ok(reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(config.timeout_secs))
        .build()?)
}

/// Match `smallthinker` against `smallthinker:latest` and similar Ollama names.
pub fn model_matches(available: &str, requested: &str) -> bool {
    let available = available.trim();
    let requested = requested.trim();
    if available == requested {
        return true;
    }
    let available_base = available.split(':').next().unwrap_or(available);
    let requested_base = requested.split(':').next().unwrap_or(requested);
    available_base == requested_base
        || available.starts_with(&format!("{requested_base}:"))
        || requested.starts_with(&format!("{available_base}:"))
}

pub async fn list_ollama_models(config: &AiConfig) -> anyhow::Result<Vec<String>> {
    let client = http_client(config)?;
    let url = ollama_tags_url(&config.base_url);
    let mut req = client.get(&url);
    if let Some(key) = resolve_api_key(config) {
        req = req.bearer_auth(key);
    }

    let resp = req.send().await.map_err(|e| {
        anyhow::anyhow!("could not reach Ollama at {url}: {e}. Is `ollama serve` running?")
    })?;
    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        anyhow::bail!("Ollama tags HTTP {status}: {text}");
    }

    let json: serde_json::Value = resp.json().await?;
    let models = json
        .pointer("/models")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|m| m.get("name").and_then(|n| n.as_str()))
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    Ok(models)
}

pub async fn test_connection(config: &AiConfig) -> anyhow::Result<String> {
    let model = config.model.trim();
    if model.is_empty() {
        anyhow::bail!("model name is empty");
    }

    let models = list_ollama_models(config).await?;
    if models.is_empty() {
        anyhow::bail!(
            "connected to Ollama at {} but no models are installed. Run `ollama pull {model}`.",
            api_root(&config.base_url)
        );
    }

    if !models.iter().any(|m| model_matches(m, model)) {
        anyhow::bail!(
            "model '{model}' not found. Installed: {}",
            models.join(", ")
        );
    }

    let client = http_client(config)?;
    let url = chat_completions_url(&config.base_url);
    let body = serde_json::json!({
        "model": model,
        "messages": [
            { "role": "user", "content": "Reply with exactly: ok" }
        ],
        "temperature": 0,
        "max_tokens": 8
    });

    let mut req = client.post(&url).json(&body);
    if let Some(key) = resolve_api_key(config) {
        req = req.bearer_auth(key);
    }

    let resp = req
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("chat request to {url} failed: {e}"))?;
    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        anyhow::bail!("LLM HTTP {status}: {text}");
    }

    let json: serde_json::Value = resp.json().await?;
    let content = json
        .pointer("/choices/0/message/content")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| anyhow::anyhow!("empty LLM response"))?;

    Ok(format!(
        "Connected to {} using model '{}'. Sample reply: {}",
        api_root(&config.base_url),
        model,
        content
    ))
}

pub async fn generate_summary(config: &AiConfig, ctx: &DayReportContext) -> anyhow::Result<String> {
    let user_content = ctx.to_prompt_text();
    let url = chat_completions_url(&config.base_url);
    let model = config.model.trim();

    let body = serde_json::json!({
        "model": model,
        "messages": [
            { "role": "system", "content": SYSTEM_PROMPT },
            { "role": "user", "content": user_content }
        ],
        "temperature": 0.3,
        "max_tokens": 400
    });

    let client = http_client(config)?;
    let mut req = client.post(&url).json(&body);
    if let Some(key) = resolve_api_key(config) {
        req = req.bearer_auth(key);
    }

    let resp = req
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("chat request to {url} failed: {e}"))?;
    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        anyhow::bail!("LLM HTTP {status}: {text}");
    }

    let json: serde_json::Value = resp.json().await?;
    let content = json
        .pointer("/choices/0/message/content")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| anyhow::anyhow!("empty LLM response"))?;

    Ok(clean_summary(content))
}

pub fn polish_summary(raw: &str) -> Option<String> {
    let cleaned = clean_summary(raw);
    if is_acceptable_summary(&cleaned) {
        Some(cleaned)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::{clean_summary, is_acceptable_summary, model_matches, polish_summary};

    #[test]
    fn model_matches_ollama_tags() {
        assert!(model_matches("smallthinker:latest", "smallthinker"));
        assert!(model_matches("smallthinker", "smallthinker:latest"));
        assert!(!model_matches("mistral:latest", "smallthinker"));
    }

    #[test]
    fn strips_chain_of_thought_prefix() {
        let raw =
            "To answer this, let me think... Today, I fixed the Homebrew cask and pushed v0.1.1.";
        let out = clean_summary(raw);
        assert!(out.starts_with("Today,"));
        assert!(!out.contains("let me think"));
    }

    #[test]
    fn rejects_long_chain_of_thought() {
        let raw = "To answer this, let me think about how I can craft a concise summary. \
            Let me check the stats. The developer worked on Ghostty. However, I need to rephrase.";
        assert!(!is_acceptable_summary(&clean_summary(raw)));
        assert!(polish_summary(raw).is_none());
    }

    #[test]
    fn rejects_smallthinker_meta_preamble() {
        let raw = "To answer this, I need to carefully review the provided developer summary \
            and extract the essential information in a concise manner.";
        assert!(!is_acceptable_summary(raw));
        assert!(polish_summary(raw).is_none());
    }

    #[test]
    fn accepts_clean_summary() {
        let raw = "Today, work centered on Ghostty UI improvements with Safari research breaks. \
            About 58 minutes of focused terminal work with no build or test failures.";
        let out = polish_summary(raw).expect("valid summary");
        assert!(is_acceptable_summary(&out));
        assert!(out.starts_with("Today,"));
    }
}
