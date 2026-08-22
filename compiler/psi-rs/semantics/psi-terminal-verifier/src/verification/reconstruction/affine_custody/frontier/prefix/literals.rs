//! Independent landed affine-sibling custody reconstruction.

use psi_core::{IntegerValue, Proposition, PropositionContext, ScalarTerm, ScalarType};

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
        let forward = step(left, right, &current, ScalarType::Integer(integer_type));
        let reverse = step(right, left, &current, ScalarType::Integer(integer_type));
        let (next, sibling) = match (forward, reverse) {
            (Some(step), None) | (None, Some(step)) => step,
            _ => return None,
        };
        let literal_axiom = match sibling.integer_value() {
            Some((actual, IntegerValue::Signed(_))) if actual == integer_type => None,
            None if matches!(sibling, ScalarTerm::Value { .. }) => Some(unique_landing(
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

fn step<'a>(
    target: &'a ScalarTerm,
    expression: &'a ScalarTerm,
    current: &ScalarTerm,
    expected: ScalarType,
) -> Option<(&'a ScalarTerm, &'a ScalarTerm)> {
    if !matches!(target, ScalarTerm::Value { .. }) || target.scalar_type() != expected {
        return None;
    }
    let (left, right, subtraction) = match expression {
        ScalarTerm::ExactIntegerAdd { left, right, .. }
        | ScalarTerm::ExactIntegerMultiply { left, right, .. } => {
            (left.as_ref(), right.as_ref(), false)
        }
        ScalarTerm::ExactIntegerSubtract { left, right, .. } => {
            (left.as_ref(), right.as_ref(), true)
        }
        _ => return None,
    };
    if expression.scalar_type() != expected {
        return None;
    }
    if left == current {
        Some((target, right))
    } else if !subtraction && right == current {
        Some((target, left))
    } else {
        None
    }
}

fn unique_landing(
    context: &PropositionContext,
    semantic_axioms: &[Proposition],
    definition_index: usize,
    sibling: &ScalarTerm,
    integer_type: psi_core::IntegerType,
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
