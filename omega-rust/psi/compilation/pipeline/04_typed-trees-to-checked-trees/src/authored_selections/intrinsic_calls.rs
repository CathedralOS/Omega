//! Checked intrinsic calls retained by the checking owner.

use checked_trees::CheckFacts;
use language_semantics::declaration_selection::AuthoredDeclarationSelectionIntrinsic;

pub(crate) fn checked_intrinsic_call_target(
    facts: &CheckFacts,
    expression: typed_trees::expression::ExpressionHandle,
) -> Option<AuthoredDeclarationSelectionIntrinsic> {
    let mut matching = facts
        .intrinsic_calls
        .iter()
        .filter(|fact| fact.expression == expression)
        .map(|fact| fact.intrinsic);
    let selected = matching.next()?;
    matching
        .all(|candidate| candidate == selected)
        .then_some(selected)
}
