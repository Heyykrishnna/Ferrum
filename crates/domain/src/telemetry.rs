use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::Verdict;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TraceContext {
    pub trace_id: String,
    pub span_id: String,
    pub sampled: bool,
}

impl TraceContext {
    pub fn new_root() -> Self {
        let trace_bytes = Uuid::new_v4().as_u128();
        let span_bytes = match (Uuid::new_v4().as_u128() & 0xffff_ffff_ffff_ffff) as u64 {
            0 => 1,
            val => val,
        };
        Self {
            trace_id: format!("{trace_bytes:032x}"),
            span_id: format!("{span_bytes:016x}"),
            sampled: true,
        }
    }

    pub fn parse_traceparent(header: &str) -> Option<Self> {
        let parts: Vec<&str> = header.trim().split('-').collect();
        if parts.len() != 4 {
            return None;
        }

        let version = parts[0];
        let trace_id = parts[1];
        let parent_id = parts[2];
        let flags = parts[3];

        if version != "00" {
            return None;
        }

        if trace_id.len() != 32
            || trace_id == "00000000000000000000000000000000"
            || !trace_id.chars().all(|c| c.is_ascii_hexdigit())
        {
            return None;
        }

        if parent_id.len() != 16
            || parent_id == "0000000000000000"
            || !parent_id.chars().all(|c| c.is_ascii_hexdigit())
        {
            return None;
        }

        if flags.len() != 2 || !flags.chars().all(|c| c.is_ascii_hexdigit()) {
            return None;
        }

        let flags_val = u8::from_str_radix(flags, 16).ok()?;
        let sampled = (flags_val & 0x01) == 1;

        Some(Self {
            trace_id: trace_id.to_ascii_lowercase(),
            span_id: parent_id.to_ascii_lowercase(),
            sampled,
        })
    }

    pub fn to_traceparent(&self) -> String {
        let flags = if self.sampled { "01" } else { "00" };
        format!("00-{}-{}-{flags}", self.trace_id, self.span_id)
    }

    pub fn child_span(&self) -> Self {
        let span_bytes = match (Uuid::new_v4().as_u128() & 0xffff_ffff_ffff_ffff) as u64 {
            0 => 1,
            val => val,
        };
        Self {
            trace_id: self.trace_id.clone(),
            span_id: format!("{span_bytes:016x}"),
            sampled: self.sampled,
        }
    }

    pub fn from_traceparent_or_generate(header: Option<&str>) -> Self {
        match header.and_then(Self::parse_traceparent) {
            Some(context) => context.child_span(),
            None => Self::new_root(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionStage {
    Intake,
    Outbox,
    Queue,
    Admission,
    SandboxInit,
    Compile,
    Execute,
    Persistence,
    Cleanup,
}

impl ExecutionStage {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Intake => "intake",
            Self::Outbox => "outbox",
            Self::Queue => "queue",
            Self::Admission => "admission",
            Self::SandboxInit => "sandbox_init",
            Self::Compile => "compile",
            Self::Execute => "execute",
            Self::Persistence => "persistence",
            Self::Cleanup => "cleanup",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct StageDuration {
    pub stage: ExecutionStage,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutcomeClass {
    Success,
    UserError,
    LimitExceeded,
    PlatformError,
}

impl OutcomeClass {
    pub fn from_verdict(verdict: Option<Verdict>) -> Self {
        match verdict {
            Some(Verdict::Accepted) => Self::Success,
            Some(Verdict::WrongAnswer) | Some(Verdict::CompilationError) | Some(Verdict::RuntimeError) => {
                Self::UserError
            }
            Some(Verdict::TimeLimitExceeded)
            | Some(Verdict::MemoryLimitExceeded)
            | Some(Verdict::OutputLimitExceeded) => Self::LimitExceeded,
            Some(Verdict::InternalError) | None => Self::PlatformError,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::UserError => "user_error",
            Self::LimitExceeded => "limit_exceeded",
            Self::PlatformError => "platform_error",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn traceparent_roundtrip_and_child_generation() {
        let root = TraceContext::new_root();
        let formatted = root.to_traceparent();
        let parsed = TraceContext::parse_traceparent(&formatted).expect("valid traceparent");

        assert_eq!(root.trace_id, parsed.trace_id);
        assert_eq!(root.span_id, parsed.span_id);
        assert!(parsed.sampled);

        let child = parsed.child_span();
        assert_eq!(child.trace_id, parsed.trace_id);
        assert_ne!(child.span_id, parsed.span_id);
    }

    #[test]
    fn invalid_traceparent_rejected() {
        assert!(TraceContext::parse_traceparent("invalid").is_none());
        assert!(TraceContext::parse_traceparent("01-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01").is_none());
        assert!(TraceContext::parse_traceparent("00-00000000000000000000000000000000-00f067aa0ba902b7-01").is_none());
        assert!(TraceContext::parse_traceparent("00-4bf92f3577b34da6a3ce929d0e0e4736-0000000000000000-01").is_none());
        assert!(TraceContext::parse_traceparent("00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-0g").is_none());
    }

    #[test]
    fn outcome_class_mapping() {
        assert_eq!(OutcomeClass::from_verdict(Some(Verdict::Accepted)), OutcomeClass::Success);
        assert_eq!(OutcomeClass::from_verdict(Some(Verdict::WrongAnswer)), OutcomeClass::UserError);
        assert_eq!(OutcomeClass::from_verdict(Some(Verdict::TimeLimitExceeded)), OutcomeClass::LimitExceeded);
        assert_eq!(OutcomeClass::from_verdict(Some(Verdict::InternalError)), OutcomeClass::PlatformError);
        assert_eq!(OutcomeClass::from_verdict(None), OutcomeClass::PlatformError);
    }
}
