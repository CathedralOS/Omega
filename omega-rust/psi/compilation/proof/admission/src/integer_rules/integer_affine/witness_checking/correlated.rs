use super::*;

pub(super) fn check_correlated_add_witness(
    context: &PropositionContext,
    semantic_axioms: &[Proposition],
    witness: &IntegerAffineWitness,
) -> Result<Option<CheckedIntegerAffineForm>, IntegerAffineWitnessError> {
    let ScalarTerm::ExactIntegerAdd {
        scalar_type,
        left,
        right,
    } = &witness.target
    else {
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
            let axiom = semantic_axioms
                .get(*index)
                .ok_or(IntegerAffineWitnessError::UnknownSemanticAxiom(*index))?;
            context
                .validate(axiom)
                .map_err(IntegerAffineWitnessError::MalformedProposition)?;
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
    let ScalarTerm::ExactIntegerSubtract {
        scalar_type: subtract_type,
        left: endpoint,
        right: subtract_right,
    } = expression
    else {
        return Ok(None);
    };
    if subtract_type != scalar_type || subtract_right.as_ref() != right.as_ref() {
        return Ok(None);
    }
    let endpoint_value = match (endpoint.integer_value(), landing) {
        (Some((actual, value)), None | Some((_, None))) => {
            (actual == *scalar_type).then_some(value)
        }
        (None, Some((index, Some(landing_index)))) => {
            let landed = landed_integer(context, semantic_axioms, index, landing_index)?;
            (landed.term == endpoint.as_ref().clone() && landed.integer_type == *scalar_type)
                .then_some(landed.value)
        }
        _ => return Ok(None),
    };
    let endpoint_step = match endpoint_value {
        Some(value) if value == scalar_type.minimum_value() => {
            CheckedIntegerEndpointStep::CorrelatedAddLower
        }
        Some(value) if value == scalar_type.maximum_value() => {
            CheckedIntegerEndpointStep::CorrelatedAddUpper
        }
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

pub(super) fn check_correlated_subtract_witness(
    context: &PropositionContext,
    semantic_axioms: &[Proposition],
    witness: &IntegerAffineWitness,
) -> Result<Option<CheckedIntegerAffineForm>, IntegerAffineWitnessError> {
    let ScalarTerm::ExactIntegerSubtract {
        scalar_type,
        left,
        right,
    } = &witness.target
    else {
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
            let axiom = semantic_axioms
                .get(*index)
                .ok_or(IntegerAffineWitnessError::UnknownSemanticAxiom(*index))?;
            context
                .validate(axiom)
                .map_err(IntegerAffineWitnessError::MalformedProposition)?;
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
    let ScalarTerm::ExactIntegerAdd {
        scalar_type: add_type,
        left: endpoint,
        right: add_right,
    } = expression
    else {
        return Ok(None);
    };
    if add_type != scalar_type || add_right.as_ref() != right.as_ref() {
        return Ok(None);
    }
    let endpoint_value = match (endpoint.integer_value(), landing) {
        (Some((actual, value)), None | Some((_, None))) => {
            (actual == *scalar_type).then_some(value)
        }
        (None, Some((index, Some(landing_index)))) => {
            let landed = landed_integer(context, semantic_axioms, index, landing_index)?;
            (landed.term == endpoint.as_ref().clone() && landed.integer_type == *scalar_type)
                .then_some(landed.value)
        }
        _ => return Ok(None),
    };
    let endpoint_step = match endpoint_value {
        Some(value) if value == scalar_type.minimum_value() => {
            CheckedIntegerEndpointStep::CorrelatedSubtractLower
        }
        Some(value) if value == scalar_type.maximum_value() => {
            CheckedIntegerEndpointStep::CorrelatedSubtractUpper
        }
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

pub(super) fn check_correlated_multiply_witness(
    context: &PropositionContext,
    semantic_axioms: &[Proposition],
    witness: &IntegerAffineWitness,
) -> Result<Option<CheckedIntegerAffineForm>, IntegerAffineWitnessError> {
    let ScalarTerm::ExactIntegerMultiply {
        scalar_type,
        left,
        right,
    } = &witness.target
    else {
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
            let axiom = semantic_axioms
                .get(*index)
                .ok_or(IntegerAffineWitnessError::UnknownSemanticAxiom(*index))?;
            context
                .validate(axiom)
                .map_err(IntegerAffineWitnessError::MalformedProposition)?;
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
    let ScalarTerm::ExactIntegerDivide {
        scalar_type: divide_type,
        left: endpoint,
        right: divide_right,
    } = expression
    else {
        return Ok(None);
    };
    if divide_type != scalar_type || divide_right.as_ref() != right.as_ref() {
        return Ok(None);
    }
    let endpoint_value = match (endpoint.integer_value(), landing) {
        (Some((actual, value)), None | Some((_, None))) => {
            (actual == *scalar_type).then_some(value)
        }
        (None, Some((index, Some(landing_index)))) => {
            let landed = landed_integer(context, semantic_axioms, index, landing_index)?;
            (landed.term == endpoint.as_ref().clone() && landed.integer_type == *scalar_type)
                .then_some(landed.value)
        }
        _ => return Ok(None),
    };
    let endpoint_step = match endpoint_value {
        Some(value) if value == scalar_type.minimum_value() => {
            CheckedIntegerEndpointStep::CorrelatedMultiplyMinimum
        }
        Some(value) if value == scalar_type.maximum_value() => {
            CheckedIntegerEndpointStep::CorrelatedMultiplyMaximum
        }
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
