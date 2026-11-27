//! A helper that returns from both arms of a branch is inlined into its
//! caller's loop, and the caller receives each arm's result.

#[path = "support/program.rs"]
mod program;
mod support;

#[test]
fn an_inlined_helper_returns_from_either_arm_on_both_engines() {
    let fixture =
        support::repository_root().join("tests/fixtures/calls/run/inlined_multiple_returns");
    program::assert_fixture_silent_return(&fixture);
}
