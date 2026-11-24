//! Term forms a clause writes, read by stage 05's one clause reader
//! (`read/clause_terms.rs`), so a form served in one position is served in
//! every other.

use super::build_purpose::judge_build;
use super::*;

/// `!b` was read only as a whole clause, so `requires a == !b` was refused
/// as a term with no carrier value. It is the Boolean negation of `b` as an
/// operand too: a caller that knows `a == !b` meets the requirement, and one
/// that knows only `a == b` does not.
#[test]
fn a_negated_boolean_is_an_operand_of_a_comparison() {
    const NEGATED: &str =
        include_str!("../../tests/fixtures/clause_terms/pass/negated_operand/build.omg");
    const UNRELATED: &str =
        include_str!("../../tests/fixtures/clause_terms/fail/negated_operand_unrelated/build.omg");
    match judge_build(NEGATED) {
        Ok(RouteVerdict::Accepted(accepted)) => {
            accepted.publish_invocation().unwrap();
        }
        other => panic!("`a == !b` must meet the same requirement: {other:?}"),
    }
    let unproved = match judge_build(UNRELATED) {
        Ok(RouteVerdict::Incomplete(unproved)) | Ok(RouteVerdict::Rejected { unproved, .. }) => {
            unproved
        }
        other => panic!("`a == b` must leave `a == !b` unproved: {other:?}"),
    };
    assert_eq!(unproved.len(), 1, "{unproved:?}");
    assert!(
        matches!(unproved[0].kind, ObligationKind::CallRequirement { .. }),
        "{unproved:?}",
    );
}
