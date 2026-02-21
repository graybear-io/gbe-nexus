//! KV key builders and field name constants for job/task state records.

/// Job state record key: `gbe.state.jobs.{job_type}.{job_id}`
#[must_use]
pub fn job_key(job_type: &str, job_id: &str) -> String {
    format!("gbe.state.jobs.{job_type}.{job_id}")
}

/// Task state record key: `gbe.state.tasks.{task_type}.{task_id}`
/// Follows existing KV pattern from kv-state-store.md.
#[must_use]
pub fn task_key(task_type: &str, task_id: &str) -> String {
    format!("gbe.state.tasks.{task_type}.{task_id}")
}

/// Index key mapping job -> task by name: `gbe.idx.jobs.{job_id}.tasks.{task_name}`
#[must_use]
pub fn job_task_index_key(job_id: &str, task_name: &str) -> String {
    format!("gbe.idx.jobs.{job_id}.tasks.{task_name}")
}

/// Scan prefix for all tasks in a job: `gbe.idx.jobs.{job_id}.tasks.`
#[must_use]
pub fn job_tasks_prefix(job_id: &str) -> String {
    format!("gbe.idx.jobs.{job_id}.tasks.")
}

/// Field name constants for type-safe KV access.
pub mod fields {
    pub mod job {
        pub const STATE: &str = "state";
        pub const JOB_TYPE: &str = "job_type";
        pub const JOB_ID: &str = "job_id";
        pub const ORG_ID: &str = "org_id";
        pub const TASK_COUNT: &str = "task_count";
        pub const COMPLETED_COUNT: &str = "completed_count";
        pub const FAILED_COUNT: &str = "failed_count";
        pub const CREATED_AT: &str = "created_at";
        pub const UPDATED_AT: &str = "updated_at";
        pub const ERROR: &str = "error";
        pub const DEFINITION_REF: &str = "definition_ref";
    }

    pub mod task {
        pub const STATE: &str = "state";
        pub const TASK_TYPE: &str = "task_type";
        pub const TASK_ID: &str = "task_id";
        pub const JOB_ID: &str = "job_id";
        pub const ORG_ID: &str = "org_id";
        pub const TASK_NAME: &str = "task_name";
        pub const WORKER: &str = "worker";
        pub const CURRENT_STEP: &str = "current_step";
        pub const STEP_COUNT: &str = "step_count";
        pub const CREATED_AT: &str = "created_at";
        pub const UPDATED_AT: &str = "updated_at";
        pub const TIMEOUT_AT: &str = "timeout_at";
        pub const ERROR: &str = "error";
        pub const PARAMS_REF: &str = "params_ref";
        pub const RESULT_REF: &str = "result_ref";
        pub const RETRY_COUNT: &str = "retry_count";
        pub const MAX_RETRIES: &str = "max_retries";
        pub const DEPENDS_ON: &str = "depends_on";
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn job_key_format() {
        assert_eq!(
            job_key("daily-report", "job_abc123"),
            "gbe.state.jobs.daily-report.job_abc123"
        );
    }

    #[test]
    fn task_key_format() {
        assert_eq!(
            task_key("email-send", "task_xyz789"),
            "gbe.state.tasks.email-send.task_xyz789"
        );
    }

    #[test]
    fn index_key_format() {
        assert_eq!(
            job_task_index_key("job_abc123", "fetch-data"),
            "gbe.idx.jobs.job_abc123.tasks.fetch-data"
        );
    }

    #[test]
    fn index_prefix_format() {
        assert_eq!(
            job_tasks_prefix("job_abc123"),
            "gbe.idx.jobs.job_abc123.tasks."
        );
    }
}
