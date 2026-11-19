//! Selecting a callback keeps the wrapper's lifetime and entry contract.

mod support;

use platform_custody::temporary_directory::TemporaryDirectory;
use std::process::{Command, Output, Stdio};

fn invoke(tier: &str) -> (Output, String) {
    let fixture = support::repository_root()
        .join("tests/fixtures")
        .join(tier)
        .join("calls/lifetime_callback_contracts");
    let scratch = TemporaryDirectory::create("lifetime-callback-contracts", false).unwrap();
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
fn lifetime_callback_contracts_run_in_both_engines() {
    let (output, report) = invoke("run");
    assert!(output.status.success(), "{report}");
    assert!(output.stdout.is_empty(), "{report}");
    for receipt in ["native exit 0", "interp exit: 0"] {
        assert!(report.lines().any(|line| line == receipt), "{report}");
    }
}

#[test]
fn a_callback_does_not_supply_a_missing_scalar_requirement() {
    let (output, report) = invoke("fail");
    assert!(!output.status.success(), "{report}");
    assert!(
        report.contains("cannot prove requires contract for call checked_remaining from forward"),
        "{report}"
    );
}
