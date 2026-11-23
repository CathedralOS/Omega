//! A `match` may have no subject: `match { a > b -> a, _ -> b }` tests each
//! arm's condition in authored order and takes the first that holds. The
//! parser used to read the `{` as the subject and refuse the arms.
//!
//! `wiki/spec/language/patterns.md`, "Bare names in dispatch arms": "In a
//! dispatch without a subject, an arm condition such as `ready -> 1` reads the
//! existing value of `ready`"; "Coverage and domain tests": "Other
//! value/comparison ladders and predicate-domain guards require a wildcard".
//!
//! The wildcard and proved-coverage fixtures must select the same values in
//! both engines. A gap between two comparisons owes a proof at the last guard.

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
    std::fs::write(
        scratch.path().join("build.omg"),
        include_str!("fixtures/patterns/run/subjectless_match/build.omg"),
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_omega"))
        .args(arguments)
        .args([
            "--build-input",
            "main.omg",
            "--build-input",
            "build.omg",
            "build.omg",
        ])
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
fn the_last_guard_must_cover_the_remaining_inputs() {
    let complete = fixture("run", "subjectless_match_ladder");
    let (output, report) = invoke(&complete, &["run", "--both"]);
    assert!(output.status.success(), "{report}");
    let gap = complete.replace("a <= b -> b", "a < b -> b");
    assert_ne!(gap, complete);
    let (output, report) = invoke(&gap, &["--check"]);
    assert!(!output.status.success(), "{report}");
    assert!(
        report.contains("cannot prove the last guard covers every remaining arrival"),
        "{report}"
    );
    assert!(report.contains("main.omg:3:"), "{report}");
}

#[test]
fn complementary_guards_cover_values_and_transitions() {
    let source = fixture("run", "complementary_guards");
    let (output, report) = invoke(&source, &["run", "--both"]);
    assert!(output.status.success(), "{report}");
    for receipt in ["native exit 0", "interp exit: 0"] {
        assert!(report.lines().any(|line| line == receipt), "{report}");
    }
    for uncovered in [
        source.replace("a != b -> 7", "a != 0 -> 7"),
        source
            .replace("let seen: bool = read_then_clear(@mutable flag);", "")
            .replace("seen ==", "read_then_clear(@mutable flag) ==")
            .replace("seen !=", "read_then_clear(@mutable flag) !="),
    ] {
        assert_ne!(uncovered, source);
        let (output, report) = invoke(&uncovered, &["--check"]);
        assert!(!output.status.success(), "{report}");
        assert!(
            report.contains("cannot prove the last guard covers every remaining arrival"),
            "{report}"
        );
    }
}
