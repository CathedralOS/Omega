//! Producer-local custody selection for landed affine siblings.

use psi_core::{IntegerValue, Proposition, PropositionContext, ScalarTerm, ScalarType};

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
        let forward = step::select(left, right, &current, ScalarType::Integer(integer_type));
        let reverse = step::select(right, left, &current, ScalarType::Integer(integer_type));
        let (next, sibling) = match (forward, reverse) {
            (Some(step), None) | (None, Some(step)) => step,
            _ => return None,
        };
        let literal_axiom = match sibling.integer_value() {
            Some((actual, IntegerValue::Signed(_))) if actual == integer_type => None,
            None if matches!(sibling, ScalarTerm::Value { .. }) => Some(landing::unique(
                context,
                semantic_axioms,
                definition_index,
                sibling,
                integer_type,
            )?),
            _ => return None,
        };
        literal_axioms.push(literal_axiom);
        current = next.clone();
    }
    (current == *target).then_some(literal_axioms)
}
