use std::future::Future;

use ferrum_domain::{Submission, SubmissionInput};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum SubmissionError {
    #[error("invalid idempotency key")]
    InvalidIdempotencyKey,
    #[error("this idempotency key was already used with a different request")]
    IdempotencyConflict,
    #[error("submission storage is unavailable")]
    Unavailable,
    #[error("could not store or read the submission")]
    Storage,
}

#[derive(Debug, Error)]
pub enum ExecutionError {
    #[error("execution storage is unavailable")]
    Unavailable,
    #[error("could not read or update execution attempt")]
    Storage,
    #[error("submission not found")]
    NotFound,
    #[error("attempt lease expired or ownership lost")]
    StaleLease,
}

pub struct IdempotencyKey(String);

impl IdempotencyKey {
    pub fn parse(value: &str) -> Result<Self, SubmissionError> {
        if value.is_empty()
            || value.len() > 128
            || !value.bytes().all(|byte| {
                byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':')
            })
        {
            return Err(SubmissionError::InvalidIdempotencyKey);
        }
        Ok(Self(value.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

pub trait SubmissionRepository {
    fn create(
        &self,
        organization_id: Uuid,
        key: IdempotencyKey,
        input: SubmissionInput,
    ) -> impl Future<Output = Result<Submission, SubmissionError>> + Send;

    fn get(
        &self,
        organization_id: Uuid,
        submission_id: Uuid,
    ) -> impl Future<Output = Result<Option<Submission>, SubmissionError>> + Send;
}
