//! An arm of a `transition` without a subject is one guard, and the guard may
//! start with a parenthesis: `(a + b) > a + 3 -> done()`.
//!
//! The parser used to read the leading `(a + b)` as a tuple pattern and then
//! refuse the rest ("expected transition pattern"). The program takes the
//! guarded arm only when `7 > 6`, and crashes in the other arm, so it exits 0
//! only when the whole comparison was the guard. Its twin writes the
//! parenthesized list `(a + b, a)`, a tuple pattern, which a transition
//! without a subject cannot hold.
//!
//! `wiki/spec/language/patterns.md#bare-names-in-dispatch-arms`: "In a
//! dispatch without a subject, an arm condition such as `ready -> 1` reads the
//! existing value of `ready`." The parser's own test is
//! `pipeline/01_tokens-to-syntax-trees/tests/unit/subjectless_guards.rs`.

mod support;

use platform_custody::temporary_directory::TemporaryDirectory;
use std::process::{Command, Output, Stdio};

fn fixture(tier: &str) -> String {
    let path = support::repository_root()
        .join("source-rust/application/tests/fixtures/transitions")
        .join(tier)
        .join("parenthesized_guard/main.omg");
    std::fs::read_to_string(path).unwrap()
}

/// Run `omega` on a copy of one source file, so nothing the compiler writes
/// lands in the checkout.
fn invoke(source: &str, arguments: &[&str]) -> (Output, String) {
    let scratch = TemporaryDirectory::create("parenthesized-guard", false).unwrap();
    std::fs::write(scratch.path().join("main.omg"), source).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_omega"))
        .args(arguments)
        .arg("main.omg")
        .current_dir(scratch.path())
        .stdin(Stdio::null())
        .output()
        .expect("the compiler runs on the fixture");
    let report = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    eprintln!("{report}");
    (output, report)
}

#[test]
fn a_guard_that_starts_with_a_parenthesis_runs_natively_and_in_psi() {
    let (output, report) = invoke(&fixture("run"), &["run", "--both"]);
    assert_eq!(output.status.code(), Some(0), "{report}");
    for receipt in ["native exit 0", "interp exit: 0"] {
        assert!(report.lines().any(|line| line == receipt), "{report}");
    }
}

#[test]
fn a_parenthesized_list_in_a_transition_without_a_subject_is_refused() {
    // The twins differ only in the first arm's condition.
    assert_eq!(
        fixture("run").replace("(a + b) > a + 3 -> done(),", "(a + b, a) -> done(),"),
        fixture("fail")
    );
    let (output, report) = invoke(&fixture("fail"), &["--check"]);
    assert!(!output.status.success(), "{report}");
    assert!(
        report.contains("main.omg:5:9: anonymous transition blocks do not support tuple patterns"),
        "{report}"
    );
}
