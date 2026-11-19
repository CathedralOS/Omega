//! Generic wrappers retain type, const and selected machine arguments.

mod support;

use platform_custody::temporary_directory::TemporaryDirectory;
use std::process::{Command, Output, Stdio};

fn invoke(tier: &str, sample: &str) -> (Output, String) {
    let fixture = support::repository_root()
        .join("source-rust/application/tests/fixtures/calls")
        .join(tier)
        .join(sample);
    let scratch = TemporaryDirectory::create("mixed-static-arguments", false).unwrap();
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
fn mixed_static_arguments_run_in_both_engines() {
    let (output, report) = invoke("run", "mixed_static_arguments");
    assert!(output.status.success(), "{report}");
    assert!(output.stdout.is_empty(), "{report}");
    for receipt in ["native exit 0", "interp exit: 0"] {
        assert!(report.lines().any(|line| line == receipt), "{report}");
    }
}

#[test]
fn a_const_argument_still_owes_its_declared_requirement() {
    let (output, report) = invoke("fail", "mixed_static_arguments");
    assert!(!output.status.success(), "{report}");
    assert!(
        report.contains("cannot prove requires contract for call keep from forward")
            && !report.contains("not supported yet"),
        "{report}"
    );
}

#[test]
fn nominal_callback_parameters_keep_their_distinct_selections() {
    let (output, report) = invoke("run", "nominal_static_arguments");
    assert!(output.status.success(), "{report}");
    for receipt in ["native exit 0", "interp exit: 0"] {
        assert!(report.lines().any(|line| line == receipt), "{report}");
    }
}

#[test]
fn a_matching_signature_does_not_bind_a_callback_to_the_requirement() {
    let (output, report) = invoke("fail", "nominal_static_arguments");
    assert!(!output.status.success(), "{report}");
    assert!(
        report.contains("callback `same_shape` must satisfy the exact requirement of `Right`"),
        "{report}"
    );
}
