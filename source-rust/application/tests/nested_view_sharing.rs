//! Nested states that pass mutable views to each other keep their writes natively.
//!
//! Native selection lets `put` reuse the view slots of `step`, which enters it;
//! writes through either state still reach the caller's buffers.

#[path = "support/program.rs"]
mod program;
mod support;

#[test]
fn nested_states_share_view_parameters_on_both_engines() {
    program::assert_fixture_silent_return(
        &support::repository_root()
            .join("source-rust/application/tests/fixtures/views/run/nested_view_sharing"),
    );
}
