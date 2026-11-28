//! A Psi work limit can stop an invocation before it returns.

mod support;

use std::process::{Command, Output, Stdio};

fn run(extra: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_omega"))
        .args(["run"])
        .args(extra)
        .arg("main.omg")
        .current_dir(
            support::repository_root()
                .join("source-rust/application/tests/fixtures/cli/run/psi_work_budget"),
        )
        .stdin(Stdio::null())
        .output()
        .unwrap()
}

#[test]
fn a_finished_program_reports_its_return() {
    let output = run(&[]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{stderr}");
    assert!(stderr.lines().any(|line| line == "Psi exit 0"), "{stderr}");
}

#[test]
fn an_exhausted_work_budget_reports_an_unfinished_execution() {
    let output = run(&["--psi-work-budget", "1"]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(200), "{stderr}");
    assert!(stderr.contains("raise --psi-work-budget"), "{stderr}");
    assert!(!stderr.lines().any(|line| line == "Psi exit 0"), "{stderr}");
}
