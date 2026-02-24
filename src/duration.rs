use std::time::Duration;

/// Parse a duration string with optional time unit suffixes.
/// Supported units: `ms` (milliseconds), `s` (seconds), `m` (minutes), `h` (hours).
/// Bare numbers without a unit are treated as seconds.
///
/// Supports:
/// - Single unit: `"30s"`, `"5m"`, `"1h"`, `"500ms"`, `"120"` (= 120 seconds)
/// - Decimal values: `"1.5m"`, `"2.7s"`, `"18.6h"`
/// - Combined units: `"1m30s"`, `"2s700ms"`, `"18h36m4s200ms"`
pub fn parse_duration(input: &str) -> Result<Duration, String> {
    let input = input.trim();
    if input.is_empty() {
        return Err("empty duration string".into());
    }

    // Try bare number first (no letters at all)
    if input.bytes().all(|b| b.is_ascii_digit() || b == b'.') {
        let n: f64 = input.parse().map_err(|_| {
            format!(
                "invalid duration '{}': expected a number with optional unit (ms, s, m, h)",
                input
            )
        })?;
        if n < 0.0 {
            return Err(format!("duration must not be negative: '{}'", input));
        }
        return Ok(Duration::from_secs_f64(n));
    }

    // Parse combined segments: sequences of {number}{unit}
    let mut total_secs: f64 = 0.0;
    let mut remaining = input;
    let mut found_any = false;

    while !remaining.is_empty() {
        // Find the end of the numeric part (digits and '.')
        let num_end = remaining
            .bytes()
            .position(|b| b.is_ascii_alphabetic())
            .ok_or_else(|| format!("invalid duration '{}': trailing number without unit", input))?;

        if num_end == 0 {
            return Err(format!(
                "invalid duration '{}': expected a number before unit",
                input
            ));
        }

        let num_str = &remaining[..num_end];
        let after_num = &remaining[num_end..];

        // Match unit
        let (multiplier, consumed) = if after_num.starts_with("ms") {
            (0.001, 2)
        } else if after_num.starts_with('h') {
            (3600.0, 1)
        } else if after_num.starts_with('m') {
            (60.0, 1)
        } else if after_num.starts_with('s') {
            (1.0, 1)
        } else {
            return Err(format!(
                "invalid duration '{}': unknown unit at '{}'",
                input, after_num
            ));
        };

        let n: f64 = num_str
            .parse()
            .map_err(|_| format!("invalid duration '{}': bad number '{}'", input, num_str))?;
        if n < 0.0 {
            return Err(format!("duration must not be negative: '{}'", input));
        }

        total_secs += n * multiplier;
        remaining = &after_num[consumed..];
        found_any = true;
    }

    if !found_any {
        return Err(format!(
            "invalid duration '{}': expected a number with optional unit (ms, s, m, h)",
            input
        ));
    }

    Ok(Duration::from_secs_f64(total_secs))
}

/// Format a Duration into a human-friendly combined string.
/// Uses the largest applicable units and combines them (e.g. `1m30s`, `2h15m`).
#[allow(dead_code)]
pub fn format_duration(d: Duration) -> String {
    let total_ms = d.as_millis();
    if total_ms == 0 {
        return "0s".into();
    }

    let total_secs = d.as_secs();
    let ms = d.subsec_millis();

    let h = total_secs / 3600;
    let m = (total_secs % 3600) / 60;
    let s = total_secs % 60;

    let mut parts = Vec::new();
    if h > 0 {
        parts.push(format!("{}h", h));
    }
    if m > 0 {
        parts.push(format!("{}m", m));
    }
    if s > 0 {
        parts.push(format!("{}s", s));
    }
    if ms > 0 {
        parts.push(format!("{}ms", ms));
    }

    if parts.is_empty() {
        "0s".into()
    } else {
        parts.join("")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_seconds_with_unit() {
        assert_eq!(parse_duration("30s").unwrap(), Duration::from_secs(30));
        assert_eq!(parse_duration("1s").unwrap(), Duration::from_secs(1));
        assert_eq!(parse_duration("0.5s").unwrap(), Duration::from_millis(500));
    }

    #[test]
    fn test_parse_milliseconds() {
        assert_eq!(parse_duration("500ms").unwrap(), Duration::from_millis(500));
        assert_eq!(
            parse_duration("1000ms").unwrap(),
            Duration::from_millis(1000)
        );
        assert_eq!(parse_duration("100ms").unwrap(), Duration::from_millis(100));
    }

    #[test]
    fn test_parse_minutes() {
        assert_eq!(parse_duration("5m").unwrap(), Duration::from_secs(300));
        assert_eq!(parse_duration("1m").unwrap(), Duration::from_secs(60));
    }

    #[test]
    fn test_parse_hours() {
        assert_eq!(parse_duration("1h").unwrap(), Duration::from_secs(3600));
        assert_eq!(parse_duration("2h").unwrap(), Duration::from_secs(7200));
    }

    #[test]
    fn test_parse_bare_number_defaults_to_seconds() {
        assert_eq!(parse_duration("30").unwrap(), Duration::from_secs(30));
        assert_eq!(parse_duration("300").unwrap(), Duration::from_secs(300));
        assert_eq!(parse_duration("0").unwrap(), Duration::from_secs(0));
    }

    #[test]
    fn test_parse_fractional() {
        assert_eq!(parse_duration("1.5s").unwrap(), Duration::from_millis(1500));
        assert_eq!(parse_duration("0.5m").unwrap(), Duration::from_secs(30));
        assert_eq!(parse_duration("2.5").unwrap(), Duration::from_millis(2500));
    }

    #[test]
    fn test_parse_decimal_units() {
        assert_eq!(parse_duration("1.5m").unwrap(), Duration::from_secs(90));
        assert_eq!(
            parse_duration("2.7s").unwrap(),
            Duration::from_secs_f64(2.7)
        );
        assert_eq!(
            parse_duration("18.6h").unwrap(),
            Duration::from_secs_f64(18.6 * 3600.0)
        );
        assert_eq!(
            parse_duration("0.5h").unwrap(),
            Duration::from_secs(30 * 60)
        );
    }

    #[test]
    fn test_parse_combined_units() {
        assert_eq!(parse_duration("1m30s").unwrap(), Duration::from_secs(90));
        assert_eq!(
            parse_duration("2s700ms").unwrap(),
            Duration::from_millis(2700)
        );
        assert_eq!(
            parse_duration("18h36m4s200ms").unwrap(),
            Duration::from_millis(18 * 3600_000 + 36 * 60_000 + 4_000 + 200)
        );
        assert_eq!(parse_duration("1h30m").unwrap(), Duration::from_secs(5400));
        assert_eq!(
            parse_duration("2h0m30s").unwrap(),
            Duration::from_secs(7230)
        );
    }

    #[test]
    fn test_parse_combined_with_decimals() {
        assert_eq!(
            parse_duration("1m30.5s").unwrap(),
            Duration::from_secs_f64(90.5)
        );
        assert_eq!(parse_duration("1h0.5m").unwrap(), Duration::from_secs(3630));
    }

    #[test]
    fn test_parse_whitespace_trimmed() {
        assert_eq!(parse_duration(" 30s ").unwrap(), Duration::from_secs(30));
    }

    #[test]
    fn test_parse_empty_error() {
        assert!(parse_duration("").is_err());
    }

    #[test]
    fn test_parse_invalid_errors() {
        assert!(parse_duration("abc").is_err());
        assert!(parse_duration("30x").is_err());
        assert!(parse_duration("--5s").is_err());
    }

    #[test]
    fn test_parse_negative_error() {
        assert!(parse_duration("-5s").is_err());
        assert!(parse_duration("-100ms").is_err());
        assert!(parse_duration("-1").is_err());
    }

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(Duration::from_secs(0)), "0s");
        assert_eq!(format_duration(Duration::from_millis(500)), "500ms");
        assert_eq!(format_duration(Duration::from_secs(1)), "1s");
        assert_eq!(format_duration(Duration::from_secs(30)), "30s");
        assert_eq!(format_duration(Duration::from_secs(60)), "1m");
        assert_eq!(format_duration(Duration::from_secs(300)), "5m");
        assert_eq!(format_duration(Duration::from_secs(3600)), "1h");
    }

    #[test]
    fn test_format_duration_combined() {
        assert_eq!(format_duration(Duration::from_secs(90)), "1m30s");
        assert_eq!(format_duration(Duration::from_secs(5400)), "1h30m");
        assert_eq!(format_duration(Duration::from_millis(2700)), "2s700ms");
        assert_eq!(format_duration(Duration::from_secs(3661)), "1h1m1s");
        assert_eq!(
            format_duration(Duration::from_millis(3661500)),
            "1h1m1s500ms"
        );
    }
}
