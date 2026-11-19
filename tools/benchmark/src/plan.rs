//! The authored comparison binds commands, inputs, and the oracle before execution.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Plan {
    pub schema: String,
    pub name: String,
    pub working_directory: PathBuf,
    pub inputs: Vec<PathBuf>,
    pub baseline: CommandSpec,
    pub candidate: CommandSpec,
    pub warmups: usize,
    pub pairs: usize,
    pub timeout_ms: u64,
    pub measurement: Measurement,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CommandSpec {
    pub program: PathBuf,
    #[serde(default)]
    pub arguments: Vec<String>,
    #[serde(default)]
    pub environment: BTreeMap<String, String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Measurement {
    Compiler {
        source: PathBuf,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        candidate_source: Option<PathBuf>,
        library_root: PathBuf,
        target: String,
        expected: ExpectedOutput,
        require_identical_artifacts: bool,
    },
    Process {
        stdin: String,
        expected: ExpectedOutput,
    },
    ReportedWork {
        stdin: String,
        work_units: u64,
        checksum: String,
    },
}

impl Measurement {
    pub fn compiler_source(&self, role: &str) -> Option<&std::path::Path> {
        match self {
            Self::Compiler {
                source,
                candidate_source,
                ..
            } => Some(if role == "candidate" {
                candidate_source.as_ref().unwrap_or(source)
            } else {
                source
            }),
            _ => None,
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ExpectedOutput {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}

impl Plan {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != "omega-performance-plan/1" {
            return Err(format!("unsupported plan schema: {}", self.schema));
        }
        if self.name.trim().is_empty() || self.inputs.is_empty() {
            return Err("name and at least one declared input are required".into());
        }
        if !(1..=100).contains(&self.warmups) || !(2..=1000).contains(&self.pairs) {
            return Err("warmups must be 1..=100 and pairs must be 2..=1000".into());
        }
        if !(1000..=3_600_000).contains(&self.timeout_ms) {
            return Err("timeout_ms must be 1000..=3600000".into());
        }
        if let Measurement::ReportedWork {
            work_units,
            checksum,
            ..
        } = &self.measurement
            && (*work_units == 0 || checksum.is_empty())
        {
            return Err("reported work requires nonzero work_units and a checksum".into());
        }
        if let Measurement::Compiler { target, .. } = &self.measurement
            && !matches!(
                target.as_str(),
                "windows_x86_64" | "linux_x86_64" | "macos_arm64" | "linux_arm64" | "macos_x86_64"
            )
        {
            return Err("compiler comparisons must use an executable hosted target".into());
        }
        for command in [&self.baseline, &self.candidate] {
            if command.program.as_os_str().is_empty() {
                return Err("each command requires an explicit executable path".into());
            }
            if matches!(self.measurement, Measurement::Compiler { .. }) {
                let mut arguments = command.arguments.iter();
                while let Some(argument) = arguments.next() {
                    match argument.as_str() {
                        "--offline" => {}
                        "--disable-optimization" if arguments.next().is_some_and(|rule| rule.starts_with(|letter: char| letter.is_ascii_uppercase()) && rule.chars().all(|letter| letter.is_ascii_alphanumeric())) => {}
                        _ => return Err("compiler arguments support --offline and --disable-optimization <ExactName>; the plan owns source, target, library, and product".into()),
                    }
                }
            }
        }
        Ok(())
    }

    pub fn metric(&self) -> &'static str {
        match self.measurement {
            Measurement::Compiler { .. } => "compiler_process_wall_ns",
            Measurement::Process { .. } => "runtime_process_wall_ns",
            Measurement::ReportedWork { .. } => "reported_work_ns_per_unit",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Plan;
    use serde_json::json;

    #[test]
    fn compiler_options_cannot_replace_the_authored_subject_or_product() {
        let mut plan: Plan = serde_json::from_value(json!({
            "schema":"omega-performance-plan/1", "name":"argument control", "working_directory":".",
            "inputs":["main.omg"], "baseline":{"program":"omega"}, "candidate":{"program":"omega"},
            "warmups":1, "pairs":2, "timeout_ms":10000,
            "measurement":{"kind":"compiler", "source":"main.omg", "library_root":"library", "target":"windows_x86_64",
                "expected":{"exit_code":0,"stdout":"","stderr":""},"require_identical_artifacts":true}
        })).unwrap();
        plan.baseline.arguments = vec![
            "--offline".into(),
            "--disable-optimization".into(),
            "CopyPropagation".into(),
        ];
        assert!(plan.validate().is_ok());
        for arguments in [
            vec!["--check"],
            vec!["other.omg"],
            vec!["--disable-optimization"],
            vec!["--disable-optimization", "--target"],
            vec!["run"],
            vec!["--target=linux_x86_64"],
        ] {
            plan.candidate.arguments = arguments.into_iter().map(str::to_owned).collect();
            assert!(plan.validate().is_err(), "{:?}", plan.candidate.arguments);
        }
    }
}
