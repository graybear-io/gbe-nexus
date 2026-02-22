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

pub mod lifecycle {
    #[must_use]
    pub fn started(component: &str) -> String {
        format!("gbe.events.lifecycle.{component}.started")
    }

    #[must_use]
    pub fn stopped(component: &str) -> String {
        format!("gbe.events.lifecycle.{component}.stopped")
    }

    #[must_use]
    pub fn heartbeat(component: &str) -> String {
        format!("gbe.events.lifecycle.{component}.heartbeat")
    }

    #[must_use]
    pub fn degraded(component: &str) -> String {
        format!("gbe.events.lifecycle.{component}.degraded")
    }

    /// Wildcard for all lifecycle events of a component (NATS-compatible).
    #[must_use]
    pub fn all(component: &str) -> String {
        format!("gbe.events.lifecycle.{component}.*")
    }

    /// Wildcard for all lifecycle events across all components.
    #[must_use]
    pub fn all_components() -> String {
        "gbe.events.lifecycle.*.*".to_string()
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

    #[test]
    fn lifecycle_subjects() {
        assert_eq!(
            lifecycle::started("operative"),
            "gbe.events.lifecycle.operative.started"
        );
        assert_eq!(
            lifecycle::stopped("oracle"),
            "gbe.events.lifecycle.oracle.stopped"
        );
        assert_eq!(
            lifecycle::heartbeat("sentinel"),
            "gbe.events.lifecycle.sentinel.heartbeat"
        );
        assert_eq!(
            lifecycle::degraded("watcher"),
            "gbe.events.lifecycle.watcher.degraded"
        );
        assert_eq!(
            lifecycle::all("operative"),
            "gbe.events.lifecycle.operative.*"
        );
        assert_eq!(lifecycle::all_components(), "gbe.events.lifecycle.*.*");
    }
}
