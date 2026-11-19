//! Specialization preserves the loans carried by a type argument.

mod support;

use platform_custody::temporary_directory::TemporaryDirectory;
use std::process::{Command, Output, Stdio};

fn invoke(tier: &str) -> (Output, String) {
    let fixture = support::repository_root()
        .join("tests/fixtures")
        .join(tier)
        .join("calls/copied_borrowed_types");
    let scratch = TemporaryDirectory::create("copied-borrowed-types", false).unwrap();
    for name in ["main.omg", "build.omg"] {
        std::fs::copy(fixture.join(name), scratch.path().join(name)).unwrap();
    }
    let output = Command::new(env!("CARGO_BIN_EXE_omega"))
        .args([
            "run",
            "--both",
            "--build-input",
            "build.omg",
            "--build-input",
            "main.omg",
            "main.omg",
        ])
        .current_dir(scratch.path())
        .stdin(Stdio::null())
        .output()
        .expect("the compiler runs with the fixture's explicit inputs");
    let report = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    eprintln!("{report}");
    (output, report)
}

#[test]
fn a_borrowed_type_argument_runs_in_both_engines() {
    let (output, report) = invoke("run");
    assert!(output.status.success(), "{report}");
    assert!(output.stdout.is_empty(), "{report}");
    for receipt in ["native exit 0", "interp exit: 0"] {
        assert!(report.lines().any(|line| line == receipt), "{report}");
    }
}

#[test]
fn a_forwarded_loan_still_excludes_overlapping_writes() {
    let (output, report) = invoke("fail");
    assert!(!output.status.success(), "{report}");
    assert!(
        report.contains("storage is changed while a conflicting reference remains live")
            && !report.contains("not supported yet"),
        "{report}"
    );
}
