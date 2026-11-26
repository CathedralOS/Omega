//! Byte views passed across transitions keep their values natively.
//!
//! Native selection copies a view descriptor straight into the next state's
//! parameter when no binding of the edge reads a slot another binding writes.

#[path = "support/build_fixture.rs"]
mod build_fixture;
mod support;

#[test]
fn unchanged_views_cross_transitions_on_both_engines() {
    build_fixture::assert_silent_return(
        &support::repository_root()
            .join("source-rust/application/tests/fixtures/views/run/view_transition_copies"),
    );
}
