//! Local / OpenAI-compatible LLM summaries (Ollama default).

use crate::context::DayReportContext;
use chronicle_config::AiConfig;

const SYSTEM_PROMPT: &str = "You are a technical work-log assistant. Write ONLY the final daily developer summary — no reasoning, planning, or meta commentary. Output 3-5 complete sentences for a human reader. Lead with what was accomplished (projects, commits, pushes, tests, builds). Mention focus time only briefly. Ignore trivial terminal noise (cat, pbcopy, clipboard helpers, exploratory rm). Only mention failures if they blocked real work (build/test/deploy/git). Use the digest's local date and times. Do not invent facts. No bullet lists or markdown headers.";

/// Strip chain-of-thought leakage from reasoning models (e.g. smallthinker).
fn clean_summary(raw: &str) -> String {
    let mut text = raw.trim().to_string();
    if text.is_empty() {
        return text;
    }

    for marker in [
        "Let me think",
        "Let me check",
        "Wait, let me",
        "To answer this",
        "I need to",
        "I'll start by",
        "Now, let me",
        "As I reflect",
    ] {
        if let Some(idx) = text.find(marker) {
            if idx > 0 && text[..idx].trim().len() > 40 {
                text = text[..idx].trim().to_string();
                break;
            }
        }
    }

    if let Some(idx) = text.rfind("\n\n") {
        let tail = text[idx..].trim();
        let head = text[..idx].trim();
        if tail.len() > 60
            && (tail.contains("Today,")
                || tail.contains("Today ")
                || tail.contains("No blockers")
                || tail.contains("focused work"))
        {
            return tail.to_string();
        }
        if head.len() < 80 && tail.len() > head.len() {
            return tail.to_string();
        }
    }

    for prefix in ["Today, ", "Today "] {
        if let Some(idx) = text.find(prefix) {
            if idx > 0 {
                return text[idx..].trim().to_string();
            }
        }
    }

    text
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

#[cfg(test)]
mod tests {
    use super::{clean_summary, model_matches};

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
}
