//! A recursive call keeps its integer and Boolean inputs after its parity
//! helper has been inlined into the caller.

#[path = "support/build_fixture.rs"]
mod build_fixture;
mod support;

#[test]
fn inlining_preserves_the_types_of_remaining_call_arguments() {
    let fixture =
        support::repository_root().join("tests/fixtures/calls/run/inlined_scalar_arguments");
    build_fixture::assert_silent_return(&fixture);
}
