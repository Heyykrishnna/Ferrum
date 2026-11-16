use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Score {
    pub earned: u32,
    pub total: u32,
}

impl Score {
    pub fn new(earned: u32, total: u32) -> Self {
        Self { earned, total }
    }

    pub fn percentage(&self) -> u32 {
        if self.total == 0 {
            0
        } else {
            ((u64::from(self.earned) * 100) / u64::from(self.total)) as u32
        }
    }
}

pub fn calculate_score(weights: &[u32], passed: &[bool]) -> Score {
    let total = weights.iter().sum();
    let earned = weights
        .iter()
        .zip(passed.iter())
        .filter_map(|(&weight, &is_passed)| if is_passed { Some(weight) } else { None })
        .sum();
    Score::new(earned, total)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calculates_full_score_when_all_pass() {
        let weights = [10, 20, 30];
        let passed = [true, true, true];
        let score = calculate_score(&weights, &passed);
        assert_eq!(score.earned, 60);
        assert_eq!(score.total, 60);
        assert_eq!(score.percentage(), 100);
    }

    #[test]
    fn calculates_partial_score_proportionally() {
        let weights = [10, 20, 30];
        let passed = [true, false, true];
        let score = calculate_score(&weights, &passed);
        assert_eq!(score.earned, 40);
        assert_eq!(score.total, 60);
        assert_eq!(score.percentage(), 66);
    }

    #[test]
    fn zero_score_when_none_pass() {
        let weights = [10, 20];
        let passed = [false, false];
        let score = calculate_score(&weights, &passed);
        assert_eq!(score.earned, 0);
        assert_eq!(score.total, 30);
        assert_eq!(score.percentage(), 0);
    }

    #[test]
    fn handles_empty_test_set() {
        let score = calculate_score(&[], &[]);
        assert_eq!(score.earned, 0);
        assert_eq!(score.total, 0);
        assert_eq!(score.percentage(), 0);
    }
}
