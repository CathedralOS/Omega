//! Run an explicitly authored workload suite and preserve each independent outcome.

use crate::{experiment, records};
use serde::Deserialize;
use serde_json::json;
use std::path::{Path, PathBuf};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Suite {
    schema: String,
    plans: Vec<PathBuf>,
}

pub fn run(path: &Path, output: &Path) -> Result<(), String> {
    let path = path.canonicalize().map_err(|error| error.to_string())?;
    let suite: Suite =
        serde_json::from_slice(&std::fs::read(&path).map_err(|error| error.to_string())?)
            .map_err(|error| error.to_string())?;
    if suite.schema != "omega-performance-suite/1"
        || suite.plans.is_empty()
        || suite.plans.len() > 64
    {
        return Err(
            "suite requires 1..=64 explicit plans and schema omega-performance-suite/1".into(),
        );
    }
    let parent = path.parent().ok_or("suite has no parent")?;
    if let Some(parent) = output
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    std::fs::create_dir(output).map_err(|error| format!("create new suite directory: {error}"))?;
    let mut outcomes = Vec::new();
    let mut markdown = String::from("# Performance suite\n\n");
    let mut failed = false;
    for (plan_index, plan_path) in suite.plans.iter().enumerate() {
        let directory = output.join(format!("case-{plan_index:02}"));
        let result = experiment::compare(&parent.join(plan_path), &directory)
            .and_then(|_| records::validate(&directory));
        match result {
            Ok(run) => {
                markdown.push_str(&records::markdown(&run));
                markdown.push('\n');
                outcomes.push(json!({"plan":plan_path,"directory":directory,"status":"complete"}));
            }
            Err(reason) => {
                failed = true;
                markdown.push_str(&format!(
                    "## {}\n\nFailed: {}\n\n",
                    plan_path.display(),
                    reason
                ));
                outcomes.push(json!({"plan":plan_path,"directory":directory,"status":"failed","reason":reason}));
            }
        }
        std::fs::write(
            output.join("suite.json"),
            serde_json::to_vec_pretty(
                &json!({"schema":"omega-performance-suite-result/1","outcomes":outcomes}),
            )
            .map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())?;
        std::fs::write(output.join("summary.md"), &markdown).map_err(|error| error.to_string())?;
    }
    if failed {
        Err("one or more suite cases failed; their records were preserved".into())
    } else {
        Ok(())
    }
}
