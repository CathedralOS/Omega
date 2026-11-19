//! Generic record construction and forwarding retain each field’s ownership.

mod support;

use platform_custody::temporary_directory::TemporaryDirectory;
use std::process::{Command, Output, Stdio};

fn invoke(tier: &str, arguments: &[&str]) -> (Output, String) {
    let fixture = support::repository_root()
        .join("source-rust/application/tests/fixtures/ownership")
        .join(tier)
        .join("generic_record_fields");
    let scratch = TemporaryDirectory::create("generic-record-fields", false).unwrap();
    for name in ["main.omg", "build.omg"] {
        std::fs::copy(fixture.join(name), scratch.path().join(name)).unwrap();
    }
    let output = Command::new(env!("CARGO_BIN_EXE_omega"))
        .args(arguments)
        .args([
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
fn generic_record_fields_survive_construction_and_calls_in_both_engines() {
    let (output, report) = invoke("run", &["run", "--both"]);
    assert!(output.status.success(), "{report}");
    assert!(output.stdout.is_empty(), "{report}");
    for receipt in ["native exit 0", "interp exit: 0"] {
        assert!(report.lines().any(|line| line == receipt), "{report}");
    }
}

#[test]
fn an_unused_generic_record_cannot_discard_its_unknown_field() {
    let (output, report) = invoke("fail", &["--check"]);
    assert!(!output.status.success(), "{report}");
    assert!(
        report.contains(
            "normal edge cannot implicitly dispose storage without an authorized disposition"
        ),
        "{report}"
    );
}
