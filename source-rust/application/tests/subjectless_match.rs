//! A `match` may have no subject: `match { a > b -> a, _ -> b }` tests each
//! arm's condition in authored order and takes the first that holds. The
//! parser used to read the `{` as the subject and refuse the arms.
//!
//! `wiki/spec/language/patterns.md`, "Bare names in dispatch arms": "In a
//! dispatch without a subject, an arm condition such as `ready -> 1` reads the
//! existing value of `ready`"; "Coverage and domain tests": "Other
//! value/comparison ladders and predicate-domain guards require a wildcard".
//!
//! The program puts a subjectless match in a machine's result, beside a
//! block-valued arm, in a call argument and as a returned record, and crashes
//! unless each returns its expected value. The twin replaces the wildcard of
//! `larger` with `a <= b`: the two comparisons do cover every pair, but a
//! comparison ladder is closed only by `_`, so the checker refuses it at the
//! `match`. The stage handoffs are tested in the `subjectless_match` tests of
//! `pipeline/03_symbol-resolved-trees-to-typed-trees`,
//! `pipeline/05_prepared-typed-trees-to-lowered-program` and
//! `pipeline/06_lowered-program-to-checked-program`.

mod support;

use platform_custody::temporary_directory::TemporaryDirectory;
use std::process::{Command, Output, Stdio};

fn fixture(tier: &str, case: &str) -> String {
    let path = support::repository_root()
        .join("source-rust/application/tests/fixtures/patterns")
        .join(tier)
        .join(case)
        .join("main.omg");
    std::fs::read_to_string(path).unwrap()
}

/// Run `omega` on a copy of one source file, so nothing the compiler writes
/// lands in the checkout.
fn invoke(source: &str, arguments: &[&str]) -> (Output, String) {
    let scratch = TemporaryDirectory::create("subjectless-match", false).unwrap();
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
fn a_match_without_a_subject_runs_natively_and_in_psi() {
    let (output, report) = invoke(&fixture("run", "subjectless_match"), &["run", "--both"]);
    assert_eq!(output.status.code(), Some(0), "{report}");
    for receipt in ["native exit 0", "interp exit: 0"] {
        assert!(report.lines().any(|line| line == receipt), "{report}");
    }
}

#[test]
fn a_comparison_ladder_without_a_wildcard_is_refused() {
    let (from, to) = ("a > b -> a, _ -> b }", "a > b -> a, a <= b -> b }");
    let run = fixture("run", "subjectless_match");
    assert_eq!(run.matches(from).count(), 1, "the change has one site");
    let twin = fixture("fail", "subjectless_match_ladder");
    assert_eq!(run.replace(from, to), twin);
    let (output, report) = invoke(&twin, &["--check"]);
    assert!(!output.status.success(), "{report}");
    for expected in [
        "this match has no `_` arm: when every condition is false no arm applies, so add a `_ ->` arm",
        "main.omg:3:5",
    ] {
        assert!(report.contains(expected), "{report}");
    }
}
