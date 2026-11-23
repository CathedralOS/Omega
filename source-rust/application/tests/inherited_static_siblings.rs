//! Sibling helpers retain their enclosing static arguments.

mod support;

use platform_custody::temporary_directory::TemporaryDirectory;
use std::process::{Command, Output, Stdio};

fn invoke(tier: &str, fixture_name: &str) -> (Output, String) {
    let fixture = support::repository_root()
        .join("source-rust/application/tests/fixtures/calls")
        .join(tier)
        .join(fixture_name);
    let scratch = TemporaryDirectory::create(fixture_name, false).unwrap();
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
            "build.omg",
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
fn inherited_static_siblings_run_in_both_engines() {
    assert_runs_in_both_engines("inherited_static_siblings");
}

#[test]
fn selected_trait_defaults_keep_their_lexical_helpers() {
    assert_runs_in_both_engines("trait_default_helpers");
}

#[test]
fn selected_maps_pass_their_own_private_callback() {
    assert_runs_in_both_engines("private_map_callback");
}

fn assert_runs_in_both_engines(fixture: &str) {
    let (output, report) = invoke("run", fixture);
    assert!(output.status.success(), "{report}");
    assert!(output.stdout.is_empty(), "{report}");
    for receipt in ["native exit 0", "interp exit: 0"] {
        assert!(report.lines().any(|line| line == receipt), "{report}");
    }
}

#[test]
fn a_const_argument_still_owes_its_declared_requirement() {
    let (output, report) = invoke("fail", "inherited_static_siblings");
    assert!(!output.status.success(), "{report}");
    assert!(
        report.contains("cannot prove requires contract for call forward from main")
            && !report.contains("not supported yet"),
        "{report}"
    );
}

#[test]
fn default_helpers_cannot_capture_an_enclosing_value() {
    let (output, report) = invoke("fail", "trait_default_helpers");
    assert!(!output.status.success(), "{report}");
    assert!(
        report.contains("cannot capture enclosing runtime binding `value`"),
        "{report}"
    );
}

#[test]
fn a_private_map_callback_must_accept_every_input_its_binder_allows() {
    let (output, report) = invoke("fail", "private_map_callback");
    assert!(!output.status.success(), "{report}");
    assert!(
        report
            .matches("cannot prove requires contract for call local from invoke: false")
            .count()
            == 1
            && report
                .matches("cannot prove the input precondition of `local`")
                .count()
                == 1
            && !report.contains("defect of the checker")
            && !report.contains("not supported yet"),
        "{report}"
    );
}
