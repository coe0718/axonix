//! Brief::collect() — assembles the morning brief from disk.

use crate::predictions::PredictionStore;
use super::types::{Brief, LastSessionSummary};
use super::parsers::{
    parse_active_goals, parse_recent_metrics, parse_recent_journal_entries,
    collect_health_summary, collect_open_predictions, collect_predictions_due_soon,
    collect_memory_context,
};
use super::priority::synthesize_priority;

impl Brief {
    /// Build the morning brief from disk.
    pub fn collect() -> Self {
        let active_goals = parse_active_goals();
        let open_predictions = collect_open_predictions();
        let recent_sessions = parse_recent_metrics(3);
        let health = collect_health_summary();

        use crate::bluesky::BlueskyHistory;
        let history = BlueskyHistory::default_path();
        let bluesky_stats = if !history.is_empty() {
            let (total, root, _replies) = history.stats();
            let last_date = history.last_root_post_date();
            Some((total, root, last_date))
        } else {
            None
        };

        let caddy_url_configured = std::env::var("CADDY_ADMIN_URL").is_ok();
        let caddy_result = crate::health::caddy_health();
        let caddy = if caddy_result.reachable || caddy_url_configured {
            Some(caddy_result)
        } else {
            None
        };

        // Collect Docker container health (graceful fallback when unavailable).
        let docker_host_configured = std::env::var("DOCKER_HOST").is_ok();
        let docker_result = crate::health::docker_health();
        let docker = if docker_result.error.is_none() || docker_host_configured {
            Some(docker_result)
        } else {
            None
        };

        // Compute calibration score from resolved predictions.
        let pred_store = PredictionStore::default_path();
        let score = pred_store.calibration_score();
        let calibration = if score.total_resolved > 0 { Some(score) } else { None };

        // Load last cycle summary from .axonix/cycle_summary.json
        let last_session = {
            let cs = crate::cycle_summary::CycleSummary::default_path();
            cs.data.map(|d| LastSessionSummary {
                session: d.session.clone(),
                date: d.date.clone(),
                completed: d.completed.clone(),
                test_count: d.test_count,
            })
        };

        // Load failure pattern summary from .axonix/failure_patterns.json
        let failure_summary = {
            let store = crate::failure_patterns::FailurePatternStore::default_path();
            if store.total_count() > 0 {
                store.most_common_failure_type().map(|ft| {
                    format!("⚠️  Most common failure: {} ({} total)", ft, store.total_count())
                })
            } else {
                None
            }
        };

        let recent_journal = parse_recent_journal_entries(3);

        let predictions_due_soon = collect_predictions_due_soon();
        let memory_context = if !active_goals.is_empty() {
            collect_memory_context(&active_goals[0])
        } else {
            vec![]
        };

        // Collect infrastructure anomalies from docker container state.
        let infrastructure_anomalies: Vec<String> = if let Some(ref d) = docker {
            d.containers.iter()
                .filter(|c| c.anomaly)
                .map(|c| format!("⚠ {} — {}", c.name, c.status))
                .collect()
        } else {
            vec![]
        };

        let mut brief = Brief {
            active_goals,
            open_predictions,
            recent_sessions,
            note: None,
            health,
            bluesky_stats,
            caddy,
            docker,
            calibration,
            last_session,
            failure_summary,
            pogo: {
                let data = crate::pogo::PogoData::fetch();
                if !data.is_empty() { Some(data) } else { None }
            },
            meta_health: Some(crate::meta_health::MetaHealthCheck::run()),
            today_priority: String::new(),
            recent_journal,
            predictions_due_soon,
            memory_context,
            infrastructure_anomalies,
        };
        brief.today_priority = synthesize_priority(&brief);
        brief
    }
}
