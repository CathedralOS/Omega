//! Specialization preserves the loans carried by a type argument.
//!
//! A type argument names borrow-carrying data with its lifetime, as in
//! `forward::<Loan<'lender>>` inside a machine that declares `'lender`.
//! lifetimes.md, "Binders and source applications": named data applications
//! "must match the declaration's lifetime arity", so a bare `forward::<Loan>`
//! is refused for that rule.

mod support;

use platform_custody::temporary_directory::TemporaryDirectory;
use std::process::{Command, Output, Stdio};

fn invoke(tier: &str, case: &str) -> (Output, String) {
    let fixture = support::repository_root()
        .join("source-rust/application/tests/fixtures/calls")
        .join(tier)
        .join(case);
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
fn a_borrowed_type_argument_runs_in_both_engines() {
    let (output, report) = invoke("run", "copied_borrowed_types");
    assert!(output.status.success(), "{report}");
    assert!(output.stdout.is_empty(), "{report}");
    for receipt in ["native exit 0", "interp exit: 0"] {
        assert!(report.lines().any(|line| line == receipt), "{report}");
    }
}

#[test]
fn a_forwarded_loan_still_excludes_overlapping_writes() {
    let (output, report) = invoke("fail", "copied_borrowed_types");
    assert!(!output.status.success(), "{report}");
    assert!(
        report.contains("storage is changed while a conflicting reference remains live")
            && !report.contains("not supported yet"),
        "{report}"
    );
}

#[test]
fn a_type_argument_without_its_lifetime_arguments_is_refused() {
    let (output, report) = invoke("fail", "bare_borrowed_type_argument");
    assert!(!output.status.success(), "{report}");
    assert!(
        report.contains(
            "type argument `Loan` supplies no lifetime arguments, but `Loan` takes 1 lifetime \
             argument; write `Loan<'lender>` with a lifetime binder declared by the enclosing \
             machine"
        ) && !report.contains("not supported yet"),
        "{report}"
    );
}
