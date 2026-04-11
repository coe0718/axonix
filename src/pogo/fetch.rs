//! HTTP fetching and data parsing from the ScrapedDuck API.
//!
//! Data source: https://raw.githubusercontent.com/bigfoott/ScrapedDuck/data/events.min.json
//! This JSON powers leekduck.com and is updated frequently.

use std::process::Command;
use super::types::{PogoData, PogoEvent, PromoCode};

pub(super) const SCRAPED_DUCK_URL: &str =
    "https://raw.githubusercontent.com/bigfoott/ScrapedDuck/data/events.min.json";

impl PogoData {
    /// Fetch and parse Pokémon GO events from ScrapedDuck. Synchronous via curl.
    /// Returns empty PogoData on any error (network, parse, etc.) — never panics.
    pub fn fetch() -> Self {
        match Self::try_fetch() {
            Ok(data) => data,
            Err(_) => PogoData::default(),
        }
    }

    pub(super) fn try_fetch() -> Result<Self, Box<dyn std::error::Error>> {
        // Use curl to avoid tokio blocking issues
        let output = Command::new("curl")
            .arg("--silent")
            .arg("--max-time")
            .arg("5")
            .arg(SCRAPED_DUCK_URL)
            .output()?;

        if !output.status.success() {
            return Err("curl failed".into());
        }

        let body = String::from_utf8(output.stdout)?;
        let json: serde_json::Value = serde_json::from_str(&body)?;

        let events = json.as_array().ok_or("expected array")?;

        // Get today's date string as "YYYY-MM-DD" for comparison
        let today = get_today_utc();
        let now_plus_7 = get_date_plus_days(7);

        let mut active_events: Vec<PogoEvent> = Vec::new();
        let mut upcoming_events: Vec<PogoEvent> = Vec::new();
        let mut promo_codes: Vec<PromoCode> = Vec::new();

        for event in events {
            let name = event["name"].as_str().unwrap_or("").to_string();
            let event_type = event["eventType"].as_str().unwrap_or("event").to_string();
            let start_str = event["start"].as_str().unwrap_or("");
            let end_str = event["end"].as_str().unwrap_or("");

            if name.is_empty() || start_str.is_empty() || end_str.is_empty() {
                continue;
            }

            // Extract date portion "YYYY-MM-DD" from "YYYY-MM-DDTHH:MM:SS.mmm"
            let start_date_key = &start_str[..start_str.len().min(10)];
            let end_date_key = &end_str[..end_str.len().min(10)];

            // Active: start <= today <= end (lexicographic comparison works for YYYY-MM-DD)
            let is_active = start_date_key <= today.as_str() && today.as_str() <= end_date_key;
            // Upcoming: today < start <= today+7
            let is_upcoming =
                today.as_str() < start_date_key && start_date_key <= now_plus_7.as_str();

            let end_display = format_date_display(end_date_key);
            let start_display = format_date_display(start_date_key);

            if is_active {
                active_events.push(PogoEvent {
                    name: name.clone(),
                    event_type: event_type.clone(),
                    end_date: end_display,
                    start_date: start_display,
                    is_active: true,
                });
            } else if is_upcoming {
                upcoming_events.push(PogoEvent {
                    name: name.clone(),
                    event_type: event_type.clone(),
                    end_date: end_display,
                    start_date: start_display,
                    is_active: false,
                });
            }

            // Collect promo codes from all events
            if let Some(extra) = event["extraData"].as_object() {
                if let Some(codes_val) = extra.get("promocodes") {
                    if let Some(codes_arr) = codes_val.as_array() {
                        let is_code_expired = end_date_key < today.as_str();
                        let expires_display = if is_code_expired {
                            format!("{} ⚠️ expired", format_date_display(end_date_key))
                        } else {
                            format_date_display(end_date_key)
                        };
                        for code_val in codes_arr {
                            if let Some(code) = code_val.as_str() {
                                if !code.is_empty() {
                                    promo_codes.push(PromoCode {
                                        code: code.to_string(),
                                        description: name.clone(),
                                        expires: expires_display.clone(),
                                        is_expired: is_code_expired,
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }

        // Sort active by end date (soonest ending first)
        active_events.sort_by(|a, b| a.end_date.cmp(&b.end_date));
        // Sort upcoming by start date
        upcoming_events.sort_by(|a, b| a.start_date.cmp(&b.start_date));

        // Limit
        active_events.truncate(10);
        upcoming_events.truncate(7);

        Ok(PogoData {
            active_events,
            upcoming_events,
            promo_codes,
        })
    }
}

/// Get today's date as "YYYY-MM-DD" using the system date command or SystemTime.
pub(super) fn get_today_utc() -> String {
    // Try using date command first
    if let Ok(output) = Command::new("date").arg("-u").arg("+%Y-%m-%d").output() {
        if output.status.success() {
            let s = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if s.len() == 10 {
                return s;
            }
        }
    }
    // Fallback: use SystemTime
    system_time_to_date_string(std::time::SystemTime::now())
}

/// Get date N days from today as "YYYY-MM-DD".
pub(super) fn get_date_plus_days(days: u64) -> String {
    let future = std::time::SystemTime::now()
        + std::time::Duration::from_secs(days * 24 * 3600);
    system_time_to_date_string(future)
}

/// Convert SystemTime to "YYYY-MM-DD" string (UTC).
pub(super) fn system_time_to_date_string(t: std::time::SystemTime) -> String {
    let secs = t
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    // Simple conversion: days since epoch -> date
    let days = secs / 86400;
    // Using the algorithm from https://howardhinnant.github.io/date_algorithms.html
    let z = days as i64 + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{:04}-{:02}-{:02}", y, m, d)
}

/// Format "YYYY-MM-DD" as "Mon D" (e.g. "Mar 24").
pub(super) fn format_date_display(date_key: &str) -> String {
    if date_key.len() < 10 {
        return date_key.to_string();
    }
    let month_num: u32 = date_key[5..7].parse().unwrap_or(0);
    let day: u32 = date_key[8..10].parse().unwrap_or(0);
    let month_name = match month_num {
        1 => "Jan",
        2 => "Feb",
        3 => "Mar",
        4 => "Apr",
        5 => "May",
        6 => "Jun",
        7 => "Jul",
        8 => "Aug",
        9 => "Sep",
        10 => "Oct",
        11 => "Nov",
        12 => "Dec",
        _ => "???",
    };
    format!("{} {}", month_name, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_date_display() {
        assert_eq!(format_date_display("2026-03-24"), "Mar 24");
        assert_eq!(format_date_display("2026-01-01"), "Jan 1");
        assert_eq!(format_date_display("2026-12-31"), "Dec 31");
    }

    #[test]
    fn test_system_time_to_date_string() {
        // 0 seconds since epoch = 1970-01-01
        let epoch = std::time::UNIX_EPOCH;
        let result = system_time_to_date_string(epoch);
        assert_eq!(result, "1970-01-01", "epoch should be 1970-01-01, got {result}");
    }
}
