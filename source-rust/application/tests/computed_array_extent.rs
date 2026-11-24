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

#[path = "support/program.rs"]
mod program;
use program::invoke;
use std::path::PathBuf;

fn fixture(tier: &str) -> PathBuf {
    support::repository_root()
        .join("source-rust/application/tests/fixtures/constants")
        .join(tier)
        .join("computed_array_extent")
}

#[test]
fn an_array_sized_by_a_computed_constant_runs_natively_and_in_psi() {
    let source = std::fs::read_to_string(fixture("run").join("main.omg")).unwrap();
    program::assert_silent_return(&source);
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
    program::assert_silent_return(&literal(2));
    let (output, report) = invoke(&literal(3), &["--check"]);
    assert!(!output.status.success(), "{report}");
    let expected = std::fs::read_to_string(fixture("fail").join("expected.txt")).unwrap();
    assert!(report.contains(expected.trim_end()), "{report}");
}
