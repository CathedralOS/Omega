//! An array's length can name a constant that a call computes:
//! `const N: u64 = extent();` and `let values: [u8; N];`. The compiler has to
//! run `extent` before it knows how long the array is, and that length then
//! has to reach the checker, the Psi interpreter and native code unchanged.
//!
//! The program stores two bytes in the array and passes it to a machine that
//! requires `@[u8; 2]`. It exits 0 only when the bytes read back as stored.
//! Its twin differs in one line: `extent` returns 3, so the same call passes
//! three bytes where two are required and the program is refused.
//!
//! `wiki/spec/language/constants.md`, "Evaluation": "A constant position
//! requests semantic evaluation." The stages this length crosses are tested
//! one at a time in the source compiler's
//! `pipeline/04_typed-trees-to-prepared-typed-trees/tests/computed_array_extents.rs`
//! and `pipeline/05_prepared-typed-trees-to-lowered-program/tests/prepared_array_extents.rs`.

mod support;

use platform_custody::temporary_directory::TemporaryDirectory;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};

fn fixture(tier: &str) -> PathBuf {
    support::repository_root()
        .join("source-rust/application/tests/fixtures/constants")
        .join(tier)
        .join("computed_array_extent")
}

/// Run `omega` on a copy of the fixture's one source file, so nothing the
/// compiler writes lands in the checkout.
fn invoke(source: &str, arguments: &[&str]) -> (Output, String) {
    let scratch = TemporaryDirectory::create("computed-array-extent", false).unwrap();
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
fn an_array_sized_by_a_computed_constant_runs_natively_and_in_psi() {
    let source = std::fs::read_to_string(fixture("run").join("main.omg")).unwrap();
    let (output, report) = invoke(&source, &["run", "--both"]);
    assert_eq!(output.status.code(), Some(0), "{report}");
    for receipt in ["native exit 0", "interp exit: 0"] {
        assert!(report.lines().any(|line| line == receipt), "{report}");
    }
}

#[test]
fn a_computed_extent_of_three_is_refused_where_two_bytes_are_required() {
    // The twins differ only in what `extent` returns.
    let read = |tier| std::fs::read_to_string(fixture(tier).join("main.omg")).unwrap();
    assert_eq!(read("run").replace("{ 2u64 }", "{ 3u64 }"), read("fail"));

    let (output, report) = invoke(&read("fail"), &["--check"]);
    assert!(!output.status.success(), "{report}");
    let expected = std::fs::read_to_string(fixture("fail").join("expected.txt")).unwrap();
    assert!(report.contains(expected.trim_end()), "{report}");
    // The refusal points at the argument: `@values` in `second(@values)`.
    assert!(report.contains("main.omg:12:27"), "{report}");
    let source = read("fail");
    let line = source.lines().nth(11).unwrap();
    assert_eq!(&line[26..], "@values);");
}

#[test]
fn a_literal_initializer_supplies_its_declared_array_length() {
    let source = std::fs::read_to_string(fixture("run").join("main.omg")).unwrap();
    let (before, after) = source.split_once("const N: u64 = extent();").unwrap();
    let literal = |n| format!("{before}const N: u64 = {n}u64;{after}");
    let (output, report) = invoke(&literal(2), &["run", "--both"]);
    assert_eq!(output.status.code(), Some(0), "{report}");
    assert!(
        report.contains("native exit 0") && report.contains("interp exit: 0"),
        "{report}"
    );
    let (output, report) = invoke(&literal(3), &["--check"]);
    assert!(!output.status.success(), "{report}");
    let expected = std::fs::read_to_string(fixture("fail").join("expected.txt")).unwrap();
    assert!(report.contains(expected.trim_end()), "{report}");
}
