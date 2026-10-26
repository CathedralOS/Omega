use super::*;

/// A correlated witness uses the target arithmetic term's matching inverse
/// sibling as the landed endpoint expression: add checks subtract, subtract
/// checks add, and multiply checks divide. Each checker differs only in the
/// two term variants it matches and the endpoint steps it records, so one
/// parameterized check carries all three.
fn check_correlated_witness(
    context: &PropositionContext,
    semantic_axioms: &[Proposition],
    witness: &IntegerAffineWitness,
    outer: TermMatcher,
    inner: TermMatcher,
    minimum_step: CheckedIntegerEndpointStep,
    maximum_step: CheckedIntegerEndpointStep,
) -> Result<Option<CheckedIntegerAffineForm>, IntegerAffineWitnessError> {
    let Some((scalar_type, left, right)) = outer(&witness.target) else {
        return Ok(None);
    };
    if scalar_type.carrier() != IntegerCarrier::Fixed
        || direct_math_leaf(left, *scalar_type).is_none()
        || direct_math_leaf(right, *scalar_type).is_none()
        || witness.root.scalar_type() != ScalarType::Integer(*scalar_type)
    {
        return Ok(None);
    }
    let (expression, landing) = match (
        witness.definition_axioms.as_slice(),
        witness.literal_axioms.as_slice(),
    ) {
        ([], []) => (&witness.root, None),
        ([index], [landing]) if matches!(witness.root, ScalarTerm::Value { .. }) => {
            let axiom = checked_axiom(context, semantic_axioms, *index)?;
            let Proposition::Equal(equal_left, equal_right) = axiom else {
                return Ok(None);
            };
            let expression = if equal_left == &witness.root {
                equal_right
            } else if equal_right == &witness.root {
                equal_left
            } else {
                return Ok(None);
            };
            (expression, Some((*index, *landing)))
        }
        _ => return Ok(None),
    };
    let Some((inner_type, endpoint, inner_right)) = inner(expression) else {
        return Ok(None);
    };
    if inner_type != scalar_type || inner_right != right {
        return Ok(None);
    }
    let endpoint_value = match (endpoint.integer_value(), landing) {
        (Some((actual, value)), None | Some((_, None))) => {
            (actual == *scalar_type).then_some(value)
        }
        (None, Some((index, Some(landing_index)))) => {
            let landed = landed_integer(context, semantic_axioms, index, landing_index)?;
            (landed.term == *endpoint && landed.integer_type == *scalar_type)
                .then_some(landed.value)
        }
        _ => return Ok(None),
    };
    let endpoint_step = match endpoint_value {
        Some(value) if value == scalar_type.minimum_value() => minimum_step,
        Some(value) if value == scalar_type.maximum_value() => maximum_step,
        _ => return Ok(None),
    };
    Ok(Some(CheckedIntegerAffineForm {
        root: witness.root.clone(),
        target: witness.target.clone(),
        integer_type: *scalar_type,
        coefficient: 1,
        offset: 0,
        endpoint_steps: vec![endpoint_step],
    }))
}

pub(super) fn check_correlated_add_witness(
    context: &PropositionContext,
    semantic_axioms: &[Proposition],
    witness: &IntegerAffineWitness,
) -> Result<Option<CheckedIntegerAffineForm>, IntegerAffineWitnessError> {
    check_correlated_witness(
        context,
        semantic_axioms,
        witness,
        exact_integer_add,
        exact_integer_subtract,
        CheckedIntegerEndpointStep::CorrelatedAddLower,
        CheckedIntegerEndpointStep::CorrelatedAddUpper,
    )
}

pub(super) fn check_correlated_subtract_witness(
    context: &PropositionContext,
    semantic_axioms: &[Proposition],
    witness: &IntegerAffineWitness,
) -> Result<Option<CheckedIntegerAffineForm>, IntegerAffineWitnessError> {
    check_correlated_witness(
        context,
        semantic_axioms,
        witness,
        exact_integer_subtract,
        exact_integer_add,
        CheckedIntegerEndpointStep::CorrelatedSubtractLower,
        CheckedIntegerEndpointStep::CorrelatedSubtractUpper,
    )
}

pub(super) fn check_correlated_multiply_witness(
    context: &PropositionContext,
    semantic_axioms: &[Proposition],
    witness: &IntegerAffineWitness,
) -> Result<Option<CheckedIntegerAffineForm>, IntegerAffineWitnessError> {
    check_correlated_witness(
        context,
        semantic_axioms,
        witness,
        exact_integer_multiply,
        exact_integer_divide,
        CheckedIntegerEndpointStep::CorrelatedMultiplyMinimum,
        CheckedIntegerEndpointStep::CorrelatedMultiplyMaximum,
    )
}
