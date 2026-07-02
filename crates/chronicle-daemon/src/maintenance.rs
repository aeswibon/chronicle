//! Background maintenance: enrichment backfill, noise pruning, scheduled rollups.

use crate::rule_engine::RuleEngine;
use chronicle_ai::{build_daily_session, enrich_event, summarize_day, SummarySource};
use chronicle_core::Session;
use chronicle_store::Store;
use chrono::Timelike;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tracing::{info, warn};

pub fn local_day_start_ms(now: chrono::DateTime<chrono::Local>) -> i64 {
    let date = now.date_naive();
    date.and_hms_opt(0, 0, 0)
        .and_then(|t| t.and_local_timezone(chrono::Local).single())
        .map(|d| d.timestamp_millis())
        .unwrap_or_else(|| now.timestamp_millis())
}

pub fn day_end_exclusive(since: i64) -> i64 {
    chrono::DateTime::from_timestamp_millis(since)
        .and_then(|dt| {
            let local = dt.with_timezone(&chrono::Local);
            let next_day = local.date_naive().succ_opt()?;
            let midnight = next_day.and_hms_opt(0, 0, 0)?;
            midnight
                .and_local_timezone(chrono::Local)
                .single()
                .map(|d| d.timestamp_millis())
        })
        .unwrap_or(since + 86_400_000)
}

pub async fn summarize_and_persist_day(
    store: Arc<Mutex<Store>>,
    since: i64,
    until: i64,
) -> Result<(String, String, Option<String>, Session), String> {
    let ai_cfg = chronicle_config::load().ai;
    let day_end_exclusive = day_end_exclusive(since);

    let (spans, events) = {
        let guard = store.lock().await;
        let spans = guard
            .query_spans(since, Some(until), 200)
            .map_err(|e| format!("spans query failed: {e}"))?;
        let events = guard
            .query_activity_events(since, Some(until), 250)
            .map_err(|e| format!("events query failed: {e}"))?;
        (spans, events)
    };

    let outcome = summarize_day(&ai_cfg, since, until, &spans, &events).await;
    let source_str = match outcome.source {
        SummarySource::Ai => "ai",
        SummarySource::Rules => "rules",
    }
    .to_string();

    let session = build_daily_session(
        since,
        until,
        &spans,
        &events,
        outcome.summary.clone(),
        &source_str,
    );

    let guard = store.lock().await;
    guard
        .delete_sessions_between(since, day_end_exclusive)
        .map_err(|e| format!("session replace failed: {e}"))?;
    guard
        .insert_session(&session)
        .map_err(|e| format!("session persist failed: {e}"))?;

    Ok((outcome.summary, source_str, outcome.ai_error, session))
}

pub fn spawn_maintenance_tasks(store: Arc<Mutex<Store>>) {
    let enrich_store = store.clone();
    tokio::spawn(async move {
        backfill_enrichment(enrich_store).await;
    });

    let summary_store = store.clone();
    tokio::spawn(async move {
        auto_daily_summary_loop(summary_store).await;
    });
}

async fn backfill_enrichment(store: Arc<Mutex<Store>>) {
    let mut rule_engine = RuleEngine::new();
    let mut total = 0usize;

    loop {
        let batch = {
            let guard = store.lock().await;
            guard
                .query_events_needing_enrichment(250)
                .unwrap_or_default()
        };

        if batch.is_empty() {
            break;
        }

        for mut event in batch {
            rule_engine.process(&mut event);
            enrich_event(&mut event);
            let guard = store.lock().await;
            if let Err(e) = guard.update_event_metadata(&event) {
                warn!("enrichment backfill failed for {}: {e}", event.id);
            } else {
                total += 1;
            }
        }

        tokio::task::yield_now().await;
    }

    if total > 0 {
        info!("enrichment backfill: updated {total} events");
    }
}

async fn auto_daily_summary_loop(store: Arc<Mutex<Store>>) {
    loop {
        tokio::time::sleep(Duration::from_secs(3600)).await;

        let cfg = chronicle_config::load();
        if !cfg.summaries.auto_daily {
            continue;
        }

        let now = chrono::Local::now();
        if now.hour() < u32::from(cfg.summaries.auto_daily_hour_local) {
            continue;
        }

        let since = local_day_start_ms(now);
        let until = now.timestamp_millis();
        let day_end = day_end_exclusive(since);

        let missing = {
            let guard = store.lock().await;
            guard
                .has_session_in_range(since, day_end)
                .map(|exists| !exists)
                .unwrap_or(false)
        };

        if !missing {
            continue;
        }

        match summarize_and_persist_day(store.clone(), since, until).await {
            Ok((_, source, _, _)) => info!("auto daily summary persisted ({source})"),
            Err(e) => warn!("auto daily summary failed: {e}"),
        }
    }
}
