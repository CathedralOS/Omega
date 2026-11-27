//! Byte views passed across transitions keep their values natively.
//!
//! Native selection leaves a view descriptor that a state passes back to its
//! own parameter in place, with no copy on the transition.

#[path = "support/program.rs"]
mod program;
mod support;

#[test]
fn unchanged_views_cross_transitions_on_both_engines() {
    program::assert_fixture_silent_return(
        &support::repository_root()
            .join("source-rust/application/tests/fixtures/views/run/view_transition_copies"),
    );
}
