/// Rough cost estimate based on model pricing (USD).
/// Prices are approximate and may change — this is a convenience indicator, not a bill.
/// Accounts for Anthropic cache pricing:
///   - cache_read tokens are 10% of input price
///   - cache_write tokens are 25% more than input price
///
/// PRICES LAST VERIFIED: 2026-03-16
/// Source: https://www.anthropic.com/pricing
/// Verify after any major Anthropic pricing announcement.
///   claude-opus-*:   $15/$75 per 1M tokens (input/output)
///   claude-sonnet-*: $3/$15 per 1M tokens
///   claude-haiku-*:  $0.25/$1.25 per 1M tokens
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

    /// Cost must always be non-negative regardless of inputs.
    #[test]
    fn test_estimate_cost_never_negative() {
        let cost = estimate_cost("claude-opus-4-6", 0, 0, 0, 0);
        assert!(cost >= 0.0, "cost must be non-negative");
        let cost2 = estimate_cost("claude-haiku-3", 1, 1, 1, 1);
        assert!(cost2 >= 0.0, "cost must be non-negative with small tokens");
    }

    /// Model name matching is case-sensitive and substring-based.
    /// "claude-OPUS" does NOT contain "opus" (lowercase) — falls to unknown.
    #[test]
    fn test_estimate_cost_model_matching_is_case_sensitive() {
        let opus_cost = estimate_cost("claude-opus-4-6", 1_000_000, 0, 0, 0);
        let upper_cost = estimate_cost("claude-OPUS-4-6", 1_000_000, 0, 0, 0);
        // uppercase OPUS doesn't match "opus" — falls to sonnet default
        assert!(
            (opus_cost - 15.0).abs() < 0.01,
            "opus model should be $15/M input, got {opus_cost}"
        );
        assert!(
            (upper_cost - 3.0).abs() < 0.01,
            "OPUS (uppercase) should fall to default $3/M, got {upper_cost}"
        );
    }

    /// Cache read cost is 10% of input cost — verify the ratio.
    #[test]
    fn test_cache_read_is_10pct_of_input() {
        let input_cost = estimate_cost("claude-opus-4-6", 1_000_000, 0, 0, 0);
        let cache_cost = estimate_cost("claude-opus-4-6", 0, 0, 1_000_000, 0);
        let ratio = cache_cost / input_cost;
        assert!(
            (ratio - 0.1).abs() < 0.001,
            "cache read should be 10% of input; ratio = {ratio}"
        );
    }

    /// Cache write cost is 125% of input cost — verify the ratio.
    #[test]
    fn test_cache_write_is_125pct_of_input() {
        let input_cost = estimate_cost("claude-sonnet-4-20250514", 1_000_000, 0, 0, 0);
        let cache_write_cost = estimate_cost("claude-sonnet-4-20250514", 0, 0, 0, 1_000_000);
        let ratio = cache_write_cost / input_cost;
        assert!(
            (ratio - 1.25).abs() < 0.001,
            "cache write should be 125% of input; ratio = {ratio}"
        );
    }

    /// Output cost should always be higher than input cost per token for all known models.
    #[test]
    fn test_output_more_expensive_than_input() {
        for model in &["claude-opus-4-6", "claude-sonnet-4-20250514", "claude-haiku-3"] {
            let input = estimate_cost(model, 1_000_000, 0, 0, 0);
            let output = estimate_cost(model, 0, 1_000_000, 0, 0);
            assert!(
                output > input,
                "output ({output}) should cost more than input ({input}) for model {model}"
            );
        }
    }

    /// Small token counts should return very small but non-negative costs.
    #[test]
    fn test_estimate_cost_small_token_counts() {
        let cost = estimate_cost("claude-sonnet-4-20250514", 100, 50, 0, 0);
        // 100 input @ $3/M = $0.0003, 50 output @ $15/M = $0.00075
        let expected = 0.0003 + 0.00075;
        assert!(
            (cost - expected).abs() < 1e-7,
            "small token cost wrong: {cost}, expected ~{expected}"
        );
    }

    /// Haiku should be cheaper than sonnet which should be cheaper than opus.
    #[test]
    fn test_model_cost_ordering() {
        let haiku = estimate_cost("claude-haiku-3", 1_000_000, 1_000_000, 0, 0);
        let sonnet = estimate_cost("claude-sonnet-4-20250514", 1_000_000, 1_000_000, 0, 0);
        let opus = estimate_cost("claude-opus-4-6", 1_000_000, 1_000_000, 0, 0);
        assert!(haiku < sonnet, "haiku ({haiku}) should be cheaper than sonnet ({sonnet})");
        assert!(sonnet < opus, "sonnet ({sonnet}) should be cheaper than opus ({opus})");
    }

}
