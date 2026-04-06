//! Listener configuration, stats, and helper utilities.

use std::collections::HashSet;

/// Default Haiku model for lightweight listener tasks
pub const DEFAULT_HAIKU_MODEL: &str = "claude-haiku-4-5-20251001";

/// Select which model to use based on Telegram command type.
/// /run needs full reasoning → Sonnet. Other commands are cheap → Haiku.
pub fn select_model_for_command(command: &str, sonnet_model: &str, haiku_model: &str) -> String {
    match command.trim() {
        cmd if cmd.starts_with("/run") => sonnet_model.to_string(),
        _ => haiku_model.to_string(),
    }
}

/// Parse `LISTENER_RATE_LIMIT` env var (format: "N/Ss" e.g. "5/60s").
/// Returns (max_count, window_secs). Falls back to provided defaults on parse error.
///
/// Examples of valid values: "5/60s", "10/30s", "3/120s"
/// The trailing "s" is optional: "10/30" is also accepted.
pub fn parse_rate_limit_env(default_max: u32, default_window: u64) -> (u32, u64) {
    if let Ok(val) = std::env::var("LISTENER_RATE_LIMIT") {
        // Expected format: "5/60s" or "10/30s"
        let val = val.trim().trim_end_matches('s');
        if let Some((n_str, w_str)) = val.split_once('/') {
            if let (Ok(n), Ok(w)) = (n_str.trim().parse::<u32>(), w_str.trim().parse::<u64>()) {
                return (n, w);
            }
        }
    }
    (default_max, default_window)
}

// ── Config ────────────────────────────────────────────────────────────────────

/// Configuration for the always-on Telegram listener.
#[derive(Debug, Clone)]
pub struct ListenerConfig {
    /// How often to poll Telegram for new messages (seconds).  Default: 2.
    pub poll_interval_secs: u64,
    /// Maximum characters in a listener response (keeps Telegram replies short).  Default: 3000.
    pub max_response_chars: usize,
    /// Path to the conversation memory file.  `None` = use default path.
    pub memory_path: Option<String>,
    /// Maximum number of conversation turns to keep in memory.  Default: 100.
    pub max_memory_turns: usize,
    /// How often to poll GitHub for new issues (seconds). Default: 900 (15 min).
    pub github_poll_interval_secs: u64,
    /// Hour (0-23 local time) to send the daily morning brief. Default: 7.
    pub daily_brief_hour: u8,
    /// Path to store the set of already-acknowledged issue numbers (JSON). None = use default.
    pub acked_issues_path: Option<String>,
    /// Maximum commands per user per rate limit window. Default: 5.
    pub rate_limit_max: u32,
    /// Rate limit window size in seconds. Default: 60.
    pub rate_limit_window_secs: u64,
}

impl Default for ListenerConfig {
    fn default() -> Self {
        Self {
            poll_interval_secs: 2,
            max_response_chars: 3000,
            memory_path: None,
            max_memory_turns: 100,
            github_poll_interval_secs: 900,
            daily_brief_hour: 7,
            acked_issues_path: None,
            rate_limit_max: 5,
            rate_limit_window_secs: 60,
        }
    }
}

// ── AckedIssues ───────────────────────────────────────────────────────────────

/// Tracks which GitHub issue numbers have already been acknowledged by the listener.
///
/// Persists to a JSON file so acknowledgements survive restarts.
pub struct AckedIssues {
    /// The set of acknowledged issue numbers.
    pub issues: HashSet<u64>,
    /// Path to the backing JSON file.
    path: std::path::PathBuf,
}

impl AckedIssues {
    /// Load from file, or return an empty set if the file doesn't exist.
    pub fn load(path: &std::path::Path) -> Self {
        let issues = if path.exists() {
            std::fs::read_to_string(path)
                .ok()
                .and_then(|s| serde_json::from_str::<Vec<u64>>(&s).ok())
                .map(|v| v.into_iter().collect::<HashSet<u64>>())
                .unwrap_or_default()
        } else {
            HashSet::new()
        };
        Self { issues, path: path.to_path_buf() }
    }

    /// Returns the default path: `.axonix/acked_issues.json`
    pub fn default_path() -> std::path::PathBuf {
        std::path::PathBuf::from(".axonix/acked_issues.json")
    }

    /// Returns true if the issue number has already been acknowledged.
    pub fn contains(&self, issue_number: u64) -> bool {
        self.issues.contains(&issue_number)
    }

    /// Mark an issue as acknowledged.
    pub fn insert(&mut self, issue_number: u64) {
        self.issues.insert(issue_number);
    }

    /// Persist the current set to the JSON file.
    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(parent) = self.path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        let mut sorted: Vec<u64> = self.issues.iter().copied().collect();
        sorted.sort_unstable();
        let json = serde_json::to_string(&sorted)?;
        std::fs::write(&self.path, json)?;
        Ok(())
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Returns the current local hour (0-23). Used for daily brief scheduling.
/// Falls back to 0 on error.
pub(crate) fn local_hour() -> u8 {
    let output = std::process::Command::new("date").arg("+%H").output();
    match output {
        Ok(o) if o.status.success() => {
            let s = String::from_utf8_lossy(&o.stdout);
            s.trim().parse::<u8>().unwrap_or(0)
        }
        _ => 0,
    }
}

// ── Stats ─────────────────────────────────────────────────────────────────────

/// Runtime statistics for the listener daemon.
#[derive(Debug, Clone)]
pub struct ListenerStats {
    /// Total number of messages successfully handled.
    pub messages_handled: u64,
    /// Total number of errors encountered.
    pub errors: u64,
    /// Seconds since the listener started.
    pub uptime_secs: u64,
}

impl ListenerStats {
    /// Create a new zero-initialised stats block.
    pub fn new() -> Self {
        Self {
            messages_handled: 0,
            errors: 0,
            uptime_secs: 0,
        }
    }

    /// Format the stats as a compact human-readable string.
    ///
    /// Example: `"📊 Listener: 42 messages, 3h 20m uptime, 0 errors"`
    pub fn format(&self) -> String {
        let uptime = format_duration(self.uptime_secs);
        format!(
            "📊 Listener: {} messages, {} uptime, {} errors",
            self.messages_handled, uptime, self.errors
        )
    }
}

impl Default for ListenerStats {
    fn default() -> Self {
        Self::new()
    }
}

/// Format a duration in seconds as a human-readable string.
///
/// - `< 60s` → `"0m"`
/// - `< 1h`  → `"42m"`
/// - `>= 1h` → `"3h 20m"`
pub(crate) fn format_duration(secs: u64) -> String {
    let hours = secs / 3600;
    let minutes = (secs % 3600) / 60;
    if hours == 0 {
        format!("{}m", minutes)
    } else {
        format!("{}h {}m", hours, minutes)
    }
}
