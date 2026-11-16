use ferrum_domain::{CaseFinding, Verdict};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JudgingReport {
    pub verdict: Verdict,
    pub score: u32,
    pub max_score: u32,
    pub cases: Vec<CaseFinding>,
}

#[cfg(test)]
mod tests {
    use ferrum_domain::{CaseContent, CaseTelemetry};

    use super::*;

    #[test]
    fn hidden_case_finding_masks_sensitive_data() {
        let finding = CaseFinding::hidden(
            1,
            Verdict::Accepted,
            10,
            10,
            CaseTelemetry {
                exit_code: Some(0),
                signal: None,
                cpu_time_ms: Some(10),
                wall_time_ms: Some(20),
                peak_memory_kib: Some(1024),
            },
        );
        let serialized = serde_json::to_value(&finding).unwrap();
        assert!(serialized.get("input").is_none());
        assert!(serialized.get("expected_output").is_none());
        assert!(serialized.get("stdout").is_none());
        assert!(serialized.get("stderr").is_none());
        assert_eq!(serialized["score"], 10);
        assert_eq!(serialized["verdict"], "accepted");
    }

    #[test]
    fn public_case_finding_retains_inputs_and_outputs() {
        let finding = CaseFinding::public(
            1,
            Verdict::Accepted,
            10,
            10,
            CaseContent {
                input: "in\n".into(),
                expected_output: "out\n".into(),
                stdout: "out\n".into(),
                stderr: "".into(),
            },
            CaseTelemetry {
                exit_code: Some(0),
                signal: None,
                cpu_time_ms: Some(10),
                wall_time_ms: Some(20),
                peak_memory_kib: Some(1024),
            },
        );
        let serialized = serde_json::to_value(&finding).unwrap();
        assert_eq!(serialized["input"], "in\n");
        assert_eq!(serialized["expected_output"], "out\n");
        assert_eq!(serialized["stdout"], "out\n");
        assert_eq!(serialized["stderr"], "");
    }
}
