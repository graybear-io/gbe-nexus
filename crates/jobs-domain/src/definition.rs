use std::collections::{HashMap, HashSet, VecDeque};

use crate::error::JobsDomainError;
use crate::ids::TaskType;

/// Root job definition. Deserializable from YAML or JSON config files.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct JobDefinition {
    /// Schema version for the definition format itself.
    pub v: u32,
    /// Human-readable name.
    pub name: String,
    /// Unique slug used in subject/key construction.
    pub job_type: String,
    /// Tasks in this job. Order is irrelevant; DAG is defined by `depends_on`.
    pub tasks: Vec<TaskDefinition>,
}

/// One task within a job definition.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TaskDefinition {
    /// Unique name within the job. Used as the task's identity in the DAG.
    pub name: String,
    /// Task type determines which worker pool handles this.
    pub task_type: TaskType,
    /// Names of tasks that must complete before this one starts.
    #[serde(default)]
    pub depends_on: Vec<String>,
    /// Static parameters passed to the worker.
    #[serde(default)]
    pub params: TaskParams,
    /// Per-task timeout override in seconds.
    #[serde(default)]
    pub timeout_secs: Option<u64>,
    /// Per-task retry limit override.
    #[serde(default)]
    pub max_retries: Option<u32>,
}

/// Typed parameter map. String keys, string values.
/// Complex values use claim-check pattern (store externally, pass ref).
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct TaskParams {
    #[serde(flatten)]
    pub entries: HashMap<String, String>,
}

impl JobDefinition {
    /// Validate the DAG: no duplicate names, all dependency refs exist, no cycles.
    pub fn validate(&self) -> Result<(), JobsDomainError> {
        if self.tasks.is_empty() {
            return Err(JobsDomainError::ValidationFailed(
                "job must have at least one task".to_string(),
            ));
        }

        // Check for duplicate task names
        let mut names = HashSet::new();
        for task in &self.tasks {
            if !names.insert(task.name.as_str()) {
                return Err(JobsDomainError::ValidationFailed(format!(
                    "duplicate task name: {}",
                    task.name
                )));
            }
        }

        // Check all dependency refs exist
        for task in &self.tasks {
            for dep in &task.depends_on {
                if !names.contains(dep.as_str()) {
                    return Err(JobsDomainError::UnknownDependency {
                        task: task.name.clone(),
                        dependency: dep.clone(),
                    });
                }
            }
            // Self-dependency
            if task.depends_on.contains(&task.name) {
                return Err(JobsDomainError::CyclicDependency);
            }
        }

        // Topological sort (Kahn's algorithm) to detect cycles
        let mut in_degree: HashMap<&str, usize> = self
            .tasks
            .iter()
            .map(|t| (t.name.as_str(), t.depends_on.len()))
            .collect();

        let mut queue: VecDeque<&str> = in_degree
            .iter()
            .filter(|&(_, deg)| *deg == 0)
            .map(|(&name, _)| name)
            .collect();

        let mut visited = 0usize;

        while let Some(node) = queue.pop_front() {
            visited += 1;
            for task in &self.tasks {
                if task.depends_on.iter().any(|d| d == node)
                    && let Some(deg) = in_degree.get_mut(task.name.as_str())
                {
                    *deg -= 1;
                    if *deg == 0 {
                        queue.push_back(task.name.as_str());
                    }
                }
            }
        }

        if visited != self.tasks.len() {
            return Err(JobsDomainError::CyclicDependency);
        }

        Ok(())
    }

    /// Return task names in topological order (roots first).
    pub fn topological_order(&self) -> Result<Vec<&str>, JobsDomainError> {
        self.validate()?;

        let mut in_degree: HashMap<&str, usize> = self
            .tasks
            .iter()
            .map(|t| (t.name.as_str(), t.depends_on.len()))
            .collect();

        let mut queue: VecDeque<&str> = in_degree
            .iter()
            .filter(|&(_, deg)| *deg == 0)
            .map(|(&name, _)| name)
            .collect();

        let mut order = Vec::with_capacity(self.tasks.len());

        while let Some(node) = queue.pop_front() {
            order.push(node);
            for task in &self.tasks {
                if task.depends_on.iter().any(|d| d == node)
                    && let Some(deg) = in_degree.get_mut(task.name.as_str())
                {
                    *deg -= 1;
                    if *deg == 0 {
                        queue.push_back(task.name.as_str());
                    }
                }
            }
        }

        Ok(order)
    }

    /// Return task names that have no dependencies (DAG roots).
    pub fn roots(&self) -> Vec<&str> {
        self.tasks
            .iter()
            .filter(|t| t.depends_on.is_empty())
            .map(|t| t.name.as_str())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn simple_dag() -> JobDefinition {
        JobDefinition {
            v: 1,
            name: "Test Job".to_string(),
            job_type: "test-job".to_string(),
            tasks: vec![
                TaskDefinition {
                    name: "fetch".to_string(),
                    task_type: TaskType::new("data-fetch").unwrap(),
                    depends_on: vec![],
                    params: TaskParams::default(),
                    timeout_secs: Some(120),
                    max_retries: None,
                },
                TaskDefinition {
                    name: "transform".to_string(),
                    task_type: TaskType::new("data-transform").unwrap(),
                    depends_on: vec!["fetch".to_string()],
                    params: TaskParams::default(),
                    timeout_secs: None,
                    max_retries: Some(3),
                },
                TaskDefinition {
                    name: "send".to_string(),
                    task_type: TaskType::new("email-send").unwrap(),
                    depends_on: vec!["transform".to_string()],
                    params: TaskParams::default(),
                    timeout_secs: None,
                    max_retries: None,
                },
            ],
        }
    }

    #[test]
    fn valid_dag_passes_validation() {
        assert!(simple_dag().validate().is_ok());
    }

    #[test]
    fn topological_order_roots_first() {
        let dag = simple_dag();
        let order = dag.topological_order().unwrap();
        assert_eq!(order, vec!["fetch", "transform", "send"]);
    }

    #[test]
    fn roots_returns_tasks_without_deps() {
        let dag = simple_dag();
        let roots = dag.roots();
        assert_eq!(roots, vec!["fetch"]);
    }

    #[test]
    fn parallel_tasks_both_root() {
        let def = JobDefinition {
            v: 1,
            name: "Parallel".to_string(),
            job_type: "parallel".to_string(),
            tasks: vec![
                TaskDefinition {
                    name: "a".to_string(),
                    task_type: TaskType::new("work").unwrap(),
                    depends_on: vec![],
                    params: TaskParams::default(),
                    timeout_secs: None,
                    max_retries: None,
                },
                TaskDefinition {
                    name: "b".to_string(),
                    task_type: TaskType::new("work").unwrap(),
                    depends_on: vec![],
                    params: TaskParams::default(),
                    timeout_secs: None,
                    max_retries: None,
                },
                TaskDefinition {
                    name: "c".to_string(),
                    task_type: TaskType::new("work").unwrap(),
                    depends_on: vec!["a".to_string(), "b".to_string()],
                    params: TaskParams::default(),
                    timeout_secs: None,
                    max_retries: None,
                },
            ],
        };
        assert!(def.validate().is_ok());
        let roots = def.roots();
        assert_eq!(roots.len(), 2);
        assert!(roots.contains(&"a"));
        assert!(roots.contains(&"b"));
    }

    #[test]
    fn cyclic_dependency_detected() {
        let def = JobDefinition {
            v: 1,
            name: "Cycle".to_string(),
            job_type: "cycle".to_string(),
            tasks: vec![
                TaskDefinition {
                    name: "a".to_string(),
                    task_type: TaskType::new("work").unwrap(),
                    depends_on: vec!["b".to_string()],
                    params: TaskParams::default(),
                    timeout_secs: None,
                    max_retries: None,
                },
                TaskDefinition {
                    name: "b".to_string(),
                    task_type: TaskType::new("work").unwrap(),
                    depends_on: vec!["a".to_string()],
                    params: TaskParams::default(),
                    timeout_secs: None,
                    max_retries: None,
                },
            ],
        };
        assert!(matches!(
            def.validate(),
            Err(JobsDomainError::CyclicDependency)
        ));
    }

    #[test]
    fn self_dependency_detected() {
        let def = JobDefinition {
            v: 1,
            name: "Self".to_string(),
            job_type: "self".to_string(),
            tasks: vec![TaskDefinition {
                name: "a".to_string(),
                task_type: TaskType::new("work").unwrap(),
                depends_on: vec!["a".to_string()],
                params: TaskParams::default(),
                timeout_secs: None,
                max_retries: None,
            }],
        };
        assert!(matches!(
            def.validate(),
            Err(JobsDomainError::CyclicDependency)
        ));
    }

    #[test]
    fn unknown_dependency_detected() {
        let def = JobDefinition {
            v: 1,
            name: "Unknown".to_string(),
            job_type: "unknown".to_string(),
            tasks: vec![TaskDefinition {
                name: "a".to_string(),
                task_type: TaskType::new("work").unwrap(),
                depends_on: vec!["nonexistent".to_string()],
                params: TaskParams::default(),
                timeout_secs: None,
                max_retries: None,
            }],
        };
        assert!(matches!(
            def.validate(),
            Err(JobsDomainError::UnknownDependency { .. })
        ));
    }

    #[test]
    fn duplicate_task_name_detected() {
        let def = JobDefinition {
            v: 1,
            name: "Dup".to_string(),
            job_type: "dup".to_string(),
            tasks: vec![
                TaskDefinition {
                    name: "a".to_string(),
                    task_type: TaskType::new("work").unwrap(),
                    depends_on: vec![],
                    params: TaskParams::default(),
                    timeout_secs: None,
                    max_retries: None,
                },
                TaskDefinition {
                    name: "a".to_string(),
                    task_type: TaskType::new("work").unwrap(),
                    depends_on: vec![],
                    params: TaskParams::default(),
                    timeout_secs: None,
                    max_retries: None,
                },
            ],
        };
        assert!(matches!(
            def.validate(),
            Err(JobsDomainError::ValidationFailed(_))
        ));
    }

    #[test]
    fn empty_tasks_rejected() {
        let def = JobDefinition {
            v: 1,
            name: "Empty".to_string(),
            job_type: "empty".to_string(),
            tasks: vec![],
        };
        assert!(matches!(
            def.validate(),
            Err(JobsDomainError::ValidationFailed(_))
        ));
    }

    #[test]
    fn params_round_trip() {
        let mut params = TaskParams::default();
        params
            .entries
            .insert("source".to_string(), "billing-api".to_string());
        let json = serde_json::to_string(&params).unwrap();
        let back: TaskParams = serde_json::from_str(&json).unwrap();
        assert_eq!(back.entries.get("source").unwrap(), "billing-api");
    }
}
