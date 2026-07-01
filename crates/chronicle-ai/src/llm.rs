//! Local / OpenAI-compatible LLM summaries (Ollama default).

use crate::context::DayReportContext;
use chronicle_config::AiConfig;

const SYSTEM_PROMPT: &str = "You are a technical work-log assistant. Write a concise daily developer summary in 3-6 complete sentences. Cover: main projects touched, meaningful work accomplished, test/build/deploy activity, failures worth revisiting, and how focus time was spent. Write in past tense, first person optional. Do not invent facts not present in the digest. No markdown headers.";

pub async fn generate_summary(config: &AiConfig, ctx: &DayReportContext) -> anyhow::Result<String> {
    let user_content = ctx.to_prompt_text();
    let api_key = config
        .api_key_env
        .as_deref()
        .and_then(|var| std::env::var(var).ok());

    let url = if config.base_url.ends_with("/v1/chat/completions") {
        config.base_url.clone()
    } else {
        format!(
            "{}/v1/chat/completions",
            config.base_url.trim_end_matches('/')
        )
    };

    let body = serde_json::json!({
        "model": config.model,
        "messages": [
            { "role": "system", "content": SYSTEM_PROMPT },
            { "role": "user", "content": user_content }
        ],
        "temperature": 0.3,
        "max_tokens": 400
    });

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(config.timeout_secs))
        .build()?;

    let mut req = client.post(&url).json(&body);
    if let Some(key) = api_key {
        req = req.bearer_auth(key);
    }

    let resp = req.send().await?;
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

    Ok(content.to_string())
}
