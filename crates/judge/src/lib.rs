mod checker;
mod comparison;
mod evaluator;
mod models;
mod scoring;
mod verdict;

pub use checker::{Checker, CheckerError, CheckerOutcome, CustomChecker, evaluate_checker_result};
pub use comparison::compare;
pub use evaluator::{CaseExecution, evaluate_case, evaluate_suite};
pub use ferrum_domain::{CaseFinding, ComparisonMode, TestCase};
pub use models::JudgingReport;
pub use scoring::{Score, calculate_score};
pub use verdict::aggregate_verdict;
