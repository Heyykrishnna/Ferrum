use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::languages::{LanguageProfile, resolve_language, resolve_revision};

const MAX_INPUT_BYTES: usize = 64 * 1024;

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RequestedLimits {
    pub cpu_time_ms: Option<u32>,
    pub wall_time_ms: Option<u32>,
    pub memory_kib: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EffectiveLimits {
    pub cpu_time_ms: u32,
    pub wall_time_ms: u32,
    pub memory_kib: u32,
    pub stack_kib: u32,
    pub processes: u32,
    pub open_files: u32,
    pub file_bytes: u32,
    pub workspace_kib: u32,
    pub workspace_inodes: u32,
    pub stdout_bytes: u32,
    pub stderr_bytes: u32,
}

pub struct SubmissionInput {
    profile: &'static LanguageProfile,
    source: String,
    stdin: String,
    limits: EffectiveLimits,
}

#[derive(Debug, Error)]
pub enum ValidationError {
    #[error("unsupported resolved runtime profile")]
    Profile,
    #[error("unsupported language")]
    Language,
    #[error("source must contain 1 to 65536 UTF-8 bytes and no NUL characters")]
    Source,
    #[error("stdin must contain at most 65536 UTF-8 bytes and no NUL characters")]
    Stdin,
    #[error("limits exceed the selected language policy or wall time is below CPU time")]
    Limits,
}

impl SubmissionInput {
    pub fn new(
        language: &str,
        source: String,
        stdin: Option<String>,
        requested: RequestedLimits,
    ) -> Result<Self, ValidationError> {
        let profile = resolve_language(language).ok_or(ValidationError::Language)?;
        Self::with_profile(profile, source, stdin, requested)
    }

    fn with_profile(
        profile: &'static LanguageProfile,
        source: String,
        stdin: Option<String>,
        requested: RequestedLimits,
    ) -> Result<Self, ValidationError> {
        if source.is_empty() || source.len() > MAX_INPUT_BYTES || source.contains('\0') {
            return Err(ValidationError::Source);
        }
        let stdin = stdin.unwrap_or_default();
        if stdin.len() > MAX_INPUT_BYTES || stdin.contains('\0') {
            return Err(ValidationError::Stdin);
        }
        let ceilings = &profile.run_limits;
        let limits = EffectiveLimits {
            cpu_time_ms: requested.cpu_time_ms.unwrap_or(ceilings.cpu_time_ms),
            wall_time_ms: requested.wall_time_ms.unwrap_or(ceilings.wall_time_ms),
            memory_kib: requested.memory_kib.unwrap_or(ceilings.memory_kib),
            ..ceilings.clone()
        };
        if !(1..=ceilings.cpu_time_ms).contains(&limits.cpu_time_ms)
            || !(limits.cpu_time_ms..=ceilings.wall_time_ms).contains(&limits.wall_time_ms)
            || !(1..=ceilings.memory_kib).contains(&limits.memory_kib)
        {
            return Err(ValidationError::Limits);
        }

        Ok(Self {
            profile,
            source,
            stdin,
            limits,
        })
    }

    pub fn from_persisted(
        language: &str,
        revision: &str,
        source: String,
        stdin: String,
        limits: EffectiveLimits,
    ) -> Result<Self, ValidationError> {
        let profile = resolve_revision(revision).ok_or(ValidationError::Profile)?;
        if profile.language != language {
            return Err(ValidationError::Profile);
        }
        let input = Self::with_profile(
            profile,
            source,
            Some(stdin),
            RequestedLimits {
                cpu_time_ms: Some(limits.cpu_time_ms),
                wall_time_ms: Some(limits.wall_time_ms),
                memory_kib: Some(limits.memory_kib),
            },
        )?;
        if input.limits != limits {
            return Err(ValidationError::Limits);
        }
        Ok(input)
    }

    pub fn profile(&self) -> &'static LanguageProfile {
        self.profile
    }

    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn stdin(&self) -> &str {
        &self.stdin
    }

    pub fn limits(&self) -> &EffectiveLimits {
        &self.limits
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_profiles_rehydrate_only_their_own_language_and_limits() {
        for profile in crate::LANGUAGES {
            let input = SubmissionInput::new(
                profile.language,
                "source".into(),
                None,
                RequestedLimits::default(),
            )
            .unwrap();
            let reconstruct = |language, limits| {
                SubmissionInput::from_persisted(
                    language,
                    profile.revision,
                    "source".into(),
                    String::new(),
                    limits,
                )
            };
            assert!(reconstruct(profile.language, input.limits().clone()).is_ok());
            assert!(matches!(
                reconstruct("unrecognized", input.limits().clone()),
                Err(ValidationError::Profile)
            ));
            let mut tampered = input.limits().clone();
            tampered.memory_kib += 1;
            assert!(matches!(
                reconstruct(profile.language, tampered),
                Err(ValidationError::Limits)
            ));
        }
    }

    #[test]
    fn persisted_profile_and_policy_are_revalidated() {
        let input = SubmissionInput::new(
            "python",
            "print('hello')".into(),
            None,
            RequestedLimits::default(),
        )
        .unwrap();
        let mut persisted = input.limits().clone();
        assert!(
            SubmissionInput::from_persisted(
                "python",
                "python3.13-v1",
                input.source().into(),
                "".into(),
                persisted.clone()
            )
            .is_ok()
        );
        persisted.processes = 2;
        assert!(
            SubmissionInput::from_persisted(
                "python",
                "python3.13-v1",
                input.source().into(),
                "".into(),
                persisted
            )
            .is_err()
        );
        assert!(
            SubmissionInput::from_persisted(
                "python",
                "other-profile",
                input.source().into(),
                "".into(),
                input.limits().clone()
            )
            .is_err()
        );
    }
}
