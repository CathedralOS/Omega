//! Producer-local unique earlier literal landing selection.

use psi_core::{IntegerType, IntegerValue, Proposition, PropositionContext, ScalarTerm};

pub(super) fn unique(
    context: &PropositionContext,
    semantic_axioms: &[Proposition],
    definition_index: usize,
    sibling: &ScalarTerm,
    integer_type: IntegerType,
) -> Option<usize> {
    let mut matches = semantic_axioms[..definition_index]
        .iter()
        .enumerate()
        .filter_map(|(index, proposition)| {
            context.validate(proposition).ok()?;
            let Proposition::Equal(left, right) = proposition else {
                return None;
            };
            [(left, right), (right, left)]
                .into_iter()
                .find_map(|(value, literal)| {
                    (value == sibling
                        && matches!(
                            literal.integer_value(),
                            Some((actual, IntegerValue::Signed(_))) if actual == integer_type
                        ))
                    .then_some(index)
                })
        });
    let index = matches.next()?;
    matches.next().is_none().then_some(index)
}
