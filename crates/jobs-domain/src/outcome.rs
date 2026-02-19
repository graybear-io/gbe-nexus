/// Outcome reported by an operative after executing a task.
/// Published on the terminal stream: gbe.tasks.{task_type}.terminal
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum TaskOutcome {
    Completed {
        output: Vec<String>,
        result_ref: Option<String>,
    },
    Failed {
        exit_code: i32,
        error: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn completed_round_trip() {
        let outcome = TaskOutcome::Completed {
            output: vec!["row1".to_string(), "row2".to_string()],
            result_ref: Some("s3://bucket/output.csv".to_string()),
        };
        let json = serde_json::to_string(&outcome).unwrap();
        let back: TaskOutcome = serde_json::from_str(&json).unwrap();
        match back {
            TaskOutcome::Completed { output, result_ref } => {
                assert_eq!(output.len(), 2);
                assert_eq!(result_ref.unwrap(), "s3://bucket/output.csv");
            }
            _ => panic!("expected Completed"),
        }
    }

    #[test]
    fn failed_round_trip() {
        let outcome = TaskOutcome::Failed {
            exit_code: 1,
            error: "connection timeout".to_string(),
        };
        let json = serde_json::to_string(&outcome).unwrap();
        let back: TaskOutcome = serde_json::from_str(&json).unwrap();
        match back {
            TaskOutcome::Failed { exit_code, error } => {
                assert_eq!(exit_code, 1);
                assert_eq!(error, "connection timeout");
            }
            _ => panic!("expected Failed"),
        }
    }
}
