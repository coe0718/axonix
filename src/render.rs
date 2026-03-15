use yoagent::Usage;

// ANSI color helpers
pub const RESET: &str = "\x1b[0m";
pub const BOLD: &str = "\x1b[1m";
pub const DIM: &str = "\x1b[2m";
pub const GREEN: &str = "\x1b[32m";
pub const YELLOW: &str = "\x1b[33m";
pub const CYAN: &str = "\x1b[36m";
pub const MAGENTA: &str = "\x1b[35m";
pub const RED: &str = "\x1b[31m";

pub fn truncate(s: &str, max: usize) -> String {
    match s.char_indices().nth(max) {
        Some((idx, _)) => format!("{}…", &s[..idx]),
        None => s.to_string(),
    }
}

pub fn print_usage(usage: &Usage, elapsed: std::time::Duration) {
    if usage.input > 0 || usage.output > 0 {
        let cache_info = if usage.cache_read > 0 || usage.cache_write > 0 {
            format!(
                " (cache: {} read, {} write)",
                usage.cache_read, usage.cache_write
            )
        } else {
            String::new()
        };
        let secs = elapsed.as_secs_f64();
        let time_str = if secs < 60.0 {
            format!("{secs:.1}s")
        } else {
            let mins = secs as u64 / 60;
            let remaining = secs as u64 % 60;
            format!("{mins}m {remaining}s")
        };
        println!(
            "\n{DIM}  tokens: {} in / {} out{cache_info} — {time_str}{RESET}",
            usage.input, usage.output
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate_short_string() {
        assert_eq!(truncate("hello", 10), "hello");
    }

    #[test]
    fn test_truncate_exact_length() {
        assert_eq!(truncate("hello", 5), "hello");
    }

    #[test]
    fn test_truncate_long_string() {
        assert_eq!(truncate("hello world", 5), "hello…");
    }

    #[test]
    fn test_truncate_unicode() {
        assert_eq!(truncate("héllo wörld", 5), "héllo…");
    }

    #[test]
    fn test_truncate_empty() {
        assert_eq!(truncate("", 5), "");
    }

    #[test]
    fn test_truncate_adds_ellipsis() {
        let result = truncate("a]long string that goes on", 6);
        assert!(result.ends_with('…'), "Truncated string should end with ellipsis: {result}");
    }

    #[test]
    fn test_ansi_constants_not_empty() {
        assert!(!RESET.is_empty());
        assert!(!BOLD.is_empty());
        assert!(!DIM.is_empty());
        assert!(!GREEN.is_empty());
        assert!(!YELLOW.is_empty());
        assert!(!CYAN.is_empty());
        assert!(!MAGENTA.is_empty());
        assert!(!RED.is_empty());
    }
}
