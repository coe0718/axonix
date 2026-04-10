//! Formatting and display: format_for_prompt, format_for_system_prompt.

use super::types::{CycleSummary, CycleSummaryData};

impl CycleSummaryData {
    /// Format this summary as a system prompt injection block.
    ///
    /// Returns None if the summary is completely empty (no meaningful content).
    pub fn format_for_prompt(&self) -> Option<String> {
        if self.completed.is_empty()
            && self.pending.is_empty()
            && self.changed_files.is_empty()
            && self.learnings.is_empty()
        {
            return None;
        }

        let mut out = String::new();
        out.push_str("## Last Session\n");
        out.push_str(&format!("**{}** ({})\n", self.session, self.date));
        if let Some(count) = self.test_count {
            out.push_str(&format!("Tests passing: {count}\n"));
        }
        if !self.completed.is_empty() {
            out.push_str("\nCompleted:\n");
            for item in self.completed.iter().take(10) {
                out.push_str(&format!("- {item}\n"));
            }
        }
        if !self.changed_files.is_empty() {
            out.push_str("\nChanged files:\n");
            for f in self.changed_files.iter().take(15) {
                out.push_str(&format!("- {f}\n"));
            }
        }
        if !self.pending.is_empty() {
            out.push_str("\nPending / next session:\n");
            for item in self.pending.iter().take(10) {
                out.push_str(&format!("- {item}\n"));
            }
        }
        if !self.learnings.is_empty() {
            out.push_str("\nKey learnings:\n");
            for item in self.learnings.iter().take(5) {
                out.push_str(&format!("- {item}\n"));
            }
        }
        Some(out)
    }
}

impl CycleSummary {
    /// Format for injection into the system prompt.
    pub fn format_for_system_prompt(&self) -> Option<String> {
        self.data.as_ref()?.format_for_prompt()
    }
}
