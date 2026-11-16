use ferrum_domain::Verdict;
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CheckerError {
    #[error("checker execution failed")]
    Execution,
    #[error("checker timed out")]
    Timeout,
    #[error("checker produced invalid outcome code")]
    InvalidOutput,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckerOutcome {
    Accepted,
    WrongAnswer { reason: Option<String> },
}

pub trait Checker {
    fn check(
        &self,
        input: &[u8],
        actual: &[u8],
        expected: &[u8],
    ) -> Result<CheckerOutcome, CheckerError>;
}

pub struct CustomChecker<F>
where
    F: Fn(&[u8], &[u8], &[u8]) -> Result<CheckerOutcome, CheckerError>,
{
    evaluator: F,
}

impl<F> CustomChecker<F>
where
    F: Fn(&[u8], &[u8], &[u8]) -> Result<CheckerOutcome, CheckerError>,
{
    pub fn new(evaluator: F) -> Self {
        Self { evaluator }
    }
}

impl<F> Checker for CustomChecker<F>
where
    F: Fn(&[u8], &[u8], &[u8]) -> Result<CheckerOutcome, CheckerError>,
{
    fn check(
        &self,
        input: &[u8],
        actual: &[u8],
        expected: &[u8],
    ) -> Result<CheckerOutcome, CheckerError> {
        (self.evaluator)(input, actual, expected)
    }
}

pub fn evaluate_checker_result(
    result: Result<CheckerOutcome, CheckerError>,
) -> (Verdict, Option<String>) {
    match result {
        Ok(CheckerOutcome::Accepted) => (Verdict::Accepted, None),
        Ok(CheckerOutcome::WrongAnswer { reason }) => (Verdict::WrongAnswer, reason),
        Err(err) => (Verdict::InternalError, Some(err.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn custom_checker_evaluates_accepted() {
        let checker = CustomChecker::new(|_input, actual, expected| {
            if actual == expected {
                Ok(CheckerOutcome::Accepted)
            } else {
                Ok(CheckerOutcome::WrongAnswer {
                    reason: Some("different".into()),
                })
            }
        });
        let result = checker.check(b"", b"42", b"42");
        assert_eq!(evaluate_checker_result(result), (Verdict::Accepted, None));
    }

    #[test]
    fn custom_checker_reports_wrong_answer() {
        let checker = CustomChecker::new(|_input, _actual, _expected| {
            Ok(CheckerOutcome::WrongAnswer {
                reason: Some("floating point tolerance exceeded".into()),
            })
        });
        let result = checker.check(b"", b"3.14", b"3.14159");
        assert_eq!(
            evaluate_checker_result(result),
            (
                Verdict::WrongAnswer,
                Some("floating point tolerance exceeded".into())
            )
        );
    }

    #[test]
    fn custom_checker_failure_maps_to_internal_error() {
        let checker = CustomChecker::new(|_input, _actual, _expected| Err(CheckerError::Execution));
        let result = checker.check(b"", b"out", b"exp");
        assert_eq!(
            evaluate_checker_result(result),
            (
                Verdict::InternalError,
                Some("checker execution failed".into())
            )
        );
    }
}
