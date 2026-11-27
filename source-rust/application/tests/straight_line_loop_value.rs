//! A loop body built from chained sub-expressions keeps its value natively.
//!
//! The source compiler gives each sub-expression of `acc + (kw ^ (acc >> 3))`
//! its own block, so native selection folds `block -> edge copies -> block`
//! chains into one block. The fixture checks the folded loop returns the same
//! value as Psi.

#[path = "support/program.rs"]
mod program;
mod support;

#[test]
fn a_chained_expression_loop_returns_the_same_value_when_interpreted_and_built() {
    program::assert_fixture_silent_return(
        &support::repository_root().join(
            "source-rust/application/tests/fixtures/transitions/run/straight_line_loop_value",
        ),
    );
}
