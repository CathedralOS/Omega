//! Compare selected guards with requirements using the same observation
//! identities as ordinary contract facts. Scalar copies, sealed predicates and
//! reference windows all retain their complete argument-effect interval.

use super::super::direct::GuardDenotations;
use typed_trees::expression::ExpressionHandle;

pub(super) fn conjunct_matches(
    queries: &GuardDenotations<'_>,
    guard: ExpressionHandle,
    required: ExpressionHandle,
) -> bool {
    super::super::direct::guard_value_proves_with(
        queries.program,
        guard,
        true,
        required,
        true,
        &|left, right| queries.equal(left, right),
    )
}
