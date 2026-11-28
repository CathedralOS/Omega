//! Nested states pass a copyable record on unchanged without copying it natively.
//!
//! Native selection keeps the record in the first state's parameter; a state
//! that changes its own copy still gets one.

#[path = "support/program.rs"]
mod program;
mod support;

#[test]
fn unchanged_records_cross_nested_states_when_interpreted_and_built() {
    program::assert_fixture_silent_return(
        &support::repository_root()
            .join("source-rust/application/tests/fixtures/views/run/nested_record_copies"),
    );
}
