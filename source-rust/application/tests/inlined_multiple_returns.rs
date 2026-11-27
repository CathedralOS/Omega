//! A helper that returns from both arms of a branch reaches its caller's loop
//! with each arm's result, whether it stays a call (the default) or the build
//! selects `SelectedAcyclicCallInliningV1` and it is inlined into the loop.

#[path = "support/program.rs"]
mod program;
mod support;

#[test]
fn an_inlined_helper_returns_from_either_arm_on_both_engines() {
    let fixture =
        support::repository_root().join("tests/fixtures/calls/run/inlined_multiple_returns");
    program::assert_fixture_silent_return(&fixture);
    program::assert_selected_fixture_silent_return(&fixture, &["SelectedAcyclicCallInliningV1"]);
}
