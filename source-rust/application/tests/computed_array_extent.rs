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

fn target_const_array_program(literal: &str, last_index: u64) -> String {
    format!(
        r#"const SIZE: u64 = size_of::<u64>();

machine sized(a: [u8; SIZE]) -> [u8; SIZE] {{ a }}

machine read_at(a: @[u8; SIZE], i: u64) -> u8 requires i < SIZE {{
    a[i]
}}

machine make_literal() -> [u8; SIZE] {{
    {literal}
}}

machine main() crashes Trap {{
    let zeroed: [u8; SIZE];
    let items: [u8; SIZE] = sized(make_literal());
    transition zeroed.length == SIZE && 0u64 < SIZE && {last_index}u64 < SIZE {{
        true -> check(@items),
        false -> failed()
    }}
    machine check(items: @[u8; SIZE]) crashes Trap
        requires 0u64 < SIZE && {last_index}u64 < SIZE
    {{
        transition read_at(items, 0u64) == 10u8 && read_at(items, {last_index}u64) == 80u8 {{
            true -> done(),
            false -> failed()
        }}
    }}
    machine done() {{}}
    machine failed() crashes Trap {{ crash Trap; }}
}}
"#
    )
}

#[test]
fn target_layout_const_array_extent_checks_and_runs_on_native_and_psi() {
    let source = target_const_array_program("[10u8, 20u8, 30u8, 40u8, 50u8, 60u8, 70u8, 80u8]", 7);
    let (check_output, check_report) = invoke(&source, &["--check"]);
    assert!(check_output.status.success(), "{check_report}");
    program::assert_silent_return(&source);
}

#[test]
fn target_layout_const_array_extent_refuses_unproved_index() {
    let source = r#"const SIZE: u64 = size_of::<u64>();

machine read_at(a: @[u8; SIZE], i: u64) -> u8 {
    a[i]
}

machine main() {
    let zeroed: [u8; SIZE];
    let _: u8 = read_at(@zeroed, 0u64);
}
"#;
    let (output, report) = invoke(source, &["--check"]);
    assert!(!output.status.success(), "{report}");
    assert!(
        report.contains("index") || report.contains("bounds") || report.contains("cannot prove"),
        "expected index-bound refusal:\n{report}"
    );
}

#[test]
fn target_layout_const_array_extent_refuses_out_of_bounds_index_at_closure() {
    let source = target_const_array_program("[10u8, 20u8, 30u8, 40u8, 50u8, 60u8, 70u8, 80u8]", 8);
    let (check_output, check_report) = invoke(&source, &["--check"]);
    assert!(!check_output.status.success(), "{check_report}");
    assert!(
        check_report.contains("scalar predicate is false"),
        "expected target-closure predicate refusal:\n{check_report}"
    );
    for engine in ["psi", "native"] {
        let (output, report) = invoke(&source, &["run", "--engine", engine]);
        assert!(
            !output.status.success(),
            "expected {engine} refusal for out-of-bounds index:\n{report}"
        );
    }
}

#[test]
fn target_layout_const_array_extent_refuses_mismatched_literal_length_at_closure() {
    let source = target_const_array_program("[10u8, 20u8, 30u8]", 2);
    let (check_output, check_report) = invoke(&source, &["--check"]);
    assert!(!check_output.status.success(), "{check_report}");
    assert!(
        check_report.contains("array literal length (3) matches target-dependent extent"),
        "expected literal-extent mismatch refusal:\n{check_report}"
    );
    for engine in ["psi", "native"] {
        let (output, report) = invoke(&source, &["run", "--engine", engine]);
        assert!(
            !output.status.success(),
            "expected {engine} refusal for mismatched literal length:\n{report}"
        );
    }
}
