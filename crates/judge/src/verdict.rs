use ferrum_domain::Verdict;

pub fn aggregate_verdict(case_verdicts: &[Verdict]) -> Verdict {
    if case_verdicts.is_empty() {
        return Verdict::Accepted;
    }

    if case_verdicts.contains(&Verdict::CompilationError) {
        return Verdict::CompilationError;
    }
    if case_verdicts.contains(&Verdict::InternalError) {
        return Verdict::InternalError;
    }

    for &verdict in case_verdicts {
        if verdict != Verdict::Accepted {
            return verdict;
        }
    }

    Verdict::Accepted
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_accepted_cases_yield_accepted_verdict() {
        assert_eq!(
            aggregate_verdict(&[Verdict::Accepted, Verdict::Accepted, Verdict::Accepted]),
            Verdict::Accepted
        );
        assert_eq!(aggregate_verdict(&[]), Verdict::Accepted);
    }

    #[test]
    fn compilation_error_takes_highest_precedence() {
        assert_eq!(
            aggregate_verdict(&[
                Verdict::Accepted,
                Verdict::WrongAnswer,
                Verdict::CompilationError,
                Verdict::TimeLimitExceeded,
            ]),
            Verdict::CompilationError
        );
    }

    #[test]
    fn internal_error_takes_precedence_over_test_failures() {
        assert_eq!(
            aggregate_verdict(&[
                Verdict::Accepted,
                Verdict::WrongAnswer,
                Verdict::InternalError,
            ]),
            Verdict::InternalError
        );
    }

    #[test]
    fn first_failing_case_determines_verdict_when_no_compilation_or_internal_errors() {
        assert_eq!(
            aggregate_verdict(&[
                Verdict::Accepted,
                Verdict::WrongAnswer,
                Verdict::TimeLimitExceeded,
            ]),
            Verdict::WrongAnswer
        );
        assert_eq!(
            aggregate_verdict(&[
                Verdict::Accepted,
                Verdict::TimeLimitExceeded,
                Verdict::WrongAnswer,
            ]),
            Verdict::TimeLimitExceeded
        );
    }
}
