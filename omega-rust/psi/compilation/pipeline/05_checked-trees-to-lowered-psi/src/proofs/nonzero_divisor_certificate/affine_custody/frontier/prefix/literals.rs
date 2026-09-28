//! Producer-local custody selection for landed affine siblings.

use semantic_vocabulary::{IntegerValue, Proposition, PropositionContext, ScalarTerm, ScalarType};

mod landing;
mod step;

pub(super) fn select(
    context: &PropositionContext,
    semantic_axioms: &[Proposition],
    root: &ScalarTerm,
    definition_axioms: &[usize],
    target: &ScalarTerm,
) -> Option<Vec<Option<usize>>> {
    let ScalarType::Integer(integer_type) = root.scalar_type() else {
        return None;
    };
    let mut current = root.clone();
    let mut literal_axioms = Vec::with_capacity(definition_axioms.len());
    for &definition_index in definition_axioms {
        let definition = semantic_axioms.get(definition_index)?;
        context.validate(definition).ok()?;
        let Proposition::Equal(left, right) = definition else {
            return None;
        };
        let mut selected = None;
        for (endpoint, expression) in [(left, right), (right, left)] {
            for (next, sibling) in step::select(
                endpoint,
                expression,
                &current,
                ScalarType::Integer(integer_type),
            ) {
                let literal_axiom = match sibling.integer_value() {
                    Some((actual, IntegerValue::Signed(_) | IntegerValue::Unsigned(_)))
                        if actual == integer_type =>
                    {
                        Some(None)
                    }
                    None if matches!(sibling, ScalarTerm::Value { .. }) => landing::unique(
                        context,
                        semantic_axioms,
                        definition_index,
                        sibling,
                        integer_type,
                    )
                    .map(Some),
                    _ => None,
                };
                let Some(literal_axiom) = literal_axiom else {
                    continue;
                };
                // More than one resolvable traversal forks the chain the same
                // way the kernel rejects an ambiguous definition.
                if selected.replace((next.clone(), literal_axiom)).is_some() {
                    return None;
                }
            }
        }
        let (next, literal_axiom) = selected?;
        literal_axioms.push(literal_axiom);
        current = next;
    }
    (current == *target).then_some(literal_axioms)
}
