//! Exercise the CLI's evidence and refusal paths with real child processes.

use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

struct TestDirectory(PathBuf);
impl TestDirectory {
    fn new() -> Self {
        static SEQUENCE: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "omega-bench-test-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path.canonicalize().unwrap())
    }
    fn path(&self) -> &Path {
        &self.0
    }
}
impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn plan(directory: &Path) -> Value {
    std::fs::write(directory.join("input.txt"), "frozen input").unwrap();
    json!({"schema":"omega-performance-plan/1", "name":"instrument calibration", "working_directory":directory,
        "inputs":["input.txt"], "baseline":{"program":env!("CARGO_BIN_EXE_benchmark-probe"), "arguments":["plain"]},
        "candidate":{"program":env!("CARGO_BIN_EXE_benchmark-probe"), "arguments":["plain"]},
        "warmups":1, "pairs":2, "timeout_ms":10000,
        "measurement":{"kind":"process", "stdin":"", "expected":{"exit_code":0,"stdout":"native ok\n","stderr":""}}})
}

fn execute(directory: &Path, plan: &Value) -> Output {
    let path = directory.join("plan.json");
    std::fs::write(&path, serde_json::to_vec(plan).unwrap()).unwrap();
    Command::new(env!("CARGO_BIN_EXE_omega-bench"))
        .arg("compare")
        .arg(path)
        .arg(directory.join("result"))
        .output()
        .unwrap()
}

fn events(directory: &Path) -> Vec<Value> {
    std::fs::read_to_string(directory.join("result/journal.jsonl"))
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

#[test]
fn retains_warmups_abba_pairs_and_refuses_overwrite() {
    let directory = TestDirectory::new();
    let plan = plan(directory.path());
    let result = execute(directory.path(), &plan);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let events = events(directory.path());
    let samples: Vec<_> = events
        .iter()
        .filter(|event| event["event"] == "sample")
        .collect();
    assert_eq!(samples.len(), 6);
    assert_eq!(
        samples
            .iter()
            .filter(|event| event["sample"]["warmup"] == true)
            .count(),
        2
    );
    let roles: Vec<_> = samples
        .iter()
        .map(|event| event["sample"]["role"].as_str().unwrap())
        .collect();
    assert_eq!(
        roles,
        [
            "baseline",
            "candidate",
            "baseline",
            "candidate",
            "candidate",
            "baseline"
        ]
    );
    assert_eq!(events.last().unwrap()["event"], "complete");
    let journal_before = std::fs::read(directory.path().join("result/journal.jsonl")).unwrap();
    assert!(!execute(directory.path(), &plan).status.success());
    assert_eq!(
        journal_before,
        std::fs::read(directory.path().join("result/journal.jsonl")).unwrap()
    );
}

#[test]
fn failures_remain_observations_and_never_produce_a_report() {
    for failure in ["wrong-exit", "stall", "flood"] {
        let directory = TestDirectory::new();
        let mut plan = plan(directory.path());
        plan["candidate"]["arguments"] = json!([failure]);
        if failure == "stall" {
            plan["timeout_ms"] = json!(5000);
        }
        let result = execute(directory.path(), &plan);
        assert!(!result.status.success(), "{failure}");
        assert!(!directory.path().join("result/report.json").exists());
        let events = events(directory.path());
        assert_eq!(events.last().unwrap()["event"], "failed");
        let reason = events.last().unwrap()["reason"].as_str().unwrap();
        assert!(
            reason.contains(match failure {
                "stall" => "TimedOut",
                "flood" => "Overflow",
                _ => "oracle",
            }),
            "{reason}"
        );
        assert!(
            events
                .iter()
                .any(|event| event["event"] == "sample" && event["sample"]["failure"].is_string())
        );
    }
}

#[test]
fn detects_declared_input_mutation_and_wrong_output() {
    for mutation in [true, false] {
        let directory = TestDirectory::new();
        let mut plan = plan(directory.path());
        if mutation {
            plan["candidate"]["arguments"] = json!(["mutate", directory.path().join("input.txt")]);
        } else {
            plan["measurement"]["expected"]["stdout"] = json!("wrong\n");
        }
        assert!(!execute(directory.path(), &plan).status.success());
        let events = events(directory.path());
        let reason = events.last().unwrap()["reason"].as_str().unwrap();
        assert!(
            reason.contains(if mutation { "changed" } else { "oracle" }),
            "{reason}"
        );
    }
}

#[test]
fn reported_work_checks_the_computation_and_uses_its_explicit_metric() {
    let directory = TestDirectory::new();
    let mut plan = plan(directory.path());
    let work_units = 10000_u64;
    let checksum = (0..work_units).fold(17_u64, |accumulator, value| {
        accumulator.wrapping_add(value.wrapping_mul(value))
    });
    for role in ["baseline", "candidate"] {
        plan[role]["arguments"] = json!(["timed", work_units.to_string(), "17"]);
    }
    plan["measurement"] = json!({"kind":"reported_work", "stdin":"", "work_units":work_units, "checksum":checksum.to_string()});
    let result = execute(directory.path(), &plan);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let report: Value = serde_json::from_slice(
        &std::fs::read(directory.path().join("result/report.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(report["metric"], "reported_work_ns_per_unit");
    assert_eq!(report["comparison"]["pairs"], 2);
    let other = TestDirectory::new();
    plan["measurement"]["checksum"] = json!("wrong");
    assert!(!execute(other.path(), &plan).status.success());
}

#[test]
fn compiler_samples_check_native_output_and_retain_every_stage_table() {
    let directory = TestDirectory::new();
    let mut plan = plan(directory.path());
    for role in ["baseline", "candidate"] {
        plan[role]["arguments"] = json!([]);
    }
    let target = match (std::env::consts::OS, std::env::consts::ARCH) {
        ("windows", "x86_64") => "windows_x86_64",
        ("linux", "x86_64") => "linux_x86_64",
        ("linux", "aarch64") => "linux_arm64",
        ("macos", "aarch64") => "macos_arm64",
        ("macos", "x86_64") => "macos_x86_64",
        _ => return,
    };
    std::fs::create_dir(directory.path().join("library")).unwrap();
    plan["measurement"] = json!({"kind":"compiler", "source":"input.txt", "library_root":"library", "target":target,
        "expected":{"exit_code":0,"stdout":"native ok\n","stderr":""}, "require_identical_artifacts":true});
    let result = execute(directory.path(), &plan);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    for event in events(directory.path())
        .iter()
        .filter(|event| event["event"] == "sample")
    {
        assert_eq!(event["sample"]["compiler_stage_ms"]["Stage fixture"], 1.25);
        assert_eq!(event["sample"]["observations"].as_array().unwrap().len(), 2);
        assert!(event["sample"]["artifact_sha256"].is_string());
    }
}

#[test]
fn compiler_failure_cannot_be_recorded_as_a_fast_compile() {
    let directory = TestDirectory::new();
    let mut plan = plan(directory.path());
    for role in ["baseline", "candidate"] {
        plan[role]["arguments"] = json!([]);
    }
    plan["candidate"]["environment"] = json!({"PROBE_COMPILE_FAIL":"1"});
    let target = if cfg!(windows) {
        "windows_x86_64"
    } else if cfg!(target_os = "macos") {
        if cfg!(target_arch = "aarch64") {
            "macos_arm64"
        } else {
            "macos_x86_64"
        }
    } else if cfg!(target_arch = "aarch64") {
        "linux_arm64"
    } else {
        "linux_x86_64"
    };
    std::fs::create_dir(directory.path().join("library")).unwrap();
    plan["measurement"] = json!({"kind":"compiler", "source":"input.txt", "library_root":"library", "target":target,
        "expected":{"exit_code":0,"stdout":"native ok\n","stderr":""}, "require_identical_artifacts":true});
    assert!(!execute(directory.path(), &plan).status.success());
    let events = events(directory.path());
    assert!(
        events.last().unwrap()["reason"]
            .as_str()
            .unwrap()
            .contains("compile failed")
    );
    assert!(!directory.path().join("result/report.json").exists());
}

#[test]
fn a_missing_oracle_or_unknown_configuration_is_rejected() {
    let directory = TestDirectory::new();
    let mut plan = plan(directory.path());
    plan["measurement"]["expected"]
        .as_object_mut()
        .unwrap()
        .remove("stdout");
    assert!(!execute(directory.path(), &plan).status.success());
    assert!(!directory.path().join("result").exists());
}

#[test]
fn lost_input_still_retains_the_attempted_sample() {
    let directory = TestDirectory::new();
    let mut plan = plan(directory.path());
    plan["candidate"]["arguments"] = json!(["delete", directory.path().join("input.txt")]);
    assert!(!execute(directory.path(), &plan).status.success());
    let events = events(directory.path());
    assert!(events.iter().any(|event| {
        event["event"] == "sample"
            && event["sample"]["role"] == "candidate"
            && event["sample"]["failure"]
                .as_str()
                .is_some_and(|reason| reason.contains("revalidate"))
    }));
}

#[test]
fn unoptimized_producers_and_stale_artifact_paths_are_refused() {
    for (environment, expected_reason) in [
        ("PROBE_UNOPTIMIZED", "optimized compiler"),
        ("PROBE_PUBLISH_OUTSIDE", "fresh build directory"),
    ] {
        let directory = TestDirectory::new();
        let mut plan = plan(directory.path());
        for role in ["baseline", "candidate"] {
            plan[role]["arguments"] = json!([]);
        }
        plan["candidate"]["environment"] = json!({(environment):"1"});
        std::fs::create_dir(directory.path().join("library")).unwrap();
        let target = if cfg!(windows) {
            "windows_x86_64"
        } else if cfg!(target_os = "macos") {
            if cfg!(target_arch = "aarch64") {
                "macos_arm64"
            } else {
                "macos_x86_64"
            }
        } else if cfg!(target_arch = "aarch64") {
            "linux_arm64"
        } else {
            "linux_x86_64"
        };
        plan["measurement"] = json!({"kind":"compiler", "source":"input.txt", "library_root":"library", "target":target,
            "expected":{"exit_code":0,"stdout":"native ok\n","stderr":""}, "require_identical_artifacts":true});
        let result = execute(directory.path(), &plan);
        assert!(!result.status.success());
        assert!(
            events(directory.path()).last().unwrap()["reason"]
                .as_str()
                .unwrap()
                .contains(expected_reason),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
    }
}
