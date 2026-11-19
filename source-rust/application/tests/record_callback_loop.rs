//! Copied loop bodies retain the exact selected operation.

mod support;

use platform_custody::temporary_directory::TemporaryDirectory;
use std::process::{Command, Output, Stdio};

fn invoke(tier: &str) -> (Output, String) {
    let fixture = support::repository_root()
        .join("tests/fixtures/calls")
        .join(tier)
        .join("record_callback_loop");
    let scratch = TemporaryDirectory::create("record-callback-loop", false).unwrap();
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
fn record_callback_loop_run_in_both_engines() {
    let (output, report) = invoke("run");
    assert!(output.status.success(), "{report}");
    assert!(output.stdout.is_empty(), "{report}");
    for receipt in ["native exit 0", "interp exit: 0"] {
        assert!(report.lines().any(|line| line == receipt), "{report}");
    }
}

#[test]
fn a_selected_operation_keeps_its_required_borrow_access() {
    let (output, report) = invoke("fail");
    assert!(!output.status.success(), "{report}");
    assert!(
        report.contains("member signature differs from its requirement"),
        "{report}"
    );
}
