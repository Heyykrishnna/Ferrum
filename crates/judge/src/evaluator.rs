use ferrum_domain::{CaseContent, CaseFinding, CaseTelemetry, TestCase, Verdict};

use crate::{comparison::compare, models::JudgingReport, verdict::aggregate_verdict};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaseExecution {
    pub success: bool,
    pub failure_verdict: Option<Verdict>,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub exit_code: Option<u8>,
    pub signal: Option<u8>,
    pub cpu_time_ms: Option<u64>,
    pub wall_time_ms: Option<u64>,
    pub peak_memory_kib: Option<u64>,
}

pub fn evaluate_case(case: &TestCase, execution: &CaseExecution) -> CaseFinding {
    let (verdict, score) = if execution.success {
        if compare(
            &execution.stdout,
            case.expected_output.as_bytes(),
            case.comparison_mode,
        ) {
            (Verdict::Accepted, case.weight)
        } else {
            (Verdict::WrongAnswer, 0)
        }
    } else {
        (
            execution.failure_verdict.unwrap_or(Verdict::RuntimeError),
            0,
        )
    };

    let telemetry = CaseTelemetry {
        exit_code: execution.exit_code,
        signal: execution.signal,
        cpu_time_ms: execution.cpu_time_ms,
        wall_time_ms: execution.wall_time_ms,
        peak_memory_kib: execution.peak_memory_kib,
    };

    if case.hidden {
        CaseFinding::hidden(case.index, verdict, score, case.weight, telemetry)
    } else {
        CaseFinding::public(
            case.index,
            verdict,
            score,
            case.weight,
            CaseContent {
                input: case.input.clone(),
                expected_output: case.expected_output.clone(),
                stdout: String::from_utf8_lossy(&execution.stdout).into_owned(),
                stderr: String::from_utf8_lossy(&execution.stderr).into_owned(),
            },
            telemetry,
        )
    }
}

pub fn evaluate_suite(cases: &[TestCase], executions: &[CaseExecution]) -> JudgingReport {
    let findings: Vec<CaseFinding> = cases
        .iter()
        .zip(executions.iter())
        .map(|(case, exec)| evaluate_case(case, exec))
        .collect();

    let verdicts: Vec<Verdict> = findings.iter().map(|f| f.verdict).collect();
    let overall_verdict = aggregate_verdict(&verdicts);
    let score = findings.iter().map(|f| f.score).sum();
    let max_score = cases.iter().map(|c| c.weight).sum();

    JudgingReport {
        verdict: overall_verdict,
        score,
        max_score,
        cases: findings,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferrum_domain::ComparisonMode;

    #[test]
    fn suite_evaluation_computes_partial_score_and_aggregate_verdict() {
        let cases = vec![
            TestCase {
                index: 1,
                input: "1\n".into(),
                expected_output: "2\n".into(),
                weight: 50,
                hidden: false,
                comparison_mode: ComparisonMode::Exact,
            },
            TestCase {
                index: 2,
                input: "2\n".into(),
                expected_output: "4\n".into(),
                weight: 50,
                hidden: true,
                comparison_mode: ComparisonMode::Exact,
            },
        ];
        let executions = vec![
            CaseExecution {
                success: true,
                failure_verdict: None,
                stdout: b"2\n".to_vec(),
                stderr: Vec::new(),
                exit_code: Some(0),
                signal: None,
                cpu_time_ms: Some(10),
                wall_time_ms: Some(20),
                peak_memory_kib: Some(1024),
            },
            CaseExecution {
                success: true,
                failure_verdict: None,
                stdout: b"wrong\n".to_vec(),
                stderr: Vec::new(),
                exit_code: Some(0),
                signal: None,
                cpu_time_ms: Some(12),
                wall_time_ms: Some(22),
                peak_memory_kib: Some(1024),
            },
        ];

        let report = evaluate_suite(&cases, &executions);
        assert_eq!(report.verdict, Verdict::WrongAnswer);
        assert_eq!(report.score, 50);
        assert_eq!(report.max_score, 100);
        assert_eq!(report.cases.len(), 2);
        assert_eq!(report.cases[0].verdict, Verdict::Accepted);
        assert_eq!(report.cases[0].stdout.as_deref(), Some("2\n"));
        assert_eq!(report.cases[1].verdict, Verdict::WrongAnswer);
        assert_eq!(report.cases[1].stdout, None);
    }
}
