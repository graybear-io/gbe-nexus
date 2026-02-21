/// Subject builders for the jobs domain.
///
/// Job lifecycle subjects are new (`gbe.jobs.*`).
/// Task subjects reuse the existing hierarchy (`gbe.tasks.*`).
pub mod jobs {
    #[must_use]
    pub fn created(job_type: &str) -> String {
        format!("gbe.jobs.{job_type}.created")
    }

    #[must_use]
    pub fn completed(job_type: &str) -> String {
        format!("gbe.jobs.{job_type}.completed")
    }

    #[must_use]
    pub fn failed(job_type: &str) -> String {
        format!("gbe.jobs.{job_type}.failed")
    }

    #[must_use]
    pub fn cancelled(job_type: &str) -> String {
        format!("gbe.jobs.{job_type}.cancelled")
    }

    /// Wildcard for all events of a job type (NATS-compatible).
    #[must_use]
    pub fn all(job_type: &str) -> String {
        format!("gbe.jobs.{job_type}.*")
    }
}

pub mod tasks {
    #[must_use]
    pub fn queue(task_type: &str) -> String {
        format!("gbe.tasks.{task_type}.queue")
    }

    #[must_use]
    pub fn progress(task_type: &str) -> String {
        format!("gbe.tasks.{task_type}.progress")
    }

    #[must_use]
    pub fn terminal(task_type: &str) -> String {
        format!("gbe.tasks.{task_type}.terminal")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn job_subjects() {
        assert_eq!(
            jobs::created("daily-report"),
            "gbe.jobs.daily-report.created"
        );
        assert_eq!(
            jobs::completed("daily-report"),
            "gbe.jobs.daily-report.completed"
        );
        assert_eq!(jobs::failed("daily-report"), "gbe.jobs.daily-report.failed");
        assert_eq!(
            jobs::cancelled("daily-report"),
            "gbe.jobs.daily-report.cancelled"
        );
        assert_eq!(jobs::all("daily-report"), "gbe.jobs.daily-report.*");
    }

    #[test]
    fn task_subjects() {
        assert_eq!(tasks::queue("email-send"), "gbe.tasks.email-send.queue");
        assert_eq!(
            tasks::progress("email-send"),
            "gbe.tasks.email-send.progress"
        );
        assert_eq!(
            tasks::terminal("email-send"),
            "gbe.tasks.email-send.terminal"
        );
    }
}
