use crate::definition::TaskParams;
use crate::ids::{JobId, OrgId, TaskId, TaskType};

// -- Job-level payloads --
// Published to gbe.jobs.{job_type}.* subjects.
// Wrap in DomainPayload<T> from gbe-nexus before publishing.

/// Job created with all tasks.
/// Subject: `gbe.jobs.{job_type}.created`
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct JobCreated {
    pub job_id: JobId,
    pub org_id: OrgId,
    pub job_type: String,
    pub task_count: u32,
    pub task_ids: Vec<TaskId>,
    pub created_at: u64,
    pub definition_ref: Option<String>,
}

/// All tasks completed successfully.
/// Subject: `gbe.jobs.{job_type}.completed`
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct JobCompleted {
    pub job_id: JobId,
    pub org_id: OrgId,
    pub job_type: String,
    pub completed_at: u64,
    pub result_ref: Option<String>,
}

/// Job failed terminally (task exhausted retries).
/// Subject: `gbe.jobs.{job_type}.failed`
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct JobFailed {
    pub job_id: JobId,
    pub org_id: OrgId,
    pub job_type: String,
    pub failed_at: u64,
    pub failed_task_id: TaskId,
    pub error: String,
}

/// Job cancelled externally.
/// Subject: `gbe.jobs.{job_type}.cancelled`
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct JobCancelled {
    pub job_id: JobId,
    pub org_id: OrgId,
    pub job_type: String,
    pub cancelled_at: u64,
    pub reason: String,
}

// -- Task-level payloads --
// Published to existing gbe.tasks.{task_type}.* subjects.
// Wrap in DomainPayload<T> from gbe-nexus before publishing.

/// Task is ready for a worker to claim.
/// Subject: `gbe.tasks.{task_type}.queue`
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TaskQueued {
    pub task_id: TaskId,
    pub job_id: JobId,
    pub org_id: OrgId,
    pub task_type: TaskType,
    pub params: TaskParams,
    pub retry_count: u32,
}

/// Worker step progress update.
/// Subject: `gbe.tasks.{task_type}.progress`
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TaskProgress {
    pub task_id: TaskId,
    pub job_id: JobId,
    pub current_step: u32,
    pub step_count: Option<u32>,
    pub message: Option<String>,
}

/// Task completed successfully.
/// Subject: `gbe.tasks.{task_type}.terminal`
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TaskCompleted {
    pub task_id: TaskId,
    pub job_id: JobId,
    pub task_type: TaskType,
    pub completed_at: u64,
    pub result_ref: Option<String>,
}

/// Task failed (retries may remain).
/// Subject: `gbe.tasks.{task_type}.terminal`
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TaskFailed {
    pub task_id: TaskId,
    pub job_id: JobId,
    pub task_type: TaskType,
    pub failed_at: u64,
    pub error: String,
    pub retry_count: u32,
    pub max_retries: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn job_created_round_trip() {
        let payload = JobCreated {
            job_id: JobId::new("job_test-001").unwrap(),
            org_id: OrgId::new("org_acme").unwrap(),
            job_type: "daily-report".to_string(),
            task_count: 3,
            task_ids: vec![
                TaskId::new("task_a").unwrap(),
                TaskId::new("task_b").unwrap(),
                TaskId::new("task_c").unwrap(),
            ],
            created_at: 1_707_934_567_000,
            definition_ref: None,
        };
        let json = serde_json::to_string(&payload).unwrap();
        let back: JobCreated = serde_json::from_str(&json).unwrap();
        assert_eq!(back.job_id, payload.job_id);
        assert_eq!(back.task_count, 3);
        assert_eq!(back.task_ids.len(), 3);
    }

    #[test]
    fn task_queued_round_trip() {
        let mut params = TaskParams::default();
        params
            .entries
            .insert("source".to_string(), "api".to_string());
        let payload = TaskQueued {
            task_id: TaskId::new("task_fetch-1").unwrap(),
            job_id: JobId::new("job_daily-001").unwrap(),
            org_id: OrgId::new("org_acme").unwrap(),
            task_type: TaskType::new("data-fetch").unwrap(),
            params,
            retry_count: 0,
        };
        let json = serde_json::to_string(&payload).unwrap();
        let back: TaskQueued = serde_json::from_str(&json).unwrap();
        assert_eq!(back.task_id, payload.task_id);
        assert_eq!(back.params.entries.get("source").unwrap(), "api");
    }

    #[test]
    fn task_failed_round_trip() {
        let payload = TaskFailed {
            task_id: TaskId::new("task_send-1").unwrap(),
            job_id: JobId::new("job_daily-001").unwrap(),
            task_type: TaskType::new("email-send").unwrap(),
            failed_at: 1_707_934_600_000,
            error: "smtp timeout".to_string(),
            retry_count: 2,
            max_retries: 3,
        };
        let json = serde_json::to_string(&payload).unwrap();
        let back: TaskFailed = serde_json::from_str(&json).unwrap();
        assert_eq!(back.retry_count, 2);
        assert_eq!(back.error, "smtp timeout");
    }

    #[test]
    fn job_completed_round_trip() {
        let payload = JobCompleted {
            job_id: JobId::new("job_done-1").unwrap(),
            org_id: OrgId::new("org_test").unwrap(),
            job_type: "daily-report".to_string(),
            completed_at: 1_707_935_000_000,
            result_ref: Some("s3://results/job_done-1.json".to_string()),
        };
        let json = serde_json::to_string(&payload).unwrap();
        let back: JobCompleted = serde_json::from_str(&json).unwrap();
        assert_eq!(back.result_ref.unwrap(), "s3://results/job_done-1.json");
    }
}
