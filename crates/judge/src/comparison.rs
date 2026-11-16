use ferrum_domain::ComparisonMode;

pub fn compare(actual: &[u8], expected: &[u8], mode: ComparisonMode) -> bool {
    match mode {
        ComparisonMode::Exact => actual == expected,
        ComparisonMode::Whitespace => compare_whitespace(actual, expected),
        ComparisonMode::Tokens => compare_tokens(actual, expected),
    }
}

fn compare_whitespace(actual: &[u8], expected: &[u8]) -> bool {
    let actual_str = String::from_utf8_lossy(actual);
    let expected_str = String::from_utf8_lossy(expected);

    normalize_whitespace_lines(&actual_str) == normalize_whitespace_lines(&expected_str)
}

fn normalize_whitespace_lines(s: &str) -> Vec<&str> {
    let mut lines: Vec<&str> = s.lines().map(|l| l.trim_end()).collect();
    while let Some(last) = lines.last() {
        if last.is_empty() {
            lines.pop();
        } else {
            break;
        }
    }
    lines
}

fn compare_tokens(actual: &[u8], expected: &[u8]) -> bool {
    let actual_str = String::from_utf8_lossy(actual);
    let expected_str = String::from_utf8_lossy(expected);

    actual_str
        .split_whitespace()
        .eq(expected_str.split_whitespace())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_comparison_requires_identical_bytes() {
        assert!(compare(b"42\n", b"42\n", ComparisonMode::Exact));
        assert!(!compare(b"42\r\n", b"42\n", ComparisonMode::Exact));
        assert!(!compare(b"42 \n", b"42\n", ComparisonMode::Exact));
        assert!(!compare(b"42", b"42\n", ComparisonMode::Exact));
        assert!(!compare(b"43\n", b"42\n", ComparisonMode::Exact));
    }

    #[test]
    fn whitespace_mode_normalizes_line_endings() {
        assert!(compare(
            b"line 1\r\nline 2\r\n",
            b"line 1\nline 2\n",
            ComparisonMode::Whitespace
        ));
    }

    #[test]
    fn whitespace_mode_normalizes_trailing_spaces() {
        assert!(compare(
            b"42   \n100\t  \n",
            b"42\n100\n",
            ComparisonMode::Whitespace
        ));
    }

    #[test]
    fn whitespace_mode_normalizes_final_newlines() {
        assert!(compare(
            b"result\n\n\n",
            b"result",
            ComparisonMode::Whitespace
        ));
        assert!(compare(b"result\n", b"result", ComparisonMode::Whitespace));
        assert!(compare(b"result", b"result\n", ComparisonMode::Whitespace));
    }

    #[test]
    fn whitespace_mode_preserves_internal_space_differences() {
        assert!(!compare(
            b"hello world\n",
            b"hello  world\n",
            ComparisonMode::Whitespace
        ));
    }

    #[test]
    fn tokens_mode_normalizes_all_whitespace() {
        assert!(compare(b"1   2\n3\n", b"1 2 3", ComparisonMode::Tokens));
        assert!(!compare(b"1 2 3", b"1 2 4", ComparisonMode::Tokens));
    }

    #[test]
    fn mode_serialization_and_parsing() {
        assert_eq!(ComparisonMode::parse("exact"), Some(ComparisonMode::Exact));
        assert_eq!(
            ComparisonMode::parse("whitespace"),
            Some(ComparisonMode::Whitespace)
        );
        assert_eq!(
            ComparisonMode::parse("tokens"),
            Some(ComparisonMode::Tokens)
        );
        assert_eq!(ComparisonMode::parse("invalid"), None);
        assert_eq!(ComparisonMode::Exact.as_str(), "exact");
        assert_eq!(ComparisonMode::Whitespace.as_str(), "whitespace");
        assert_eq!(ComparisonMode::Tokens.as_str(), "tokens");
    }
}
