use crate::error::JobsDomainError;

/// Checks that a string contains only alphanumeric chars, hyphens, and underscores.
fn is_valid_slug(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 64
        && s.chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
}

/// Checks that a string is a valid prefixed identifier (e.g. "job_foo-bar").
fn is_valid_prefixed_id(s: &str, prefix: &str) -> bool {
    s.starts_with(prefix) && is_valid_slug(s)
}

macro_rules! validated_id {
    ($name:ident, $prefix:expr, $err:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
        #[serde(try_from = "String", into = "String")]
        pub struct $name(String);

        impl $name {
            pub fn new(raw: &str) -> Result<Self, JobsDomainError> {
                if !is_valid_prefixed_id(raw, $prefix) {
                    return Err(JobsDomainError::$err(raw.to_string()));
                }
                Ok(Self(raw.to_string()))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl TryFrom<String> for $name {
            type Error = JobsDomainError;
            fn try_from(s: String) -> Result<Self, Self::Error> {
                Self::new(&s)
            }
        }

        impl From<$name> for String {
            fn from(id: $name) -> String {
                id.0
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                &self.0
            }
        }
    };
}

validated_id!(JobId, "job_", InvalidJobId);
validated_id!(TaskId, "task_", InvalidTaskId);
validated_id!(OrgId, "org_", InvalidOrgId);

/// Task type slug: lowercase alphanumeric + hyphens, 1-48 chars.
/// No prefix required. Examples: "email-send", "data-fetch".
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct TaskType(String);

impl TaskType {
    pub fn new(raw: &str) -> Result<Self, JobsDomainError> {
        if raw.is_empty()
            || raw.len() > 48
            || !raw
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
            || raw.starts_with('-')
            || raw.ends_with('-')
        {
            return Err(JobsDomainError::InvalidTaskType(raw.to_string()));
        }
        Ok(Self(raw.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for TaskType {
    type Error = JobsDomainError;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::new(&s)
    }
}

impl From<TaskType> for String {
    fn from(t: TaskType) -> String {
        t.0
    }
}

impl std::fmt::Display for TaskType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for TaskType {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_job_id() {
        assert!(JobId::new("job_daily-report").is_ok());
        assert!(JobId::new("job_abc_123").is_ok());
        assert!(JobId::new("job_a").is_ok());
    }

    #[test]
    fn invalid_job_id() {
        assert!(JobId::new("").is_err());
        assert!(JobId::new("daily-report").is_err()); // missing prefix
        assert!(JobId::new("job_").is_ok()); // prefix only is valid slug
        assert!(JobId::new("job_has spaces").is_err());
        assert!(JobId::new("job_has.dots").is_err());
        let long = "job_".to_string() + &"a".repeat(61);
        assert!(JobId::new(&long).is_err()); // too long
    }

    #[test]
    fn valid_task_id() {
        assert!(TaskId::new("task_fetch-data").is_ok());
        assert!(TaskId::new("task_step_1").is_ok());
    }

    #[test]
    fn invalid_task_id() {
        assert!(TaskId::new("").is_err());
        assert!(TaskId::new("fetch-data").is_err());
        assert!(TaskId::new("job_wrong-prefix").is_err());
    }

    #[test]
    fn valid_org_id() {
        assert!(OrgId::new("org_acme").is_ok());
        assert!(OrgId::new("org_test-corp").is_ok());
    }

    #[test]
    fn invalid_org_id() {
        assert!(OrgId::new("").is_err());
        assert!(OrgId::new("acme").is_err());
    }

    #[test]
    fn valid_task_type() {
        assert!(TaskType::new("email-send").is_ok());
        assert!(TaskType::new("data-fetch").is_ok());
        assert!(TaskType::new("a").is_ok());
        assert!(TaskType::new("step1").is_ok());
    }

    #[test]
    fn invalid_task_type() {
        assert!(TaskType::new("").is_err());
        assert!(TaskType::new("-leading").is_err());
        assert!(TaskType::new("trailing-").is_err());
        assert!(TaskType::new("UPPERCASE").is_err());
        assert!(TaskType::new("has spaces").is_err());
        assert!(TaskType::new("has.dots").is_err());
        assert!(TaskType::new(&"a".repeat(49)).is_err()); // too long
    }

    #[test]
    fn serde_round_trip_job_id() {
        let id = JobId::new("job_test-123").unwrap();
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, "\"job_test-123\"");
        let back: JobId = serde_json::from_str(&json).unwrap();
        assert_eq!(id, back);
    }

    #[test]
    fn serde_rejects_invalid_job_id() {
        let result: Result<JobId, _> = serde_json::from_str("\"not-a-job-id\"");
        assert!(result.is_err());
    }

    #[test]
    fn serde_round_trip_task_type() {
        let tt = TaskType::new("email-send").unwrap();
        let json = serde_json::to_string(&tt).unwrap();
        let back: TaskType = serde_json::from_str(&json).unwrap();
        assert_eq!(tt, back);
    }
}
