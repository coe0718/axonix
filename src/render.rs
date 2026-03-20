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

    #[test]
    fn test_truncate_zero_max_returns_ellipsis() {
        // max=0 means nth(0) returns the first char index → truncate to empty + ellipsis
        let result = truncate("hello", 0);
        assert!(result.ends_with('…'), "max=0 should produce ellipsis: {result}");
    }

    #[test]
    fn test_truncate_single_char_fits() {
        assert_eq!(truncate("a", 1), "a");
    }

    #[test]
    fn test_truncate_single_char_over() {
        // max=0 truncates the single char
        let result = truncate("a", 0);
        assert_eq!(result, "…");
    }

    #[test]
    fn test_truncate_multibyte_boundary_safe() {
        // "日本語" has 3 chars, each 3 bytes — truncate at 2 must not slice mid-char
        let s = "日本語";
        let result = truncate(s, 2);
        assert!(result.ends_with('…'), "should end with ellipsis");
        // The slice must be valid UTF-8 (this would panic otherwise)
        let _slice: &str = &result;
    }

    #[test]
    fn test_truncate_exactly_fits_unicode() {
        let s = "日本語"; // 3 chars
        assert_eq!(truncate(s, 3), "日本語", "exact fit should return unchanged");
    }

    #[test]
    fn test_truncate_long_with_newlines() {
        let s = "line1\nline2\nline3";
        let result = truncate(s, 5);
        assert_eq!(result, "line1…");
    }

    #[test]
    fn test_truncate_preserves_whole_string_when_short() {
        let s = "short";
        assert_eq!(truncate(s, 100), "short");
    }

    #[test]
    fn test_ansi_constants_start_with_escape() {
        // All ANSI constants should start with ESC byte (\x1b)
        assert!(RESET.starts_with('\x1b'));
        assert!(BOLD.starts_with('\x1b'));
        assert!(DIM.starts_with('\x1b'));
        assert!(GREEN.starts_with('\x1b'));
        assert!(YELLOW.starts_with('\x1b'));
        assert!(CYAN.starts_with('\x1b'));
        assert!(MAGENTA.starts_with('\x1b'));
        assert!(RED.starts_with('\x1b'));
    }

    #[test]
    fn test_ansi_reset_distinct_from_others() {
        // RESET must be different from color codes — mixing them up breaks rendering
        assert_ne!(RESET, GREEN);
        assert_ne!(RESET, RED);
        assert_ne!(RESET, YELLOW);
        assert_ne!(RESET, BOLD);
    }

    #[test]
    fn test_truncate_result_char_count_at_most_max_plus_ellipsis() {
        let s = "abcdefghijklmnop";
        let result = truncate(s, 5);
        // The result should have exactly 5 chars + 1 for the ellipsis = 6 chars
        let char_count = result.chars().count();
        assert_eq!(char_count, 6, "truncated to 5 should give 6 chars (5 + ellipsis): got {char_count}");
    }

    #[test]
    fn test_print_usage_no_panic_with_zero_usage() {
        use yoagent::Usage;
        // print_usage should not panic when tokens are 0 (it just prints nothing)
        let usage = Usage::default();
        let elapsed = std::time::Duration::from_secs(5);
        // We can't easily capture stdout, but at minimum it must not panic
        print_usage(&usage, elapsed);
    }

    #[test]
    fn test_print_usage_no_panic_with_real_usage() {
        use yoagent::Usage;
        let mut usage = Usage::default();
        usage.input = 1000;
        usage.output = 500;
        usage.cache_read = 200;
        usage.cache_write = 100;
        let elapsed = std::time::Duration::from_secs(90); // 1m30s
        print_usage(&usage, elapsed);
    }

    #[test]
    fn test_truncate_only_ellipsis_at_boundary() {
        // Ensure only ONE ellipsis is added, not multiple
        let s = "hello world this is a long string";
        let result = truncate(s, 10);
        let ellipsis_count = result.chars().filter(|&c| c == '…').count();
        assert_eq!(ellipsis_count, 1, "should have exactly one ellipsis, got: {ellipsis_count}");
    }
}
