//! Type definitions and formatting for Pokémon GO events and promo codes.

/// A Pokémon GO event (active or upcoming).
#[derive(Debug, Clone)]
pub struct PogoEvent {
    pub name: String,
    pub event_type: String,
    pub end_date: String,   // "Mar 24" format for display
    pub start_date: String, // "Mar 24" format, for upcoming
    pub is_active: bool,    // currently running
}

/// A Pokémon GO promo code from an event.
#[derive(Debug, Clone)]
pub struct PromoCode {
    pub code: String,
    pub description: String, // event name it came from
    pub expires: String,     // "Mar 2" or "expired" if past
    pub is_expired: bool,
}

/// Aggregated Pokémon GO data (events + promo codes).
#[derive(Debug, Default)]
pub struct PogoData {
    pub active_events: Vec<PogoEvent>,   // happening right now
    pub upcoming_events: Vec<PogoEvent>, // starting within 7 days
    pub promo_codes: Vec<PromoCode>,     // all known codes (include expired for reference)
}

impl PogoData {
    /// Format for terminal output (multi-line string).
    pub fn format_terminal(&self) -> String {
        if self.is_empty() {
            return "🎮 POKÉMON GO (leekduck.com)\n   (unavailable — check leekduck.com)\n"
                .to_string();
        }

        let mut out = String::new();
        out.push_str("🎮 POKÉMON GO (leekduck.com)\n");

        if !self.active_events.is_empty() {
            out.push_str(&format!(
                "   ACTIVE EVENTS ({}):\n",
                self.active_events.len()
            ));
            for ev in &self.active_events {
                out.push_str(&format!("   • {} [ends {}]\n", ev.name, ev.end_date));
            }
        }

        if !self.upcoming_events.is_empty() {
            out.push_str(&format!(
                "   UPCOMING (next 7 days, {}):\n",
                self.upcoming_events.len()
            ));
            for ev in &self.upcoming_events {
                out.push_str(&format!("   • {} [{}]\n", ev.name, ev.start_date));
            }
        }

        if !self.promo_codes.is_empty() {
            out.push_str("   PROMO CODES:\n");
            for pc in &self.promo_codes {
                out.push_str(&format!(
                    "   • {} — {} [exp {}]\n",
                    pc.code, pc.description, pc.expires
                ));
            }
        }

        out
    }

    /// Format for Telegram (compact, markdown-safe).
    pub fn format_telegram(&self) -> String {
        if self.is_empty() {
            return "*Pokémon GO* (leekduck.com)\n(unavailable — check leekduck.com)\n"
                .to_string();
        }

        let mut out = String::new();
        out.push_str("🎮 *Pokémon GO*\n");

        if !self.active_events.is_empty() {
            out.push_str("_Active:_\n");
            for ev in self.active_events.iter().take(4) {
                out.push_str(&format!("• {} →{}\n", ev.name, ev.end_date));
            }
            if self.active_events.len() > 4 {
                out.push_str(&format!("  (+{} more)\n", self.active_events.len() - 4));
            }
        }

        if !self.upcoming_events.is_empty() {
            out.push_str("_Upcoming:_\n");
            for ev in self.upcoming_events.iter().take(3) {
                out.push_str(&format!("• {} {}\n", ev.name, ev.start_date));
            }
        }

        let valid_codes: Vec<&PromoCode> = self
            .promo_codes
            .iter()
            .filter(|pc| !pc.is_expired)
            .collect();
        if !valid_codes.is_empty() {
            out.push_str("_Codes:_\n");
            for pc in &valid_codes {
                out.push_str(&format!("`{}` exp {}\n", pc.code, pc.expires));
            }
        }

        out
    }

    /// True if there is anything to show (events or codes).
    pub fn is_empty(&self) -> bool {
        self.active_events.is_empty()
            && self.upcoming_events.is_empty()
            && self.promo_codes.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_active_event(name: &str) -> PogoEvent {
        PogoEvent {
            name: name.to_string(),
            event_type: "event".to_string(),
            end_date: "Mar 30".to_string(),
            start_date: "Mar 20".to_string(),
            is_active: true,
        }
    }

    fn make_upcoming_event(name: &str) -> PogoEvent {
        PogoEvent {
            name: name.to_string(),
            event_type: "event".to_string(),
            end_date: "Apr 5".to_string(),
            start_date: "Apr 1".to_string(),
            is_active: false,
        }
    }

    fn make_promo_code(code: &str, desc: &str, expired: bool) -> PromoCode {
        PromoCode {
            code: code.to_string(),
            description: desc.to_string(),
            expires: if expired {
                "Mar 2 ⚠️ expired".to_string()
            } else {
                "Mar 30".to_string()
            },
            is_expired: expired,
        }
    }

    #[test]
    fn test_pogo_data_default_is_empty() {
        let data = PogoData::default();
        assert!(data.active_events.is_empty());
        assert!(data.upcoming_events.is_empty());
        assert!(data.promo_codes.is_empty());
    }

    #[test]
    fn test_format_terminal_empty() {
        let data = PogoData::default();
        let out = data.format_terminal();
        assert!(
            out.contains("unavailable"),
            "empty data should mention unavailable: {out}"
        );
        assert!(out.contains("leekduck.com"), "should mention leekduck.com: {out}");
    }

    #[test]
    fn test_format_terminal_with_active_event() {
        let data = PogoData {
            active_events: vec![make_active_event("Bug Out 2026")],
            upcoming_events: vec![],
            promo_codes: vec![],
        };
        let out = data.format_terminal();
        assert!(out.contains("Bug Out 2026"), "should contain event name: {out}");
        assert!(out.contains("ACTIVE"), "should contain ACTIVE: {out}");
        assert!(out.contains("ends"), "should mention end date: {out}");
    }

    #[test]
    fn test_format_terminal_with_upcoming() {
        let data = PogoData {
            active_events: vec![],
            upcoming_events: vec![make_upcoming_event("Regieleki Raid Hour")],
            promo_codes: vec![],
        };
        let out = data.format_terminal();
        assert!(
            out.contains("Regieleki Raid Hour"),
            "should contain upcoming event: {out}"
        );
        assert!(out.contains("UPCOMING"), "should contain UPCOMING section: {out}");
    }

    #[test]
    fn test_format_terminal_with_promo_code() {
        let data = PogoData {
            active_events: vec![],
            upcoming_events: vec![],
            promo_codes: vec![make_promo_code("TESTCODE123", "Some Event", false)],
        };
        let out = data.format_terminal();
        assert!(out.contains("TESTCODE123"), "should contain promo code: {out}");
        assert!(out.contains("PROMO CODES"), "should contain PROMO CODES section: {out}");
        assert!(out.contains("Some Event"), "should contain event description: {out}");
    }

    #[test]
    fn test_format_telegram_empty() {
        let data = PogoData::default();
        let out = data.format_telegram();
        assert!(out.contains("unavailable"), "empty data should mention unavailable: {out}");
        assert!(out.contains("Pokémon GO"), "should mention Pokémon GO: {out}");
    }

    #[test]
    fn test_format_telegram_with_event() {
        let data = PogoData {
            active_events: vec![make_active_event("Bug Out 2026")],
            upcoming_events: vec![],
            promo_codes: vec![],
        };
        let out = data.format_telegram();
        assert!(out.contains("Bug Out 2026"), "should contain event name: {out}");
        assert!(out.contains("Active:"), "should contain Active: label: {out}");
        assert!(out.contains("*Pokémon GO*"), "should use bold markdown: {out}");
    }

    #[test]
    fn test_format_telegram_with_promo_code() {
        let data = PogoData {
            active_events: vec![],
            upcoming_events: vec![],
            promo_codes: vec![make_promo_code("FAIRYCODE", "Fairy Event", false)],
        };
        let out = data.format_telegram();
        assert!(out.contains("`FAIRYCODE`"), "code should be backtick-formatted: {out}");
        assert!(out.contains("Codes:"), "should contain Codes: label: {out}");
    }

    #[test]
    fn test_is_empty_true() {
        let data = PogoData::default();
        assert!(data.is_empty(), "default PogoData should be empty");
    }

    #[test]
    fn test_is_empty_false_active() {
        let data = PogoData {
            active_events: vec![make_active_event("Some Event")],
            upcoming_events: vec![],
            promo_codes: vec![],
        };
        assert!(!data.is_empty(), "PogoData with active event should not be empty");
    }

    #[test]
    fn test_promo_code_expired_flag() {
        let data = PogoData {
            active_events: vec![],
            upcoming_events: vec![],
            promo_codes: vec![make_promo_code("TH4NKY0UF41RYMUCH", "GO Tour", true)],
        };
        let out = data.format_terminal();
        assert!(out.contains("TH4NKY0UF41RYMUCH"), "should show code: {out}");
        assert!(out.contains("expired"), "should show expired marker: {out}");
    }

    #[test]
    fn test_active_events_limited() {
        let events: Vec<PogoEvent> = (0..15).map(|i| make_active_event(&format!("Event {i}"))).collect();
        // Simulate the truncation that fetch() applies
        let mut data = PogoData {
            active_events: events,
            upcoming_events: vec![],
            promo_codes: vec![],
        };
        data.active_events.truncate(10);
        let out = data.format_terminal();
        // Only 10 events should appear
        let count = out.matches("Event ").count();
        assert_eq!(count, 10, "should only show 10 active events, got {count}: {out}");
    }
}
