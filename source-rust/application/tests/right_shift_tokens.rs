//! A right shift is two `>` that touch, and the same two `>` close two nested
//! generic argument lists (wiki/spec/language/expressions.md). The run
//! program uses both and runs in both engines; its twin puts a space between
//! the two `>` of the shift and is refused at the second one.

#[path = "support/program.rs"]
mod program;
mod support;

use std::path::PathBuf;

fn fixture(tier: &str) -> PathBuf {
    support::repository_root()
        .join("source-rust/application/tests/fixtures/numeric")
        .join(tier)
        .join("right_shift_tokens")
}

#[test]
fn touching_greater_tokens_shift_and_close_nested_lists_in_both_engines() {
    program::assert_fixture_silent_return(&fixture("run"));
}

#[test]
fn a_space_between_the_two_greater_tokens_is_not_a_shift() {
    let expected = std::fs::read_to_string(fixture("fail").join("expected.txt")).unwrap();
    let (output, report) = program::check_fixture(&fixture("fail"));
    assert!(!output.status.success(), "{report}");
    assert!(report.contains(expected.trim()), "{report}");
}
