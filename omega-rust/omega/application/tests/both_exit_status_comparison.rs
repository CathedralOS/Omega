//! `omega run --both` exits with the program's status only when it saw the
//! native and Psi runs finish and agree. When the Psi run stops early, here
//! because its work budget runs out, nothing was compared, so the run reports
//! `DECLINED` and exits 204 even though the native program exited 0. The twin
//! runs the same program with the default budget and agrees.
//!
//! The other declined path, a Psi exit status a Unix host shows only as its
//! low byte, is pinned by the unit tests beside `compare_exit_status`.
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

fn project() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../tests/fixtures/run/cli/both_comparison_verdict")
}

fn run_both(extra: &[&str]) -> (Output, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_omega"))
        .args(["run", "--both"])
        .args(extra)
        .arg("main.omg")
        .current_dir(project())
        .stdin(Stdio::null())
        .output()
        .expect("omega runs on the hosted target");
    let report = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    (output, report)
}

#[test]
fn both_runs_finish_and_agree() {
    let (output, report) = run_both(&[]);
    assert_eq!(output.status.code(), Some(0), "{report}");
    assert!(
        report.lines().any(|line| line == "native exit 0"),
        "{report}"
    );
    assert!(
        report.lines().any(|line| line == "interp exit: 0"),
        "{report}"
    );
}

#[test]
fn an_unfinished_psi_run_is_declined_not_agreed() {
    let (output, report) = run_both(&["--psi-work-budget", "1"]);
    assert_eq!(output.status.code(), Some(204), "{report}");
    assert!(
        report.lines().any(|line| line == "native exit 0"),
        "{report}"
    );
    assert!(
        report.contains("interp: DECLINED (Psi reference execution: Work budget exhausted"),
        "{report}"
    );
    assert!(!report.contains("interp exit:"), "{report}");
}
