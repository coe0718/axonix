//! `/watch` command handler extracted from `handle_command`.

use super::types::CommandResult;

/// Handle the `/watch` command: display health snapshot vs configured thresholds.
pub fn handle_watch() -> CommandResult {
    let snap = crate::health::HealthSnapshot::collect();
    let config = crate::watch::WatchConfig::default();
    let watch_state = crate::watch::AlertState::default_for_repl();
    let alerts = crate::watch::evaluate_thresholds(&snap, &config, &watch_state);

    let mut lines = vec![
        format!("  🔍 Health vs thresholds (CPU>{:.1} | Mem>{}% | Disk>{}%):",
            config.cpu_threshold, config.mem_threshold, config.disk_threshold),
        format!("  CPU load:  {}", snap.load_avg),
        format!("  Memory:    {}", snap.memory),
        format!("  Disk (/):  {}", snap.disk),
        format!("  Uptime:    {}", snap.uptime),
        String::new(),
    ];
    if alerts.is_empty() {
        lines.push("  ✅ All metrics within thresholds".to_string());
    } else {
        lines.push(format!("  ⚠ {} threshold(s) exceeded:", alerts.len()));
        for alert in &alerts {
            let first_line = alert.lines().next().unwrap_or("").trim();
            lines.push(format!("    {first_line}"));
        }
        lines.push(String::new());
        lines.push("  Use --watch to send alerts via Telegram.".to_string());
    }
    lines.push(String::new());
    CommandResult::Handled(lines)
}
