/// Rough cost estimate based on model pricing (USD).
/// Prices are approximate and may change — this is a convenience indicator, not a bill.
/// Accounts for Anthropic cache pricing:
///   - cache_read tokens are 10% of input price
///   - cache_write tokens are 25% more than input price
pub fn estimate_cost(model: &str, input_tokens: u64, output_tokens: u64, cache_read: u64, cache_write: u64) -> f64 {
    let (input_per_m, output_per_m) = if model.contains("opus") {
        (15.0, 75.0)
    } else if model.contains("sonnet") {
        (3.0, 15.0)
    } else if model.contains("haiku") {
        (0.25, 1.25)
    } else {
        // Unknown model — use sonnet pricing as default
        (3.0, 15.0)
    };
    let cache_read_per_m = input_per_m * 0.1;
    let cache_write_per_m = input_per_m * 1.25;
    (input_tokens as f64 / 1_000_000.0) * input_per_m
        + (output_tokens as f64 / 1_000_000.0) * output_per_m
        + (cache_read as f64 / 1_000_000.0) * cache_read_per_m
        + (cache_write as f64 / 1_000_000.0) * cache_write_per_m
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_estimate_cost_opus() {
        let cost = estimate_cost("claude-opus-4-6", 1_000_000, 1_000_000, 0, 0);
        assert!((cost - 90.0).abs() < 0.01, "Opus cost estimate wrong: {cost}");
    }

    #[test]
    fn test_estimate_cost_sonnet() {
        let cost = estimate_cost("claude-sonnet-4-20250514", 1_000_000, 0, 0, 0);
        assert!((cost - 3.0).abs() < 0.01, "Sonnet input cost wrong: {cost}");
    }

    #[test]
    fn test_estimate_cost_haiku() {
        let cost = estimate_cost("claude-haiku-3", 0, 1_000_000, 0, 0);
        assert!((cost - 1.25).abs() < 0.01, "Haiku output cost wrong: {cost}");
    }

    #[test]
    fn test_estimate_cost_unknown_model() {
        let cost = estimate_cost("some-unknown-model", 1_000_000, 1_000_000, 0, 0);
        assert!((cost - 18.0).abs() < 0.01, "Unknown model cost wrong: {cost}");
    }

    #[test]
    fn test_estimate_cost_zero_tokens() {
        let cost = estimate_cost("claude-opus-4-6", 0, 0, 0, 0);
        assert!((cost - 0.0).abs() < 0.001, "Zero tokens should cost $0: {cost}");
    }

    #[test]
    fn test_estimate_cost_with_cache_read() {
        let cost = estimate_cost("claude-opus-4-6", 0, 0, 1_000_000, 0);
        assert!((cost - 1.5).abs() < 0.01, "Cache read cost wrong: {cost}");
    }

    #[test]
    fn test_estimate_cost_with_cache_write() {
        let cost = estimate_cost("claude-opus-4-6", 0, 0, 0, 1_000_000);
        assert!((cost - 18.75).abs() < 0.01, "Cache write cost wrong: {cost}");
    }

    #[test]
    fn test_estimate_cost_full_breakdown() {
        let cost = estimate_cost("claude-sonnet-4-20250514", 1_000_000, 1_000_000, 1_000_000, 1_000_000);
        let expected = 3.0 + 15.0 + 0.3 + 3.75;
        assert!((cost - expected).abs() < 0.01, "Full breakdown cost wrong: {cost}, expected: {expected}");
    }
}
