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
    let validated = Command::new(env!("CARGO_BIN_EXE_omega-bench"))
        .arg("validate")
        .arg(directory.path().join("result"))
        .output()
        .unwrap();
    assert!(
        validated.status.success(),
        "{}",
        String::from_utf8_lossy(&validated.stderr)
    );
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
    let validation = Command::new(env!("CARGO_BIN_EXE_omega-bench"))
        .arg("validate")
        .arg(directory.path().join("result"))
        .output()
        .unwrap();
    assert!(
        validation.status.success(),
        "{}",
        String::from_utf8_lossy(&validation.stderr)
    );
    // Different producers need their own accepted project locks. Their copied
    // entries stay byte-identical and native evidence must name the chosen copy.
    std::fs::copy(
        directory.path().join("input.txt"),
        directory.path().join("candidate.txt"),
    )
    .unwrap();
    plan["measurement"]["candidate_source"] = json!("candidate.txt");
    plan["inputs"]
        .as_array_mut()
        .unwrap()
        .push(json!("candidate.txt"));
    let copies = TestDirectory::new();
    let comparison = execute(copies.path(), &plan);
    assert!(
        comparison.status.success(),
        "{}",
        String::from_utf8_lossy(&comparison.stderr)
    );
    let validation = Command::new(env!("CARGO_BIN_EXE_omega-bench"))
        .arg("validate")
        .arg(copies.path().join("result"))
        .output()
        .unwrap();
    assert!(
        validation.status.success(),
        "{}",
        String::from_utf8_lossy(&validation.stderr)
    );
    std::fs::write(directory.path().join("candidate.txt"), "different source").unwrap();
    let mismatched = TestDirectory::new();
    let refusal = execute(mismatched.path(), &plan);
    assert!(!refusal.status.success());
    assert!(String::from_utf8_lossy(&refusal.stderr).contains("identical entry bytes"));
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

#[test]
fn kernel_resource_observations_recover_touched_memory_and_cpu_work() {
    let directory = TestDirectory::new();
    let mut plan = plan(directory.path());
    for role in ["baseline", "candidate"] {
        plan[role]["arguments"] = json!(["resources"]);
    }
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
        let resources = &event["sample"]["observations"][0]["resources"];
        assert!(
            resources["peak_resident_bytes"]["value"]
                .as_u64()
                .unwrap_or(0)
                >= 32 * 1024 * 1024,
            "{resources}"
        );
        assert!(
            resources["user_cpu_ns"]["value"].as_u64().unwrap_or(0)
                + resources["system_cpu_ns"]["value"].as_u64().unwrap_or(0)
                > 0,
            "{resources}"
        );
        if cfg!(windows) {
            assert!(resources["cpu_cycles"]["value"].as_u64().unwrap_or(0) > 0);
        }
    }
}

#[test]
fn validation_detects_changed_streams_and_recomputed_fake_summaries() {
    use sha2::{Digest, Sha256};
    let directory = TestDirectory::new();
    let plan = plan(directory.path());
    assert!(execute(directory.path(), &plan).status.success());
    let validate = || {
        Command::new(env!("CARGO_BIN_EXE_omega-bench"))
            .arg("validate")
            .arg(directory.path().join("result"))
            .output()
            .unwrap()
    };
    let capture = directory
        .path()
        .join("result/pair-0000-baseline/subject.stdout");
    let original = std::fs::read(&capture).unwrap();
    std::fs::write(&capture, b"changed").unwrap();
    assert!(!validate().status.success());
    std::fs::write(&capture, original).unwrap();
    assert!(validate().status.success());
    let report_path = directory.path().join("result/report.json");
    let mut report: Value = serde_json::from_slice(&std::fs::read(&report_path).unwrap()).unwrap();
    report["comparison"]["baseline"]["median"] = json!(0.0);
    let report = serde_json::to_vec(&report).unwrap();
    std::fs::write(report_path, &report).unwrap();
    let mut events = events(directory.path());
    events.last_mut().unwrap()["report_sha256"] = json!(format!("{:x}", Sha256::digest(&report)));
    std::fs::write(
        directory.path().join("result/journal.jsonl"),
        events
            .iter()
            .map(|event| serde_json::to_string(event).unwrap() + "\n")
            .collect::<String>(),
    )
    .unwrap();
    let result = validate();
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("summary disagrees"));
}

#[test]
fn suite_preserves_successes_when_another_workload_fails() {
    let directory = TestDirectory::new();
    let good = plan(directory.path());
    let mut bad = good.clone();
    bad["measurement"]["expected"]["stdout"] = json!("incorrect");
    for (name, plan) in [("good.json", good), ("bad.json", bad)] {
        std::fs::write(
            directory.path().join(name),
            serde_json::to_vec(&plan).unwrap(),
        )
        .unwrap();
    }
    let suite = directory.path().join("suite.json");
    std::fs::write(
        &suite,
        serde_json::to_vec(
            &json!({"schema":"omega-performance-suite/1","plans":["good.json","bad.json"]}),
        )
        .unwrap(),
    )
    .unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_omega-bench"))
        .arg("suite")
        .arg(suite)
        .arg(directory.path().join("suite-result"))
        .output()
        .unwrap();
    assert!(!result.status.success());
    let outcomes: Value = serde_json::from_slice(
        &std::fs::read(directory.path().join("suite-result/suite.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(outcomes["outcomes"][0]["status"], "complete");
    assert_eq!(outcomes["outcomes"][1]["status"], "failed");
}

#[test]
fn reference_workloads_agree_with_independent_oracles() {
    for (kind, units, checksum) in [
        ("compute", "1000000", "8724022130139432207"),
        ("memory", "1000000", "2833458506033"),
        ("calls", "1000000", "251382716753"),
        ("allocations", "100000", "14442337"),
    ] {
        let result = Command::new(env!("CARGO_BIN_EXE_omega-bench-workload"))
            .args([kind, units, "17"])
            .output()
            .unwrap();
        assert!(result.status.success());
        let report: Value = serde_json::from_slice(&result.stdout).unwrap();
        assert_eq!(report["checksum"], checksum, "{kind}");
        assert_eq!(
            report["work_units"].as_u64().unwrap(),
            units.parse::<u64>().unwrap()
        );
        assert!(report["elapsed_ns"].as_u64().unwrap() > 0);
    }
}

#[cfg(feature = "allocation-observation")]
#[test]
fn allocation_profile_preserves_correctness_and_real_counters() {
    let directory = TestDirectory::new();
    let mut plan = plan(directory.path());
    for role in ["baseline", "candidate"] {
        plan[role] = json!({"program":env!("CARGO_BIN_EXE_omega-bench-workload"),"arguments":["allocations","100000","17"]});
    }
    plan["measurement"] =
        json!({"kind":"reported_work","stdin":"","work_units":100000,"checksum":"14442337"});
    let path = directory.path().join("profile-plan.json");
    std::fs::write(&path, serde_json::to_vec(&plan).unwrap()).unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_omega-bench"))
        .arg("profile")
        .arg(path)
        .args(["candidate", "allocations"])
        .arg(directory.path().join("profile"))
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let profile: Value = serde_json::from_slice(
        &std::fs::read(directory.path().join("profile/profile.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(profile["status"], "complete");
    assert!(profile["allocations"]["allocations"].as_u64().unwrap() >= 100000);
    assert!(profile["allocations"]["requested_bytes"].as_u64().unwrap() > 6_400_000);
}

#[test]
fn historical_comparisons_refuse_a_changed_workload_and_never_pair_separate_runs() {
    let first = TestDirectory::new();
    let second = TestDirectory::new();
    let third = TestDirectory::new();
    let plan = plan(first.path());
    assert!(execute(first.path(), &plan).status.success());
    assert!(execute(second.path(), &plan).status.success());
    let history = |after: &Path| {
        Command::new(env!("CARGO_BIN_EXE_omega-bench"))
            .arg("history")
            .arg(first.path().join("result"))
            .arg(after.join("result"))
            .output()
            .unwrap()
    };
    let result = history(second.path());
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let comparison: Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(comparison["comparison_kind"], "unpaired_independent_runs");
    std::fs::write(first.path().join("input.txt"), "different workload bytes").unwrap();
    assert!(execute(third.path(), &plan).status.success());
    let changed = history(third.path());
    assert!(!changed.status.success());
    assert!(String::from_utf8_lossy(&changed.stderr).contains("input bytes changed"));
    let check = Command::new(env!("CARGO_BIN_EXE_omega-bench"))
        .arg("check")
        .arg(first.path().join("result"))
        .arg("5")
        .output()
        .unwrap();
    assert_eq!(check.status.code(), Some(2));
}

#[test]
fn regression_gate_refuses_nonfinite_budgets() {
    // The unit-level statistics control is exercised through complete retained
    // histories above; here the CLI must also reject nonfinite gate budgets.
    let directory = TestDirectory::new();
    let plan = plan(directory.path());
    assert!(execute(directory.path(), &plan).status.success());
    let invalid = Command::new(env!("CARGO_BIN_EXE_omega-bench"))
        .arg("check")
        .arg(directory.path().join("result"))
        .arg("NaN")
        .output()
        .unwrap();
    assert_eq!(invalid.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&invalid.stderr).contains("finite"));
}

#[test]
fn malformed_profile_retains_the_checked_sample_and_failure_reason() {
    let directory = TestDirectory::new();
    let mut plan = plan(directory.path());
    plan["candidate"]["environment"] = json!({"PROBE_BAD_ALLOCATION":"1"});
    let path = directory.path().join("profile-plan.json");
    std::fs::write(&path, serde_json::to_vec(&plan).unwrap()).unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_omega-bench"))
        .arg("profile")
        .arg(path)
        .args(["candidate", "allocations"])
        .arg(directory.path().join("profile"))
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(2));
    let profile: Value = serde_json::from_slice(
        &std::fs::read(directory.path().join("profile/profile.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(profile["status"], "failed");
    assert!(profile["reason"].is_string());
    assert!(
        !profile["sample"]["observations"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn validation_binds_native_execution_to_the_published_compiler_artifact() {
    let directory = TestDirectory::new();
    let mut plan = plan(directory.path());
    for role in ["baseline", "candidate"] {
        plan[role]["arguments"] = json!([]);
    }
    plan["measurement"] = json!({"kind":"compiler","source":"input.txt","library_root":directory.path().join("library"),"target":if cfg!(windows){"windows_x86_64"}else if cfg!(target_os="macos"){if cfg!(target_arch="aarch64"){"macos_arm64"}else{"macos_x86_64"}}else if cfg!(target_arch="aarch64"){"linux_arm64"}else{"linux_x86_64"},"expected":{"exit_code":0,"stdout":"native ok\n","stderr":""},"require_identical_artifacts":true});
    std::fs::create_dir(directory.path().join("library")).unwrap();
    assert!(execute(directory.path(), &plan).status.success());
    let mut journal = events(directory.path());
    let sample = journal
        .iter_mut()
        .find(|event| event["event"] == "sample")
        .unwrap();
    sample["sample"]["observations"][1]["program"] = json!("unrelated-program");
    std::fs::write(
        directory.path().join("result/journal.jsonl"),
        journal
            .iter()
            .map(|event| serde_json::to_string(event).unwrap() + "\n")
            .collect::<String>(),
    )
    .unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_omega-bench"))
        .arg("validate")
        .arg(directory.path().join("result"))
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("native execution"));
}
