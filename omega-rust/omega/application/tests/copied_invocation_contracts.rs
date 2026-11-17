//! Copying a generic helper preserves its authored invocation ceiling.

use platform_custody::temporary_directory::TemporaryDirectory;
use std::path::Path;
use std::process::{Command, Output, Stdio};

fn invoke(tier: &str) -> (Output, String) {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../tests/fixtures")
        .join(tier)
        .join("calls/copied_invocation_contracts");
    let scratch = TemporaryDirectory::create("copied-invocation-contracts", false).unwrap();
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
fn copied_invocation_contracts_run_in_both_engines() {
    let (output, report) = invoke("run");
    assert!(output.status.success(), "{report}");
    assert!(output.stdout.is_empty(), "{report}");
    for receipt in ["native exit 0", "interp exit: 0"] {
        assert!(report.lines().any(|line| line == receipt), "{report}");
    }
}

#[test]
fn a_copied_helper_cannot_enter_outside_its_written_ceiling() {
    let (output, report) = invoke("fail");
    assert!(!output.status.success(), "{report}");
    assert!(
        report.contains("machine `relay` invokes")
            && report.contains("(synchronous invocation)")
            && !report.contains("not supported yet"),
        "{report}"
    );
}
