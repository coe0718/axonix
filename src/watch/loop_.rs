//! `run_watch` — the main health watch event loop.

use crate::health::HealthSnapshot;
use crate::telegram::TelegramClient;
use crate::watch::alerts::{check_container_restarts, evaluate_thresholds};
use crate::watch::config::WatchConfig;
use crate::watch::alerts::AlertState;
use std::time::Instant;

/// Run the health watch loop.
///
/// Checks health at `config.interval`, sends Telegram alerts when thresholds are
/// exceeded, and respects `config.cooldown` to avoid flooding.
///
/// This function runs indefinitely — call it in a task or with a timeout.
/// Returns only on Telegram send error (which is logged but not fatal).
pub async fn run_watch(config: WatchConfig, tg: &TelegramClient) {
    let mut state = AlertState::default();

    // Startup notification
    let snapshot = HealthSnapshot::collect();
    let startup_msg = format!(
        "👁 *Axonix health watch started*\n\
         Checking every {}s, cooldown {}s\n\
         Thresholds: CPU>{:.1} | Mem>{}% | Disk>{}%\n\
         | Restarts: per-container 1h cooldown\n\
         Current: {}",
        config.interval.as_secs(),
        config.cooldown.as_secs(),
        config.cpu_threshold,
        config.mem_threshold,
        config.disk_threshold,
        snapshot.format_compact(),
    );
    tg.send_message(&startup_msg).await.ok();

    loop {
        tokio::time::sleep(config.interval).await;

        let snapshot = HealthSnapshot::collect();
        let alerts = evaluate_thresholds(&snapshot, &config, &state);

        let docker = crate::health::docker_health();
        let restart_alerts = check_container_restarts(&docker, &config, &state);

        for alert in &alerts {
            // Best-effort: log failures but don't break the watch loop
            if let Err(e) = tg.send_message(alert).await {
                eprintln!("  watch: alert send failed: {e}");
            }
        }

        for alert in &restart_alerts {
            if let Err(e) = tg.send_message(alert).await {
                eprintln!("  watch: restart alert send failed: {e}");
            }
        }

        // Update alert state after sending
        let now = Instant::now();
        if alerts.iter().any(|a| a.contains("CPU load")) {
            state.last_cpu_alert = Some(now);
        }
        if alerts.iter().any(|a| a.contains("memory")) {
            state.last_mem_alert = Some(now);
        }
        if alerts.iter().any(|a| a.contains("disk")) {
            state.last_disk_alert = Some(now);
        }
        // Update restart alert state for any containers that were alerted
        for alert in &restart_alerts {
            // Extract container name from "⚠️ *Container restarting*: <name>\n  Status: ..."
            if let Some(rest) = alert.strip_prefix("⚠️ *Container restarting*: ") {
                let name = rest.split('\n').next().unwrap_or("").to_string();
                if !name.is_empty() {
                    state.last_restart_alert.insert(name, now);
                }
            }
        }
    }
}
