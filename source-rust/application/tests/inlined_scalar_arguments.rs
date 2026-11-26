//! A recursive call keeps its integer and Boolean inputs after its parity
//! helper has been inlined into the caller.

#[path = "support/program.rs"]
mod program;
mod support;

#[test]
fn inlining_preserves_the_types_of_remaining_call_arguments() {
    let fixture =
        support::repository_root().join("tests/fixtures/calls/run/inlined_scalar_arguments");
    program::assert_fixture_silent_return(&fixture);
}
