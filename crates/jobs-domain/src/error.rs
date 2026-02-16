/// Errors for job domain schema validation.
#[derive(Debug, thiserror::Error)]
pub enum JobsDomainError {
    #[error("invalid job id: {0}")]
    InvalidJobId(String),

    #[error("invalid task id: {0}")]
    InvalidTaskId(String),

    #[error("invalid org id: {0}")]
    InvalidOrgId(String),

    #[error("invalid task type: {0}")]
    InvalidTaskType(String),

    #[error("invalid state transition: {from} -> {to}")]
    InvalidTransition { from: String, to: String },

    #[error("unknown dependency '{dependency}' in task '{task}'")]
    UnknownDependency { task: String, dependency: String },

    #[error("job definition contains cyclic dependencies")]
    CyclicDependency,

    #[error("job definition validation: {0}")]
    ValidationFailed(String),
}
