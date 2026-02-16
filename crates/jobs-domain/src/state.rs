use crate::error::JobsDomainError;

/// Job-level state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobState {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

impl JobState {
    pub fn can_transition_to(self, next: Self) -> bool {
        matches!(
            (self, next),
            (Self::Pending, Self::Running)
                | (Self::Running, Self::Completed)
                | (Self::Running, Self::Failed)
                | (Self::Pending, Self::Cancelled)
                | (Self::Running, Self::Cancelled)
        )
    }

    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Cancelled)
    }

    pub fn transition_to(self, next: Self) -> Result<Self, JobsDomainError> {
        if self.can_transition_to(next) {
            Ok(next)
        } else {
            Err(JobsDomainError::InvalidTransition {
                from: format!("{self:?}"),
                to: format!("{next:?}"),
            })
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }
}

impl std::fmt::Display for JobState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Task-level state machine.
/// Compatible with watcher expectations: terminal states are "completed", "failed", "cancelled".
/// Watcher resets non-terminal stuck tasks to "pending".
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskState {
    Blocked,
    Pending,
    Claimed,
    Running,
    Completed,
    Failed,
    Cancelled,
}

impl TaskState {
    pub fn can_transition_to(self, next: Self) -> bool {
        matches!(
            (self, next),
            // Normal flow
            (Self::Blocked, Self::Pending)
                | (Self::Pending, Self::Claimed)
                | (Self::Claimed, Self::Running)
                | (Self::Running, Self::Completed)
                | (Self::Running, Self::Failed)
                // Watcher retries (claim timeout or execution timeout)
                | (Self::Claimed, Self::Pending)
                | (Self::Running, Self::Pending)
                // Cancel from any non-terminal
                | (Self::Blocked, Self::Cancelled)
                | (Self::Pending, Self::Cancelled)
                | (Self::Claimed, Self::Cancelled)
                | (Self::Running, Self::Cancelled)
        )
    }

    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Cancelled)
    }

    pub fn transition_to(self, next: Self) -> Result<Self, JobsDomainError> {
        if self.can_transition_to(next) {
            Ok(next)
        } else {
            Err(JobsDomainError::InvalidTransition {
                from: format!("{self:?}"),
                to: format!("{next:?}"),
            })
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Blocked => "blocked",
            Self::Pending => "pending",
            Self::Claimed => "claimed",
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }
}

impl std::fmt::Display for TaskState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- JobState transitions --

    #[test]
    fn job_pending_to_running() {
        assert!(JobState::Pending.can_transition_to(JobState::Running));
    }

    #[test]
    fn job_running_to_completed() {
        assert!(JobState::Running.can_transition_to(JobState::Completed));
    }

    #[test]
    fn job_running_to_failed() {
        assert!(JobState::Running.can_transition_to(JobState::Failed));
    }

    #[test]
    fn job_cancel_from_pending_and_running() {
        assert!(JobState::Pending.can_transition_to(JobState::Cancelled));
        assert!(JobState::Running.can_transition_to(JobState::Cancelled));
    }

    #[test]
    fn job_no_backward_transitions() {
        assert!(!JobState::Running.can_transition_to(JobState::Pending));
        assert!(!JobState::Completed.can_transition_to(JobState::Running));
        assert!(!JobState::Failed.can_transition_to(JobState::Running));
    }

    #[test]
    fn job_no_transitions_from_terminal() {
        for terminal in [JobState::Completed, JobState::Failed, JobState::Cancelled] {
            for target in [
                JobState::Pending,
                JobState::Running,
                JobState::Completed,
                JobState::Failed,
                JobState::Cancelled,
            ] {
                assert!(!terminal.can_transition_to(target));
            }
        }
    }

    #[test]
    fn job_terminal_states() {
        assert!(!JobState::Pending.is_terminal());
        assert!(!JobState::Running.is_terminal());
        assert!(JobState::Completed.is_terminal());
        assert!(JobState::Failed.is_terminal());
        assert!(JobState::Cancelled.is_terminal());
    }

    #[test]
    fn job_transition_to_returns_error_on_invalid() {
        let result = JobState::Completed.transition_to(JobState::Running);
        assert!(result.is_err());
    }

    // -- TaskState transitions --

    #[test]
    fn task_normal_flow() {
        assert!(TaskState::Blocked.can_transition_to(TaskState::Pending));
        assert!(TaskState::Pending.can_transition_to(TaskState::Claimed));
        assert!(TaskState::Claimed.can_transition_to(TaskState::Running));
        assert!(TaskState::Running.can_transition_to(TaskState::Completed));
    }

    #[test]
    fn task_running_to_failed() {
        assert!(TaskState::Running.can_transition_to(TaskState::Failed));
    }

    #[test]
    fn task_watcher_retries() {
        assert!(TaskState::Claimed.can_transition_to(TaskState::Pending));
        assert!(TaskState::Running.can_transition_to(TaskState::Pending));
    }

    #[test]
    fn task_cancel_from_any_non_terminal() {
        assert!(TaskState::Blocked.can_transition_to(TaskState::Cancelled));
        assert!(TaskState::Pending.can_transition_to(TaskState::Cancelled));
        assert!(TaskState::Claimed.can_transition_to(TaskState::Cancelled));
        assert!(TaskState::Running.can_transition_to(TaskState::Cancelled));
    }

    #[test]
    fn task_no_transitions_from_terminal() {
        for terminal in [
            TaskState::Completed,
            TaskState::Failed,
            TaskState::Cancelled,
        ] {
            for target in [
                TaskState::Blocked,
                TaskState::Pending,
                TaskState::Claimed,
                TaskState::Running,
                TaskState::Completed,
                TaskState::Failed,
                TaskState::Cancelled,
            ] {
                assert!(!terminal.can_transition_to(target));
            }
        }
    }

    #[test]
    fn task_no_skip_transitions() {
        assert!(!TaskState::Blocked.can_transition_to(TaskState::Claimed));
        assert!(!TaskState::Blocked.can_transition_to(TaskState::Running));
        assert!(!TaskState::Pending.can_transition_to(TaskState::Running));
    }

    #[test]
    fn task_terminal_states() {
        assert!(!TaskState::Blocked.is_terminal());
        assert!(!TaskState::Pending.is_terminal());
        assert!(!TaskState::Claimed.is_terminal());
        assert!(!TaskState::Running.is_terminal());
        assert!(TaskState::Completed.is_terminal());
        assert!(TaskState::Failed.is_terminal());
        assert!(TaskState::Cancelled.is_terminal());
    }

    // -- Serde --

    #[test]
    fn job_state_serde_snake_case() {
        let json = serde_json::to_string(&JobState::Running).unwrap();
        assert_eq!(json, "\"running\"");
        let back: JobState = serde_json::from_str(&json).unwrap();
        assert_eq!(back, JobState::Running);
    }

    #[test]
    fn task_state_serde_snake_case() {
        let json = serde_json::to_string(&TaskState::Blocked).unwrap();
        assert_eq!(json, "\"blocked\"");
        let back: TaskState = serde_json::from_str(&json).unwrap();
        assert_eq!(back, TaskState::Blocked);
    }
}
