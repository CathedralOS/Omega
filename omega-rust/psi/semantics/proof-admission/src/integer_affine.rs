//! Independently checked normalization for ordered same-carrier endpoint maps.
//!
//! This is a certificate prerequisite, not an arithmetic proof rule. It binds
//! a producer's normalized affine or landed-count exact-shift claim to exact,
//! prior semantic-axiom rows so later proof rules do not need to trust an
//! analyzer's coefficients or endpoint arithmetic.

pub use terminal_psi::IntegerAffineWitness;

use semantic_vocabulary::{
    IntegerCarrier, IntegerMathTerm, IntegerSign, IntegerType, IntegerValue, Proposition,
    PropositionContext, ScalarTerm, ScalarType,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedIntegerAffineForm {
    root: ScalarTerm,
    target: ScalarTerm,
    integer_type: IntegerType,
    coefficient: i128,
    offset: i128,
    endpoint_steps: Vec<CheckedIntegerEndpointStep>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CheckedIntegerEndpointStep {
    Add(i128),
    Subtract(i128),
    Multiply(i128),
    Divide(i128),
    Remainder(i128),
    ShiftLeft(u32),
    ShiftRight(u32),
    CorrelatedAddLower,
    CorrelatedAddUpper,
    CorrelatedSubtractLower,
    CorrelatedSubtractUpper,
    CorrelatedUnsignedSubtract,
    CorrelatedMultiplyMinimum,
    CorrelatedMultiplyMaximum,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LandedInteger {
    term: ScalarTerm,
    integer_type: IntegerType,
    value: IntegerValue,
}

impl CheckedIntegerAffineForm {
    pub const fn integer_type(&self) -> IntegerType {
        self.integer_type
    }

    pub const fn coefficient(&self) -> i128 {
        self.coefficient
    }

    pub const fn offset(&self) -> i128 {
        self.offset
    }

    pub const fn root(&self) -> &ScalarTerm {
        &self.root
    }

    pub const fn target(&self) -> &ScalarTerm {
        &self.target
    }
}

pub fn check_integer_affine_witness(
    context: &PropositionContext,
    semantic_axioms: &[Proposition],
    witness: &IntegerAffineWitness,
) -> Result<CheckedIntegerAffineForm, IntegerAffineWitnessError> {
    if let Some(form) = check_correlated_add_witness(context, semantic_axioms, witness)? {
        return Ok(form);
    }
    if let Some(form) = check_correlated_subtract_witness(context, semantic_axioms, witness)? {
        return Ok(form);
    }
    if let Some(form) = check_correlated_multiply_witness(context, semantic_axioms, witness)? {
        return Ok(form);
    }
    if witness.definition_axioms.is_empty()
        && witness.literal_axioms.is_empty()
        && let ScalarTerm::ExactIntegerSubtract {
            scalar_type,
            left,
            right,
        } = &witness.target
        && scalar_type.carrier() == IntegerCarrier::Fixed
        && scalar_type.sign() == IntegerSign::Unsigned
        && right.as_ref() == &witness.root
        && left.as_ref() != right.as_ref()
        && direct_math_leaf(left, *scalar_type).is_some()
        && direct_math_leaf(right, *scalar_type).is_some()
    {
        return Ok(CheckedIntegerAffineForm {
            root: witness.root.clone(),
            target: witness.target.clone(),
            integer_type: *scalar_type,
            coefficient: 1,
            offset: 0,
            endpoint_steps: vec![CheckedIntegerEndpointStep::CorrelatedUnsignedSubtract],
        });
    }
    if witness.definition_axioms.is_empty()
        && witness.literal_axioms.is_empty()
        && let ScalarTerm::ExactIntegerMultiply {
            scalar_type,
            left,
            right,
        } = &witness.target
        && (left.as_ref() == &witness.root || right.as_ref() == &witness.root)
        && scalar_type.carrier() == IntegerCarrier::Fixed
        && direct_math_leaf(left, *scalar_type).is_some()
        && direct_math_leaf(right, *scalar_type).is_some()
    {
        return Ok(CheckedIntegerAffineForm {
            root: witness.root.clone(),
            target: witness.target.clone(),
            integer_type: *scalar_type,
            coefficient: 1,
            offset: 0,
            endpoint_steps: Vec::new(),
        });
    }
    if witness.definition_axioms.is_empty()
        && witness.literal_axioms.is_empty()
        && let ScalarTerm::ExactIntegerAdd {
            scalar_type,
            left,
            right,
        } = &witness.target
        && left.as_ref() == &witness.root
        && scalar_type.carrier() == IntegerCarrier::Fixed
        && direct_math_leaf(left, *scalar_type).is_some()
        && direct_math_leaf(right, *scalar_type).is_some()
    {
        return Ok(CheckedIntegerAffineForm {
            root: witness.root.clone(),
            target: witness.target.clone(),
            integer_type: *scalar_type,
            coefficient: 1,
            offset: 0,
            endpoint_steps: Vec::new(),
        });
    }
    if witness.definition_axioms.is_empty()
        && witness.literal_axioms.is_empty()
        && let ScalarTerm::ExactIntegerSubtract {
            scalar_type,
            left,
            right,
        } = &witness.target
        && left.as_ref() == &witness.root
        && scalar_type.carrier() == IntegerCarrier::Fixed
        && direct_math_leaf(left, *scalar_type).is_some()
        && direct_math_leaf(right, *scalar_type).is_some()
    {
        return Ok(CheckedIntegerAffineForm {
            root: witness.root.clone(),
            target: witness.target.clone(),
            integer_type: *scalar_type,
            coefficient: 1,
            offset: 0,
            endpoint_steps: Vec::new(),
        });
    }
    if witness.definition_axioms.is_empty()
        && witness.literal_axioms.is_empty()
        && let ScalarTerm::ExactIntegerShiftLeft {
            value_type,
            count_type,
            value,
            count: _,
        } = &witness.target
        && value.as_ref() == &witness.root
        && value_type.carrier() == IntegerCarrier::Fixed
        && count_type.carrier() == IntegerCarrier::Fixed
    {
        return Ok(CheckedIntegerAffineForm {
            root: witness.root.clone(),
            target: witness.target.clone(),
            integer_type: *value_type,
            coefficient: 1,
            offset: 0,
            endpoint_steps: Vec::new(),
        });
    }
    if !matches!(witness.root, ScalarTerm::Value { .. }) {
        return Err(IntegerAffineWitnessError::RootNotValue);
    }
    let ScalarType::Integer(integer_type) = witness.root.scalar_type() else {
        return Err(IntegerAffineWitnessError::RootNotInteger);
    };
    if integer_type.carrier() != IntegerCarrier::Fixed {
        return Err(IntegerAffineWitnessError::UnsupportedCarrier(integer_type));
    }
    if witness.target.scalar_type() != ScalarType::Integer(integer_type) {
        return Err(IntegerAffineWitnessError::TargetTypeMismatch);
    }
    if witness.definition_axioms.is_empty() {
        return Err(IntegerAffineWitnessError::EmptyDefinitionChain);
    }
    if witness.literal_axioms.len() != witness.definition_axioms.len() {
        return Err(IntegerAffineWitnessError::LiteralAxiomCountMismatch);
    }
    if witness
        .definition_axioms
        .windows(2)
        .any(|indices| indices[0] >= indices[1])
    {
        return Err(IntegerAffineWitnessError::NonCanonicalDefinitionOrder);
    }

    let mut current = witness.root.clone();
    let mut coefficient = 1_i128;
    let mut offset = 0_i128;
    let mut endpoint_steps = Vec::with_capacity(witness.definition_axioms.len());
    let mut previous = None;
    for (&index, &literal_index) in witness
        .definition_axioms
        .iter()
        .zip(&witness.literal_axioms)
    {
        if previous.is_some_and(|previous| index <= previous) {
            return Err(IntegerAffineWitnessError::NonCanonicalDefinitionOrder);
        }
        previous = Some(index);
        let proposition = semantic_axioms
            .get(index)
            .ok_or(IntegerAffineWitnessError::UnknownSemanticAxiom(index))?;
        context
            .validate(proposition)
            .map_err(IntegerAffineWitnessError::MalformedProposition)?;
        let Proposition::Equal(left, right) = proposition else {
            return Err(IntegerAffineWitnessError::DefinitionNotEquality(index));
        };
        let landed = literal_index
            .map(|literal_index| landed_integer(context, semantic_axioms, index, literal_index))
            .transpose()?;
        let forward = apply_definition(
            left,
            right,
            &current,
            integer_type,
            coefficient,
            offset,
            landed.as_ref(),
        );
        let reverse = apply_definition(
            right,
            left,
            &current,
            integer_type,
            coefficient,
            offset,
            landed.as_ref(),
        );
        let shift_forward =
            apply_shift_definition(left, right, &current, integer_type, landed.as_ref(), index);
        let shift_reverse =
            apply_shift_definition(right, left, &current, integer_type, landed.as_ref(), index);
        let candidates = [forward, reverse, shift_forward, shift_reverse]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        let (next, next_coefficient, next_offset, used_landing, endpoint_step) =
            match candidates.as_slice() {
                [next] => next.clone()?,
                [] => return Err(IntegerAffineWitnessError::DefinitionShapeMismatch(index)),
                _ => {
                    return Err(IntegerAffineWitnessError::AmbiguousDefinition(index));
                }
            };
        if landed.is_some() != used_landing {
            return Err(IntegerAffineWitnessError::UnusedLiteralAxiom(index));
        }
        current = next;
        coefficient = next_coefficient;
        offset = next_offset;
        endpoint_steps.push(endpoint_step);
    }
    if current != witness.target {
        return Err(IntegerAffineWitnessError::TargetMismatch);
    }
    Ok(CheckedIntegerAffineForm {
        root: witness.root.clone(),
        target: witness.target.clone(),
        integer_type,
        coefficient,
        offset,
        endpoint_steps,
    })
}

fn check_correlated_add_witness(
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

fn check_correlated_subtract_witness(
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

fn check_correlated_multiply_witness(
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

fn apply_definition(
    target: &ScalarTerm,
    expression: &ScalarTerm,
    current: &ScalarTerm,
    integer_type: IntegerType,
    coefficient: i128,
    offset: i128,
    landed: Option<&LandedInteger>,
) -> Option<
    Result<(ScalarTerm, i128, i128, bool, CheckedIntegerEndpointStep), IntegerAffineWitnessError>,
> {
    if !matches!(target, ScalarTerm::Value { .. })
        || target.scalar_type() != ScalarType::Integer(integer_type)
    {
        return None;
    }
    let transformed = match expression {
        ScalarTerm::ExactIntegerAdd {
            scalar_type,
            left,
            right,
        } if *scalar_type == integer_type && left.as_ref() == current => {
            let (literal, used_landing) = signed_literal(right, integer_type, landed)?;
            (
                Some(coefficient),
                offset.checked_add(literal),
                used_landing,
                CheckedIntegerEndpointStep::Add(literal),
            )
        }
        ScalarTerm::ExactIntegerAdd {
            scalar_type,
            left,
            right,
        } if *scalar_type == integer_type && right.as_ref() == current => {
            let (literal, used_landing) = signed_literal(left, integer_type, landed)?;
            (
                Some(coefficient),
                offset.checked_add(literal),
                used_landing,
                CheckedIntegerEndpointStep::Add(literal),
            )
        }
        ScalarTerm::ExactIntegerSubtract {
            scalar_type,
            left,
            right,
        } if *scalar_type == integer_type && left.as_ref() == current => {
            let (literal, used_landing) = signed_literal(right, integer_type, landed)?;
            (
                Some(coefficient),
                offset.checked_sub(literal),
                used_landing,
                CheckedIntegerEndpointStep::Subtract(literal),
            )
        }
        ScalarTerm::ExactIntegerMultiply {
            scalar_type,
            left,
            right,
        } if *scalar_type == integer_type && left.as_ref() == current => {
            let (literal, used_landing) = signed_literal(right, integer_type, landed)?;
            (
                coefficient.checked_mul(literal),
                offset.checked_mul(literal),
                used_landing,
                CheckedIntegerEndpointStep::Multiply(literal),
            )
        }
        ScalarTerm::ExactIntegerDivide {
            scalar_type,
            left,
            right,
        } if *scalar_type == integer_type && left.as_ref() == current => {
            let (literal, used_landing) = signed_literal(right, integer_type, landed)?;
            if literal == 0 {
                return Some(Err(IntegerAffineWitnessError::ZeroDivisionLiteral));
            }
            (
                Some(coefficient),
                Some(offset),
                used_landing,
                CheckedIntegerEndpointStep::Divide(literal),
            )
        }
        ScalarTerm::ExactIntegerRemainder {
            scalar_type,
            left,
            right,
        } if *scalar_type == integer_type && left.as_ref() == current => {
            let (literal, used_landing) = signed_literal(right, integer_type, landed)?;
            if literal == 0 {
                return Some(Err(IntegerAffineWitnessError::ZeroDivisionLiteral));
            }
            (
                Some(coefficient),
                Some(offset),
                used_landing,
                CheckedIntegerEndpointStep::Remainder(literal),
            )
        }
        ScalarTerm::ExactIntegerMultiply {
            scalar_type,
            left,
            right,
        } if *scalar_type == integer_type && right.as_ref() == current => {
            let (literal, used_landing) = signed_literal(left, integer_type, landed)?;
            (
                coefficient.checked_mul(literal),
                offset.checked_mul(literal),
                used_landing,
                CheckedIntegerEndpointStep::Multiply(literal),
            )
        }
        _ => return None,
    };
    Some(match transformed {
        (Some(coefficient), Some(offset), used_landing, endpoint_step) => Ok((
            target.clone(),
            coefficient,
            offset,
            used_landing,
            endpoint_step,
        )),
        _ => Err(IntegerAffineWitnessError::CoefficientOverflow),
    })
}

fn signed_literal(
    term: &ScalarTerm,
    integer_type: IntegerType,
    landed: Option<&LandedInteger>,
) -> Option<(i128, bool)> {
    if let Some((actual_type, IntegerValue::Signed(value))) = term.integer_value()
        && actual_type == integer_type
    {
        return Some((value, false));
    }
    if let Some((actual_type, IntegerValue::Unsigned(value))) = term.integer_value()
        && actual_type == integer_type
    {
        return i128::try_from(value).ok().map(|value| (value, false));
    }
    landed
        .filter(|landed| landed.term == *term && landed.integer_type == integer_type)
        .and_then(|landed| match landed.value {
            IntegerValue::Signed(value) => Some((value, true)),
            IntegerValue::Unsigned(value) => i128::try_from(value).ok().map(|value| (value, true)),
        })
}

fn landed_integer(
    context: &PropositionContext,
    semantic_axioms: &[Proposition],
    definition_index: usize,
    literal_index: usize,
) -> Result<LandedInteger, IntegerAffineWitnessError> {
    if literal_index >= definition_index {
        return Err(IntegerAffineWitnessError::LiteralAxiomNotPrior {
            definition: definition_index,
            literal: literal_index,
        });
    }
    let proposition = semantic_axioms.get(literal_index).ok_or(
        IntegerAffineWitnessError::UnknownSemanticAxiom(literal_index),
    )?;
    context
        .validate(proposition)
        .map_err(IntegerAffineWitnessError::MalformedProposition)?;
    let Proposition::Equal(left, right) = proposition else {
        return Err(IntegerAffineWitnessError::LiteralAxiomNotEquality(
            literal_index,
        ));
    };
    for (value, literal) in [(left, right), (right, left)] {
        if matches!(value, ScalarTerm::Value { .. })
            && let Some((integer_type, literal)) = literal.integer_value()
        {
            return Ok(LandedInteger {
                term: value.clone(),
                integer_type,
                value: literal,
            });
        }
    }
    Err(IntegerAffineWitnessError::LiteralAxiomShapeMismatch(
        literal_index,
    ))
}

fn apply_shift_definition(
    target: &ScalarTerm,
    expression: &ScalarTerm,
    current: &ScalarTerm,
    integer_type: IntegerType,
    landed: Option<&LandedInteger>,
    definition_index: usize,
) -> Option<
    Result<(ScalarTerm, i128, i128, bool, CheckedIntegerEndpointStep), IntegerAffineWitnessError>,
> {
    if !matches!(target, ScalarTerm::Value { .. })
        || target.scalar_type() != ScalarType::Integer(integer_type)
        || !matches!(integer_type.bits(), 8 | 16 | 32 | 64)
    {
        return None;
    }
    let (count_type, count, left) = match expression {
        ScalarTerm::ExactIntegerShiftLeft {
            value_type,
            count_type,
            value,
            count,
        } if *value_type == integer_type && value.as_ref() == current => {
            (*count_type, count.as_ref(), true)
        }
        ScalarTerm::ExactIntegerShiftRight {
            value_type,
            count_type,
            value,
            count,
        } if *value_type == integer_type && value.as_ref() == current => {
            (*count_type, count.as_ref(), false)
        }
        _ => return None,
    };
    let Some((count, used_landing)) = nonnegative_count(count, count_type, landed) else {
        return Some(Err(IntegerAffineWitnessError::ShiftCountNotLanded(
            definition_index,
        )));
    };
    let Ok(count) = u32::try_from(count) else {
        return Some(Err(IntegerAffineWitnessError::ShiftCountOutsideValueWidth(
            definition_index,
        )));
    };
    if count >= u32::from(integer_type.bits()) {
        return Some(Err(IntegerAffineWitnessError::ShiftCountOutsideValueWidth(
            definition_index,
        )));
    }
    let step = if left {
        CheckedIntegerEndpointStep::ShiftLeft(count)
    } else {
        CheckedIntegerEndpointStep::ShiftRight(count)
    };
    Some(Ok((target.clone(), 1, 0, used_landing, step)))
}

fn nonnegative_count(
    term: &ScalarTerm,
    count_type: IntegerType,
    landed: Option<&LandedInteger>,
) -> Option<(u128, bool)> {
    let (actual_type, value, used_landing) =
        if let Some((actual_type, value)) = term.integer_value() {
            (actual_type, value, false)
        } else {
            let landed = landed.filter(|landed| landed.term == *term)?;
            (landed.integer_type, landed.value, true)
        };
    if actual_type != count_type || actual_type.carrier() != IntegerCarrier::Fixed {
        return None;
    }
    match value {
        IntegerValue::Signed(value) => u128::try_from(value)
            .ok()
            .map(|value| (value, used_landing)),
        IntegerValue::Unsigned(value) => Some((value, used_landing)),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IntegerAffineWitnessError {
    RootNotValue,
    RootNotInteger,
    UnsupportedCarrier(IntegerType),
    TargetTypeMismatch,
    EmptyDefinitionChain,
    LiteralAxiomCountMismatch,
    NonCanonicalDefinitionOrder,
    UnknownSemanticAxiom(usize),
    MalformedProposition(semantic_vocabulary::PropositionError),
    DefinitionNotEquality(usize),
    LiteralAxiomNotPrior { definition: usize, literal: usize },
    LiteralAxiomNotEquality(usize),
    LiteralAxiomShapeMismatch(usize),
    UnusedLiteralAxiom(usize),
    DefinitionShapeMismatch(usize),
    AmbiguousDefinition(usize),
    CoefficientOverflow,
    ZeroDivisionLiteral,
    ShiftCountNotLanded(usize),
    ShiftCountOutsideValueWidth(usize),
    TargetMismatch,
}

impl std::fmt::Display for IntegerAffineWitnessError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for IntegerAffineWitnessError {}

/// Check the monotone or antitone image of one canonical root bound.
///
/// The caller must supply a root-bound proposition whose proof or citation was
/// checked independently. This function accepts no proof authority; it checks
/// only that the already-normalized ordered endpoint transform maps that exact
/// bound to the claimed target relation.
pub fn check_integer_affine_bound_conversion(
    form: &CheckedIntegerAffineForm,
    root_bound: &Proposition,
    conclusion: &Proposition,
) -> Result<(), IntegerAffineBoundConversionError> {
    let expected = map_integer_affine_bound(form, root_bound)?;
    if conclusion != &expected {
        return Err(IntegerAffineBoundConversionError::ConclusionMismatch);
    }
    Ok(())
}

/// Compute the unique endpoint relation established by one checked ordered
/// transform and one independently proved root bound.
pub fn map_integer_affine_bound(
    form: &CheckedIntegerAffineForm,
    root_bound: &Proposition,
) -> Result<Proposition, IntegerAffineBoundConversionError> {
    if matches!(
        form.endpoint_steps.as_slice(),
        [CheckedIntegerEndpointStep::CorrelatedAddLower]
            | [CheckedIntegerEndpointStep::CorrelatedAddUpper]
    ) {
        return map_correlated_add_bound(form, root_bound);
    }
    if matches!(
        form.endpoint_steps.as_slice(),
        [CheckedIntegerEndpointStep::CorrelatedSubtractLower]
            | [CheckedIntegerEndpointStep::CorrelatedSubtractUpper]
            | [CheckedIntegerEndpointStep::CorrelatedUnsignedSubtract]
    ) {
        return map_correlated_subtract_bound(form, root_bound);
    }
    if matches!(
        form.endpoint_steps.as_slice(),
        [CheckedIntegerEndpointStep::CorrelatedMultiplyMinimum]
            | [CheckedIntegerEndpointStep::CorrelatedMultiplyMaximum]
    ) {
        return map_correlated_multiply_bound(form, root_bound);
    }
    if form.endpoint_steps.is_empty() && matches!(form.target(), ScalarTerm::ExactIntegerAdd { .. })
    {
        return map_direct_add_bound(form, root_bound);
    }
    if form.endpoint_steps.is_empty()
        && matches!(form.target(), ScalarTerm::ExactIntegerSubtract { .. })
    {
        return map_direct_subtract_bound(form, root_bound);
    }
    if form.endpoint_steps.is_empty()
        && matches!(form.target(), ScalarTerm::ExactIntegerMultiply { .. })
    {
        return map_direct_multiply_bound(form, root_bound);
    }
    if form.endpoint_steps.is_empty()
        && let ScalarTerm::ExactIntegerShiftLeft {
            count_type, count, ..
        } = form.target()
    {
        return map_direct_shift_left_bound(form, *count_type, count, root_bound);
    }
    let Proposition::LessOrEqual(bound_left, bound_right) = root_bound else {
        return Err(IntegerAffineBoundConversionError::RootBoundNotLessOrEqual);
    };
    let (bound, root_is_lower_endpoint) = if bound_left == form.root() {
        (bound_right, false)
    } else if bound_right == form.root() {
        (bound_left, true)
    } else {
        return Err(IntegerAffineBoundConversionError::RootBoundMismatch);
    };
    let Some(bound) = integer_literal_as_i128(bound, form.integer_type()) else {
        return Err(IntegerAffineBoundConversionError::RootBoundNotTypedLiteral);
    };
    let mut mapped = bound;
    let mut reverses_order = false;
    for step in &form.endpoint_steps {
        mapped = match *step {
            CheckedIntegerEndpointStep::Add(value) => mapped.checked_add(value),
            CheckedIntegerEndpointStep::Subtract(value) => mapped.checked_sub(value),
            CheckedIntegerEndpointStep::Multiply(value) => {
                if value < 0 {
                    reverses_order = !reverses_order;
                }
                mapped.checked_mul(value)
            }
            CheckedIntegerEndpointStep::Divide(value) => {
                if value < 0 {
                    reverses_order = !reverses_order;
                }
                mapped.checked_div(value)
            }
            CheckedIntegerEndpointStep::Remainder(value) => {
                let magnitude = value
                    .checked_abs()
                    .ok_or(IntegerAffineBoundConversionError::MappedBoundOverflow)?;
                let current_is_lower = root_is_lower_endpoint ^ reverses_order;
                Some(
                    if current_is_lower && form.integer_type().sign() == IntegerSign::Signed {
                        1_i128
                            .checked_sub(magnitude)
                            .ok_or(IntegerAffineBoundConversionError::MappedBoundOverflow)?
                    } else if current_is_lower {
                        0
                    } else {
                        magnitude
                            .checked_sub(1)
                            .ok_or(IntegerAffineBoundConversionError::MappedBoundOverflow)?
                    },
                )
            }
            CheckedIntegerEndpointStep::ShiftLeft(count) => mapped.checked_mul(
                1_i128
                    .checked_shl(count)
                    .ok_or(IntegerAffineBoundConversionError::MappedBoundOverflow)?,
            ),
            CheckedIntegerEndpointStep::ShiftRight(count) => Some(mapped >> count),
            CheckedIntegerEndpointStep::CorrelatedAddLower
            | CheckedIntegerEndpointStep::CorrelatedAddUpper
            | CheckedIntegerEndpointStep::CorrelatedSubtractLower
            | CheckedIntegerEndpointStep::CorrelatedSubtractUpper
            | CheckedIntegerEndpointStep::CorrelatedUnsignedSubtract
            | CheckedIntegerEndpointStep::CorrelatedMultiplyMinimum
            | CheckedIntegerEndpointStep::CorrelatedMultiplyMaximum => {
                return Err(IntegerAffineBoundConversionError::DirectAddEvidenceMismatch);
            }
        }
        .ok_or(IntegerAffineBoundConversionError::MappedBoundOverflow)?;
    }
    let mapped_value = match form.integer_type().sign() {
        IntegerSign::Signed => IntegerValue::Signed(mapped),
        IntegerSign::Unsigned => IntegerValue::Unsigned(
            u128::try_from(mapped)
                .map_err(|_| IntegerAffineBoundConversionError::MappedBoundOutsideCarrier)?,
        ),
    };
    let mapped = ScalarTerm::integer(form.integer_type(), mapped_value)
        .map_err(|_| IntegerAffineBoundConversionError::MappedBoundOutsideCarrier)?;

    // Positive forms preserve order, negative forms reverse it. A constant
    // form can soundly provide either orientation; retaining the root bound's
    // orientation makes that choice deterministic.
    let target_is_left = if reverses_order {
        root_is_lower_endpoint
    } else {
        !root_is_lower_endpoint
    };
    Ok(if target_is_left {
        Proposition::LessOrEqual(form.target().clone(), mapped)
    } else {
        Proposition::LessOrEqual(mapped, form.target().clone())
    })
}

fn map_correlated_add_bound(
    form: &CheckedIntegerAffineForm,
    evidence: &Proposition,
) -> Result<Proposition, IntegerAffineBoundConversionError> {
    let ScalarTerm::ExactIntegerAdd { left, right, .. } = form.target() else {
        return Err(IntegerAffineBoundConversionError::DirectAddEvidenceMismatch);
    };
    let lower = matches!(
        form.endpoint_steps.as_slice(),
        [CheckedIntegerEndpointStep::CorrelatedAddLower]
    );
    let expected = if lower {
        Proposition::LessOrEqual(form.root().clone(), left.as_ref().clone())
    } else {
        Proposition::LessOrEqual(left.as_ref().clone(), form.root().clone())
    };
    if evidence != &expected {
        return Err(IntegerAffineBoundConversionError::DirectAddEvidenceMismatch);
    }
    let sum = IntegerMathTerm::Add(
        Box::new(
            direct_math_leaf(left, form.integer_type())
                .ok_or(IntegerAffineBoundConversionError::DirectAddEvidenceMismatch)?,
        ),
        Box::new(
            direct_math_leaf(right, form.integer_type())
                .ok_or(IntegerAffineBoundConversionError::DirectAddEvidenceMismatch)?,
        ),
    );
    Ok(if lower {
        Proposition::IntegerMathLessOrEqual(
            IntegerMathTerm::literal(form.integer_type().minimum_value()),
            sum,
        )
    } else {
        Proposition::IntegerMathLessOrEqual(
            sum,
            IntegerMathTerm::literal(form.integer_type().maximum_value()),
        )
    })
}

fn map_correlated_subtract_bound(
    form: &CheckedIntegerAffineForm,
    evidence: &Proposition,
) -> Result<Proposition, IntegerAffineBoundConversionError> {
    let ScalarTerm::ExactIntegerSubtract { left, right, .. } = form.target() else {
        return Err(IntegerAffineBoundConversionError::DirectSubtractEvidenceMismatch);
    };
    let lower = matches!(
        form.endpoint_steps.as_slice(),
        [CheckedIntegerEndpointStep::CorrelatedSubtractLower]
            | [CheckedIntegerEndpointStep::CorrelatedUnsignedSubtract]
    );
    let unsigned = matches!(
        form.endpoint_steps.as_slice(),
        [CheckedIntegerEndpointStep::CorrelatedUnsignedSubtract]
    );
    let expected = if unsigned {
        Proposition::LessOrEqual(right.as_ref().clone(), left.as_ref().clone())
    } else if lower {
        Proposition::LessOrEqual(form.root().clone(), left.as_ref().clone())
    } else {
        Proposition::LessOrEqual(left.as_ref().clone(), form.root().clone())
    };
    if evidence != &expected {
        return Err(IntegerAffineBoundConversionError::DirectSubtractEvidenceMismatch);
    }
    let difference = IntegerMathTerm::Subtract(
        Box::new(
            direct_math_leaf(left, form.integer_type())
                .ok_or(IntegerAffineBoundConversionError::DirectSubtractEvidenceMismatch)?,
        ),
        Box::new(
            direct_math_leaf(right, form.integer_type())
                .ok_or(IntegerAffineBoundConversionError::DirectSubtractEvidenceMismatch)?,
        ),
    );
    Ok(if lower {
        Proposition::IntegerMathLessOrEqual(
            IntegerMathTerm::literal(form.integer_type().minimum_value()),
            difference,
        )
    } else {
        Proposition::IntegerMathLessOrEqual(
            difference,
            IntegerMathTerm::literal(form.integer_type().maximum_value()),
        )
    })
}

fn map_correlated_multiply_bound(
    form: &CheckedIntegerAffineForm,
    evidence: &Proposition,
) -> Result<Proposition, IntegerAffineBoundConversionError> {
    let ScalarTerm::ExactIntegerMultiply { left, right, .. } = form.target() else {
        return Err(IntegerAffineBoundConversionError::DirectMultiplyEvidenceMismatch);
    };
    let Proposition::Conjunction(parts) = evidence else {
        return Err(IntegerAffineBoundConversionError::DirectMultiplyEvidenceMismatch);
    };
    let [sign_evidence, bound_evidence] = parts.as_slice() else {
        return Err(IntegerAffineBoundConversionError::DirectMultiplyEvidenceMismatch);
    };
    let one = ScalarTerm::integer(
        form.integer_type(),
        match form.integer_type().sign() {
            IntegerSign::Signed => IntegerValue::Signed(1),
            IntegerSign::Unsigned => IntegerValue::Unsigned(1),
        },
    )
    .map_err(|_| IntegerAffineBoundConversionError::DirectMultiplyEvidenceMismatch)?;
    let positive = sign_evidence == &Proposition::LessOrEqual(one, right.as_ref().clone());
    let negative_two = ScalarTerm::integer(form.integer_type(), IntegerValue::Signed(-2)).ok();
    let negative = negative_two.is_some_and(|negative_two| {
        sign_evidence == &Proposition::LessOrEqual(right.as_ref().clone(), negative_two)
    });
    if !positive && !negative {
        return Err(IntegerAffineBoundConversionError::DirectMultiplyEvidenceMismatch);
    }
    let endpoint_minimum = matches!(
        form.endpoint_steps.as_slice(),
        [CheckedIntegerEndpointStep::CorrelatedMultiplyMinimum]
    );
    let lower = endpoint_minimum;
    let expected_bound = if lower == positive {
        Proposition::LessOrEqual(form.root().clone(), left.as_ref().clone())
    } else {
        Proposition::LessOrEqual(left.as_ref().clone(), form.root().clone())
    };
    if bound_evidence != &expected_bound {
        return Err(IntegerAffineBoundConversionError::DirectMultiplyEvidenceMismatch);
    }
    let product = IntegerMathTerm::Multiply(
        Box::new(
            direct_math_leaf(left, form.integer_type())
                .ok_or(IntegerAffineBoundConversionError::DirectMultiplyEvidenceMismatch)?,
        ),
        Box::new(
            direct_math_leaf(right, form.integer_type())
                .ok_or(IntegerAffineBoundConversionError::DirectMultiplyEvidenceMismatch)?,
        ),
    );
    Ok(if lower {
        Proposition::IntegerMathLessOrEqual(
            IntegerMathTerm::literal(form.integer_type().minimum_value()),
            product,
        )
    } else {
        Proposition::IntegerMathLessOrEqual(
            product,
            IntegerMathTerm::literal(form.integer_type().maximum_value()),
        )
    })
}

fn map_direct_add_bound(
    form: &CheckedIntegerAffineForm,
    evidence: &Proposition,
) -> Result<Proposition, IntegerAffineBoundConversionError> {
    let Proposition::Conjunction(parts) = evidence else {
        return Err(IntegerAffineBoundConversionError::DirectAddEvidenceMismatch);
    };
    let [left_evidence, right_evidence] = parts.as_slice() else {
        return Err(IntegerAffineBoundConversionError::DirectAddEvidenceMismatch);
    };
    let ScalarTerm::ExactIntegerAdd {
        scalar_type,
        left,
        right,
    } = form.target()
    else {
        return Err(IntegerAffineBoundConversionError::DirectAddEvidenceMismatch);
    };
    if *scalar_type != form.integer_type() {
        return Err(IntegerAffineBoundConversionError::DirectAddEvidenceMismatch);
    }
    let left_endpoint = direct_operand_endpoint(left, left_evidence, form.integer_type())?;
    let right_endpoint = direct_operand_endpoint(right, right_evidence, form.integer_type())?;
    let lower = match (left_endpoint.orientation(), right_endpoint.orientation()) {
        (Some(left), Some(right)) if left == right => left,
        (None, Some(right)) => right,
        (Some(left), None) => left,
        (None, None) => return Err(IntegerAffineBoundConversionError::DirectAddEvidenceMismatch),
        _ => return Err(IntegerAffineBoundConversionError::DirectAddEvidenceMismatch),
    };
    let left_bound = left_endpoint.literal(form.integer_type(), lower);
    let right_bound = right_endpoint.literal(form.integer_type(), lower);
    let bound = add_math_literals(left_bound, right_bound)?;
    let sum = IntegerMathTerm::Add(
        Box::new(
            direct_math_leaf(left, form.integer_type())
                .ok_or(IntegerAffineBoundConversionError::DirectAddEvidenceMismatch)?,
        ),
        Box::new(
            direct_math_leaf(right, form.integer_type())
                .ok_or(IntegerAffineBoundConversionError::DirectAddEvidenceMismatch)?,
        ),
    );
    Ok(if lower {
        Proposition::IntegerMathLessOrEqual(IntegerMathTerm::IntegerLiteral(bound), sum)
    } else {
        Proposition::IntegerMathLessOrEqual(sum, IntegerMathTerm::IntegerLiteral(bound))
    })
}

fn map_direct_subtract_bound(
    form: &CheckedIntegerAffineForm,
    evidence: &Proposition,
) -> Result<Proposition, IntegerAffineBoundConversionError> {
    let Proposition::Conjunction(parts) = evidence else {
        return Err(IntegerAffineBoundConversionError::DirectSubtractEvidenceMismatch);
    };
    let [left_evidence, right_evidence] = parts.as_slice() else {
        return Err(IntegerAffineBoundConversionError::DirectSubtractEvidenceMismatch);
    };
    let ScalarTerm::ExactIntegerSubtract {
        scalar_type,
        left,
        right,
    } = form.target()
    else {
        return Err(IntegerAffineBoundConversionError::DirectSubtractEvidenceMismatch);
    };
    if *scalar_type != form.integer_type() {
        return Err(IntegerAffineBoundConversionError::DirectSubtractEvidenceMismatch);
    }
    let left_endpoint = direct_operand_endpoint(left, left_evidence, form.integer_type())?;
    let right_endpoint = direct_operand_endpoint(right, right_evidence, form.integer_type())?;
    let lower = match (left_endpoint.orientation(), right_endpoint.orientation()) {
        (Some(left), Some(right)) if left != right => left,
        (None, Some(right)) => !right,
        (Some(left), None) => left,
        (None, None)
            if matches!(
                (&left_endpoint, &right_endpoint),
                (DirectAddEndpoint::Exact(left), DirectAddEndpoint::Carrier)
                    if left.as_integer_value(form.integer_type())
                        == Some(form.integer_type().maximum_value())
            ) =>
        {
            true
        }
        (None, None)
            if matches!(
                (&left_endpoint, &right_endpoint),
                (DirectAddEndpoint::Exact(left), DirectAddEndpoint::Carrier)
                    if left.as_integer_value(form.integer_type())
                        == Some(form.integer_type().minimum_value())
            ) =>
        {
            false
        }
        (None, None) => {
            return Err(IntegerAffineBoundConversionError::DirectSubtractEvidenceMismatch);
        }
        _ => return Err(IntegerAffineBoundConversionError::DirectSubtractEvidenceMismatch),
    };
    let left_bound = left_endpoint.literal(form.integer_type(), lower);
    let right_bound = right_endpoint.literal(form.integer_type(), !lower);
    let bound = subtract_math_literals(left_bound, right_bound)?;
    let difference = IntegerMathTerm::Subtract(
        Box::new(
            direct_math_leaf(left, form.integer_type())
                .ok_or(IntegerAffineBoundConversionError::DirectSubtractEvidenceMismatch)?,
        ),
        Box::new(
            direct_math_leaf(right, form.integer_type())
                .ok_or(IntegerAffineBoundConversionError::DirectSubtractEvidenceMismatch)?,
        ),
    );
    Ok(if lower {
        Proposition::IntegerMathLessOrEqual(IntegerMathTerm::IntegerLiteral(bound), difference)
    } else {
        Proposition::IntegerMathLessOrEqual(difference, IntegerMathTerm::IntegerLiteral(bound))
    })
}

fn map_direct_multiply_bound(
    form: &CheckedIntegerAffineForm,
    evidence: &Proposition,
) -> Result<Proposition, IntegerAffineBoundConversionError> {
    let Proposition::Conjunction(parts) = evidence else {
        return Err(IntegerAffineBoundConversionError::DirectMultiplyEvidenceMismatch);
    };
    let [left_first, left_second, right_first, right_second] = parts.as_slice() else {
        return Err(IntegerAffineBoundConversionError::DirectMultiplyEvidenceMismatch);
    };
    let ScalarTerm::ExactIntegerMultiply {
        scalar_type,
        left,
        right,
    } = form.target()
    else {
        return Err(IntegerAffineBoundConversionError::DirectMultiplyEvidenceMismatch);
    };
    if *scalar_type != form.integer_type() {
        return Err(IntegerAffineBoundConversionError::DirectMultiplyEvidenceMismatch);
    }
    let left_first = direct_operand_endpoint(left, left_first, form.integer_type())?;
    let left_second = direct_operand_endpoint(left, left_second, form.integer_type())?;
    let right_first = direct_operand_endpoint(right, right_first, form.integer_type())?;
    let right_second = direct_operand_endpoint(right, right_second, form.integer_type())?;
    let pair_orientation = |first: &DirectAddEndpoint,
                            second: &DirectAddEndpoint|
     -> Result<Option<bool>, IntegerAffineBoundConversionError> {
        match (first.orientation(), second.orientation()) {
            (Some(first), Some(second)) if first != second => Ok(Some(first)),
            (Some(first), None) if matches!(second, DirectAddEndpoint::Carrier) => Ok(Some(first)),
            (None, Some(second)) if matches!(first, DirectAddEndpoint::Carrier) => {
                Ok(Some(!second))
            }
            (None, None)
                if matches!(
                    (first, second),
                    (DirectAddEndpoint::Exact(left), DirectAddEndpoint::Exact(right)) if left == right
                ) || matches!(
                    (first, second),
                    (DirectAddEndpoint::Carrier, DirectAddEndpoint::Carrier)
                ) =>
            {
                Ok(None)
            }
            _ => Err(IntegerAffineBoundConversionError::DirectMultiplyEvidenceMismatch),
        }
    };
    let left_orientation = pair_orientation(&left_first, &left_second)?;
    let right_orientation = pair_orientation(&right_first, &right_second)?;
    let lower = match (left_orientation, right_orientation) {
        (Some(left), Some(right)) if left == right => left,
        (Some(left), None) => left,
        (None, Some(right)) => right,
        _ => return Err(IntegerAffineBoundConversionError::DirectMultiplyEvidenceMismatch),
    };
    let (left_lower, left_upper, right_lower, right_upper) = if lower {
        (
            left_first.literal(form.integer_type(), true),
            left_second.literal(form.integer_type(), false),
            right_first.literal(form.integer_type(), true),
            right_second.literal(form.integer_type(), false),
        )
    } else {
        (
            left_second.literal(form.integer_type(), true),
            left_first.literal(form.integer_type(), false),
            right_second.literal(form.integer_type(), true),
            right_first.literal(form.integer_type(), false),
        )
    };
    let products = [
        multiply_math_literals(left_lower, right_lower)?,
        multiply_math_literals(left_lower, right_upper)?,
        multiply_math_literals(left_upper, right_lower)?,
        multiply_math_literals(left_upper, right_upper)?,
    ];
    let bound = products
        .into_iter()
        .reduce(|current, candidate| {
            if (lower && math_literal_less(candidate, current))
                || (!lower && math_literal_less(current, candidate))
            {
                candidate
            } else {
                current
            }
        })
        .expect("four product corners");
    let product = IntegerMathTerm::Multiply(
        Box::new(
            direct_math_leaf(left, form.integer_type())
                .ok_or(IntegerAffineBoundConversionError::DirectMultiplyEvidenceMismatch)?,
        ),
        Box::new(
            direct_math_leaf(right, form.integer_type())
                .ok_or(IntegerAffineBoundConversionError::DirectMultiplyEvidenceMismatch)?,
        ),
    );
    Ok(if lower {
        Proposition::IntegerMathLessOrEqual(IntegerMathTerm::IntegerLiteral(bound), product)
    } else {
        Proposition::IntegerMathLessOrEqual(product, IntegerMathTerm::IntegerLiteral(bound))
    })
}

enum DirectAddEndpoint {
    Exact(semantic_vocabulary::IntegerMathLiteral),
    Oriented {
        literal: semantic_vocabulary::IntegerMathLiteral,
        lower: bool,
    },
    Carrier,
}

impl DirectAddEndpoint {
    fn orientation(&self) -> Option<bool> {
        match self {
            Self::Oriented { lower, .. } => Some(*lower),
            Self::Exact(_) | Self::Carrier => None,
        }
    }

    fn literal(
        &self,
        integer_type: IntegerType,
        lower: bool,
    ) -> semantic_vocabulary::IntegerMathLiteral {
        match self {
            Self::Exact(literal) | Self::Oriented { literal, .. } => *literal,
            Self::Carrier => {
                semantic_vocabulary::IntegerMathLiteral::from_integer_value(if lower {
                    integer_type.minimum_value()
                } else {
                    integer_type.maximum_value()
                })
            }
        }
    }
}

fn direct_operand_endpoint(
    operand: &ScalarTerm,
    evidence: &Proposition,
    integer_type: IntegerType,
) -> Result<DirectAddEndpoint, IntegerAffineBoundConversionError> {
    if let ScalarTerm::Integer { scalar_type, value } = operand {
        if *scalar_type != integer_type || evidence != &Proposition::Truth {
            return Err(IntegerAffineBoundConversionError::DirectAddEvidenceMismatch);
        }
        return Ok(DirectAddEndpoint::Exact(
            semantic_vocabulary::IntegerMathLiteral::from_integer_value(*value),
        ));
    }
    if evidence == &Proposition::Truth {
        return Ok(DirectAddEndpoint::Carrier);
    }
    if let Proposition::Equal(left, right) = evidence {
        let literal = if left == operand {
            right
        } else if right == operand {
            left
        } else {
            return Err(IntegerAffineBoundConversionError::DirectAddEvidenceMismatch);
        };
        let (actual, value) = literal
            .integer_value()
            .ok_or(IntegerAffineBoundConversionError::DirectAddEvidenceMismatch)?;
        if actual != integer_type {
            return Err(IntegerAffineBoundConversionError::DirectAddEvidenceMismatch);
        }
        return Ok(DirectAddEndpoint::Exact(
            semantic_vocabulary::IntegerMathLiteral::from_integer_value(value),
        ));
    }
    let Proposition::LessOrEqual(left, right) = evidence else {
        return Err(IntegerAffineBoundConversionError::DirectAddEvidenceMismatch);
    };
    if left == right {
        return Err(IntegerAffineBoundConversionError::DirectAddEvidenceMismatch);
    }
    let (bound, lower) = if right == operand {
        (left, true)
    } else if left == operand {
        (right, false)
    } else {
        return Err(IntegerAffineBoundConversionError::DirectAddEvidenceMismatch);
    };
    let (actual, value) = bound
        .integer_value()
        .ok_or(IntegerAffineBoundConversionError::DirectAddEvidenceMismatch)?;
    if actual != integer_type {
        return Err(IntegerAffineBoundConversionError::DirectAddEvidenceMismatch);
    }
    Ok(DirectAddEndpoint::Oriented {
        literal: semantic_vocabulary::IntegerMathLiteral::from_integer_value(value),
        lower,
    })
}

fn add_math_literals(
    left: semantic_vocabulary::IntegerMathLiteral,
    right: semantic_vocabulary::IntegerMathLiteral,
) -> Result<semantic_vocabulary::IntegerMathLiteral, IntegerAffineBoundConversionError> {
    let (negative, magnitude) = if left.negative() == right.negative() {
        (
            left.negative(),
            left.magnitude()
                .checked_add(right.magnitude())
                .ok_or(IntegerAffineBoundConversionError::DirectAddBoundOverflow)?,
        )
    } else {
        match left.magnitude().cmp(&right.magnitude()) {
            std::cmp::Ordering::Less => (right.negative(), right.magnitude() - left.magnitude()),
            std::cmp::Ordering::Equal => (false, 0),
            std::cmp::Ordering::Greater => (left.negative(), left.magnitude() - right.magnitude()),
        }
    };
    semantic_vocabulary::IntegerMathLiteral::new(negative, magnitude)
        .map_err(|_| IntegerAffineBoundConversionError::DirectAddBoundOverflow)
}

fn subtract_math_literals(
    left: semantic_vocabulary::IntegerMathLiteral,
    right: semantic_vocabulary::IntegerMathLiteral,
) -> Result<semantic_vocabulary::IntegerMathLiteral, IntegerAffineBoundConversionError> {
    let (negative, magnitude) = if left.negative() != right.negative() {
        (
            left.negative(),
            left.magnitude()
                .checked_add(right.magnitude())
                .ok_or(IntegerAffineBoundConversionError::DirectSubtractBoundOverflow)?,
        )
    } else {
        match left.magnitude().cmp(&right.magnitude()) {
            std::cmp::Ordering::Less => (!left.negative(), right.magnitude() - left.magnitude()),
            std::cmp::Ordering::Equal => (false, 0),
            std::cmp::Ordering::Greater => (left.negative(), left.magnitude() - right.magnitude()),
        }
    };
    semantic_vocabulary::IntegerMathLiteral::new(negative, magnitude)
        .map_err(|_| IntegerAffineBoundConversionError::DirectSubtractBoundOverflow)
}

fn multiply_math_literals(
    left: semantic_vocabulary::IntegerMathLiteral,
    right: semantic_vocabulary::IntegerMathLiteral,
) -> Result<semantic_vocabulary::IntegerMathLiteral, IntegerAffineBoundConversionError> {
    let magnitude = left
        .magnitude()
        .checked_mul(right.magnitude())
        .ok_or(IntegerAffineBoundConversionError::DirectMultiplyBoundOverflow)?;
    semantic_vocabulary::IntegerMathLiteral::new(
        magnitude != 0 && left.negative() != right.negative(),
        magnitude,
    )
    .map_err(|_| IntegerAffineBoundConversionError::DirectMultiplyBoundOverflow)
}

fn math_literal_less(
    left: semantic_vocabulary::IntegerMathLiteral,
    right: semantic_vocabulary::IntegerMathLiteral,
) -> bool {
    match (left.negative(), right.negative()) {
        (true, false) => true,
        (false, true) => false,
        (true, true) => left.magnitude() > right.magnitude(),
        (false, false) => left.magnitude() < right.magnitude(),
    }
}

fn direct_math_leaf(term: &ScalarTerm, expected: IntegerType) -> Option<IntegerMathTerm> {
    match term {
        ScalarTerm::Value {
            id,
            scalar_type: ScalarType::Integer(actual),
        } if *actual == expected => Some(IntegerMathTerm::MathValue {
            source_type: expected,
            value: *id,
        }),
        ScalarTerm::Integer { scalar_type, value } if *scalar_type == expected => {
            Some(IntegerMathTerm::literal(*value))
        }
        _ => None,
    }
}

fn map_direct_shift_left_bound(
    form: &CheckedIntegerAffineForm,
    count_type: IntegerType,
    count: &ScalarTerm,
    evidence: &Proposition,
) -> Result<Proposition, IntegerAffineBoundConversionError> {
    let parts = match evidence {
        Proposition::Conjunction(parts) => parts.as_slice(),
        proposition => std::slice::from_ref(proposition),
    };
    let mut root_bound = None;
    let mut exact_count = None;
    let mut count_lower = count_type.sign() == IntegerSign::Unsigned;
    let mut count_upper = None;
    for part in parts {
        match part {
            Proposition::LessOrEqual(left, right)
                if left == form.root() || right == form.root() =>
            {
                if root_bound.replace(part).is_some() {
                    return Err(IntegerAffineBoundConversionError::AmbiguousDirectShiftEvidence);
                }
            }
            Proposition::Equal(left, right) if left == count || right == count => {
                let literal = if left == count { right } else { left };
                let (actual, value) = literal
                    .integer_value()
                    .ok_or(IntegerAffineBoundConversionError::DirectShiftCountEvidenceMismatch)?;
                if actual != count_type || exact_count.replace(value).is_some() {
                    return Err(IntegerAffineBoundConversionError::AmbiguousDirectShiftEvidence);
                }
            }
            Proposition::LessOrEqual(left, right) if right == count => {
                let (actual, value) = left
                    .integer_value()
                    .ok_or(IntegerAffineBoundConversionError::DirectShiftCountEvidenceMismatch)?;
                if actual == count_type && integer_value_to_u128(value) == Some(0) {
                    count_lower = true;
                }
            }
            Proposition::LessOrEqual(left, right) if left == count => {
                let (actual, value) = right
                    .integer_value()
                    .ok_or(IntegerAffineBoundConversionError::DirectShiftCountEvidenceMismatch)?;
                if actual != count_type || count_upper.replace(value).is_some() {
                    return Err(IntegerAffineBoundConversionError::AmbiguousDirectShiftEvidence);
                }
            }
            _ => {
                return Err(IntegerAffineBoundConversionError::DirectShiftEvidenceMismatch);
            }
        }
    }
    let root_bound =
        root_bound.ok_or(IntegerAffineBoundConversionError::DirectShiftRootBoundMissing)?;
    let (minimum_count, maximum_count) = if let Some((actual, embedded)) = count.integer_value() {
        if actual != count_type || exact_count.is_some() || count_upper.is_some() {
            return Err(IntegerAffineBoundConversionError::DirectShiftCountEvidenceMismatch);
        }
        (embedded, embedded)
    } else if let Some(exact) = exact_count {
        if count_upper.is_some() {
            return Err(IntegerAffineBoundConversionError::AmbiguousDirectShiftEvidence);
        }
        (exact, exact)
    } else if let Some(upper) = count_upper {
        if !count_lower {
            return Err(IntegerAffineBoundConversionError::DirectShiftCountLowerMissing);
        }
        let zero = match count_type.sign() {
            IntegerSign::Signed => IntegerValue::Signed(0),
            IntegerSign::Unsigned => IntegerValue::Unsigned(0),
        };
        (zero, upper)
    } else {
        if !count_lower {
            return Err(IntegerAffineBoundConversionError::DirectShiftCountLowerMissing);
        }
        let zero = match count_type.sign() {
            IntegerSign::Signed => IntegerValue::Signed(0),
            IntegerSign::Unsigned => IntegerValue::Unsigned(0),
        };
        (zero, count_type.maximum_value())
    };
    let minimum_count = integer_value_to_u128(minimum_count)
        .and_then(|count| u32::try_from(count).ok())
        .filter(|count| *count < u32::from(form.integer_type().bits()))
        .ok_or(IntegerAffineBoundConversionError::DirectShiftCountOutsideValueWidth)?;
    let maximum_count = integer_value_to_u128(maximum_count)
        .and_then(|count| u32::try_from(count).ok())
        .filter(|count| *count < u32::from(form.integer_type().bits()))
        .ok_or(IntegerAffineBoundConversionError::DirectShiftCountOutsideValueWidth)?;
    if minimum_count > maximum_count {
        return Err(IntegerAffineBoundConversionError::DirectShiftCountEvidenceMismatch);
    }

    let Proposition::LessOrEqual(bound_left, bound_right) = root_bound else {
        return Err(IntegerAffineBoundConversionError::RootBoundNotLessOrEqual);
    };
    let (bound, root_is_lower_endpoint) = if bound_left == form.root() {
        (bound_right, false)
    } else if bound_right == form.root() {
        (bound_left, true)
    } else {
        return Err(IntegerAffineBoundConversionError::RootBoundMismatch);
    };
    let (actual_type, bound) = bound
        .integer_value()
        .ok_or(IntegerAffineBoundConversionError::RootBoundNotTypedLiteral)?;
    if actual_type != form.integer_type() {
        return Err(IntegerAffineBoundConversionError::RootBoundNotTypedLiteral);
    }
    let negative = matches!(bound, IntegerValue::Signed(value) if value < 0);
    let count = match (root_is_lower_endpoint, negative) {
        (true, true) | (false, false) => maximum_count,
        (true, false) | (false, true) => minimum_count,
    };
    let shifted_bound = shift_math_literal(bound, count)?;
    let shifted = direct_shift_math_term(form)?;
    Ok(if root_is_lower_endpoint {
        Proposition::IntegerMathLessOrEqual(IntegerMathTerm::IntegerLiteral(shifted_bound), shifted)
    } else {
        Proposition::IntegerMathLessOrEqual(shifted, IntegerMathTerm::IntegerLiteral(shifted_bound))
    })
}

fn integer_value_to_u128(value: IntegerValue) -> Option<u128> {
    match value {
        IntegerValue::Signed(value) => u128::try_from(value).ok(),
        IntegerValue::Unsigned(value) => Some(value),
    }
}

fn shift_math_literal(
    value: IntegerValue,
    count: u32,
) -> Result<semantic_vocabulary::IntegerMathLiteral, IntegerAffineBoundConversionError> {
    let literal = semantic_vocabulary::IntegerMathLiteral::from_integer_value(value);
    let magnitude = literal
        .magnitude()
        .checked_shl(count)
        .ok_or(IntegerAffineBoundConversionError::MappedBoundOverflow)?;
    semantic_vocabulary::IntegerMathLiteral::new(literal.negative(), magnitude)
        .map_err(|_| IntegerAffineBoundConversionError::MappedBoundOverflow)
}

fn direct_shift_math_term(
    form: &CheckedIntegerAffineForm,
) -> Result<IntegerMathTerm, IntegerAffineBoundConversionError> {
    let ScalarTerm::ExactIntegerShiftLeft {
        value_type,
        count_type,
        value,
        count,
    } = form.target()
    else {
        return Err(IntegerAffineBoundConversionError::DirectShiftEvidenceMismatch);
    };
    let lift = |term: &ScalarTerm, expected: IntegerType| match term {
        ScalarTerm::Value {
            id,
            scalar_type: ScalarType::Integer(actual),
        } if *actual == expected => Some(IntegerMathTerm::MathValue {
            source_type: expected,
            value: *id,
        }),
        ScalarTerm::Integer { scalar_type, value } if *scalar_type == expected => {
            Some(IntegerMathTerm::literal(*value))
        }
        _ => None,
    };
    Ok(IntegerMathTerm::ShiftLeft {
        value: Box::new(
            lift(value, *value_type)
                .ok_or(IntegerAffineBoundConversionError::DirectShiftEvidenceMismatch)?,
        ),
        count: Box::new(
            lift(count, *count_type)
                .ok_or(IntegerAffineBoundConversionError::DirectShiftEvidenceMismatch)?,
        ),
    })
}

/// Derive the two carrier-tight endpoint relations for a checked word whose
/// landed nonzero exact divide or remainder has a total full-carrier image,
/// making its output range independent of the incoming root value. The caller
/// still supplies and recursively checks a `Truth` child so ordinary
/// proof-node custody remains explicit.
pub fn integer_affine_truth_bounds(
    form: &CheckedIntegerAffineForm,
) -> Result<Vec<Proposition>, IntegerAffineBoundConversionError> {
    let mut minimum = integer_value_to_i128(form.integer_type().minimum_value())
        .ok_or(IntegerAffineBoundConversionError::MappedBoundOutsideCarrier)?;
    let mut maximum = integer_value_to_i128(form.integer_type().maximum_value())
        .ok_or(IntegerAffineBoundConversionError::MappedBoundOutsideCarrier)?;
    let carrier_minimum = minimum;
    let carrier_maximum = maximum;
    let mut saw_total_image = false;
    for step in &form.endpoint_steps {
        match *step {
            CheckedIntegerEndpointStep::Add(value) => {
                minimum = minimum
                    .checked_add(value)
                    .ok_or(IntegerAffineBoundConversionError::MappedBoundOverflow)?;
                maximum = maximum
                    .checked_add(value)
                    .ok_or(IntegerAffineBoundConversionError::MappedBoundOverflow)?;
            }
            CheckedIntegerEndpointStep::Subtract(value) => {
                minimum = minimum
                    .checked_sub(value)
                    .ok_or(IntegerAffineBoundConversionError::MappedBoundOverflow)?;
                maximum = maximum
                    .checked_sub(value)
                    .ok_or(IntegerAffineBoundConversionError::MappedBoundOverflow)?;
            }
            CheckedIntegerEndpointStep::Multiply(value) => {
                let left = minimum
                    .checked_mul(value)
                    .ok_or(IntegerAffineBoundConversionError::MappedBoundOverflow)?;
                let right = maximum
                    .checked_mul(value)
                    .ok_or(IntegerAffineBoundConversionError::MappedBoundOverflow)?;
                (minimum, maximum) = (left.min(right), left.max(right));
                if value == 0 {
                    saw_total_image = true;
                }
            }
            CheckedIntegerEndpointStep::Divide(value) => {
                if form.integer_type().sign() == IntegerSign::Signed && value == -1 {
                    return Err(IntegerAffineBoundConversionError::NonTotalDivisionImage);
                }
                let left = carrier_minimum
                    .checked_div(value)
                    .ok_or(IntegerAffineBoundConversionError::MappedBoundOverflow)?;
                let right = carrier_maximum
                    .checked_div(value)
                    .ok_or(IntegerAffineBoundConversionError::MappedBoundOverflow)?;
                (minimum, maximum) = (left.min(right), left.max(right));
                saw_total_image = true;
            }
            CheckedIntegerEndpointStep::Remainder(value) => {
                let magnitude = value
                    .checked_abs()
                    .ok_or(IntegerAffineBoundConversionError::MappedBoundOverflow)?;
                minimum = if form.integer_type().sign() == IntegerSign::Signed {
                    1_i128
                        .checked_sub(magnitude)
                        .ok_or(IntegerAffineBoundConversionError::MappedBoundOverflow)?
                } else {
                    0
                };
                maximum = magnitude
                    .checked_sub(1)
                    .ok_or(IntegerAffineBoundConversionError::MappedBoundOverflow)?;
                saw_total_image = true;
            }
            CheckedIntegerEndpointStep::ShiftLeft(count) => {
                if count == 0 {
                    minimum = carrier_minimum;
                    maximum = carrier_maximum;
                    saw_total_image = true;
                    continue;
                }
                let scale = 1_i128
                    .checked_shl(count)
                    .ok_or(IntegerAffineBoundConversionError::MappedBoundOverflow)?;
                minimum = minimum
                    .checked_mul(scale)
                    .ok_or(IntegerAffineBoundConversionError::MappedBoundOverflow)?;
                maximum = maximum
                    .checked_mul(scale)
                    .ok_or(IntegerAffineBoundConversionError::MappedBoundOverflow)?;
            }
            CheckedIntegerEndpointStep::ShiftRight(count) => {
                minimum = carrier_minimum >> count;
                maximum = carrier_maximum >> count;
                saw_total_image = true;
            }
            CheckedIntegerEndpointStep::CorrelatedAddLower
            | CheckedIntegerEndpointStep::CorrelatedAddUpper
            | CheckedIntegerEndpointStep::CorrelatedSubtractLower
            | CheckedIntegerEndpointStep::CorrelatedSubtractUpper
            | CheckedIntegerEndpointStep::CorrelatedUnsignedSubtract
            | CheckedIntegerEndpointStep::CorrelatedMultiplyMinimum
            | CheckedIntegerEndpointStep::CorrelatedMultiplyMaximum => {
                return Err(IntegerAffineBoundConversionError::TruthRootWithoutTotalImage);
            }
        }
    }
    if !saw_total_image {
        return Err(IntegerAffineBoundConversionError::TruthRootWithoutTotalImage);
    }
    let minimum = scalar_integer_from_i128(form.integer_type(), minimum)?;
    let maximum = scalar_integer_from_i128(form.integer_type(), maximum)?;
    Ok(vec![
        Proposition::LessOrEqual(minimum, form.target().clone()),
        Proposition::LessOrEqual(form.target().clone(), maximum),
    ])
}

fn scalar_integer_from_i128(
    integer_type: IntegerType,
    value: i128,
) -> Result<ScalarTerm, IntegerAffineBoundConversionError> {
    let value = match integer_type.sign() {
        IntegerSign::Signed => IntegerValue::Signed(value),
        IntegerSign::Unsigned => IntegerValue::Unsigned(
            u128::try_from(value)
                .map_err(|_| IntegerAffineBoundConversionError::MappedBoundOutsideCarrier)?,
        ),
    };
    ScalarTerm::integer(integer_type, value)
        .map_err(|_| IntegerAffineBoundConversionError::MappedBoundOutsideCarrier)
}

fn integer_value_to_i128(value: IntegerValue) -> Option<i128> {
    match value {
        IntegerValue::Signed(value) => Some(value),
        IntegerValue::Unsigned(value) => i128::try_from(value).ok(),
    }
}

fn integer_literal_as_i128(term: &ScalarTerm, integer_type: IntegerType) -> Option<i128> {
    let (actual_type, value) = term.integer_value()?;
    if actual_type != integer_type {
        return None;
    }
    match value {
        IntegerValue::Signed(value) => Some(value),
        IntegerValue::Unsigned(value) => i128::try_from(value).ok(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IntegerAffineBoundConversionError {
    RootBoundNotLessOrEqual,
    RootBoundMismatch,
    RootBoundNotTypedLiteral,
    MappedBoundOverflow,
    MappedBoundOutsideCarrier,
    TruthRootWithoutTotalImage,
    NonTotalDivisionImage,
    DirectShiftEvidenceMismatch,
    DirectShiftRootBoundMissing,
    DirectShiftCountEvidenceMismatch,
    DirectShiftCountLowerMissing,
    DirectShiftCountOutsideValueWidth,
    AmbiguousDirectShiftEvidence,
    DirectAddEvidenceMismatch,
    DirectAddBoundOverflow,
    DirectSubtractEvidenceMismatch,
    DirectSubtractBoundOverflow,
    DirectMultiplyEvidenceMismatch,
    DirectMultiplyBoundOverflow,
    ConclusionMismatch,
}

impl std::fmt::Display for IntegerAffineBoundConversionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for IntegerAffineBoundConversionError {}

#[cfg(test)]
mod tests {
    use super::*;
    use semantic_vocabulary::{IntegerSign, ValueId};

    fn value(id: u64, integer_type: IntegerType) -> ScalarTerm {
        ScalarTerm::value(
            ValueId::new(id).expect("value"),
            ScalarType::Integer(integer_type),
        )
    }

    fn literal(integer_type: IntegerType, value: i128) -> ScalarTerm {
        ScalarTerm::integer(integer_type, IntegerValue::Signed(value)).expect("literal")
    }

    #[test]
    fn checks_ordered_add_subtract_multiply_normalization() {
        let integer_type = IntegerType::new(IntegerSign::Signed, 16).expect("i16");
        let root = value(1, integer_type);
        let added = value(2, integer_type);
        let subtracted = value(3, integer_type);
        let target = value(4, integer_type);
        let axioms = vec![
            Proposition::Equal(
                added.clone(),
                ScalarTerm::exact_integer_add(integer_type, root.clone(), literal(integer_type, 7))
                    .unwrap(),
            ),
            Proposition::Equal(
                ScalarTerm::exact_integer_subtract(
                    integer_type,
                    added.clone(),
                    literal(integer_type, 2),
                )
                .unwrap(),
                subtracted.clone(),
            ),
            Proposition::Equal(
                target.clone(),
                ScalarTerm::exact_integer_multiply(
                    integer_type,
                    literal(integer_type, -3),
                    subtracted,
                )
                .unwrap(),
            ),
        ];
        let context = PropositionContext::from_value_types(
            (1..=4).map(|id| (ValueId::new(id).unwrap(), ScalarType::Integer(integer_type))),
        )
        .unwrap();
        let checked = check_integer_affine_witness(
            &context,
            &axioms,
            &IntegerAffineWitness {
                root: root.clone(),
                target: target.clone(),
                definition_axioms: vec![0, 1, 2],
                literal_axioms: vec![None, None, None],
            },
        )
        .expect("ordered affine witness");
        assert_eq!(checked.root(), &root);
        assert_eq!(checked.target(), &target);
        assert_eq!(checked.coefficient(), -3);
        assert_eq!(checked.offset(), -15);
    }

    #[test]
    fn checks_landed_literal_sibling_and_rejects_stale_or_unused_custody() {
        let integer_type = IntegerType::new(IntegerSign::Signed, 16).expect("i16");
        let root = value(1, integer_type);
        let sibling = value(2, integer_type);
        let target = value(3, integer_type);
        let context = PropositionContext::from_value_types(
            (1..=5).map(|id| (ValueId::new(id).unwrap(), ScalarType::Integer(integer_type))),
        )
        .unwrap();
        let landing = Proposition::Equal(sibling.clone(), literal(integer_type, 7));
        let definition = Proposition::Equal(
            target.clone(),
            ScalarTerm::exact_integer_add(integer_type, root.clone(), sibling.clone()).unwrap(),
        );
        let witness = IntegerAffineWitness {
            root: root.clone(),
            target: target.clone(),
            definition_axioms: vec![1],
            literal_axioms: vec![Some(0)],
        };

        let checked = check_integer_affine_witness(
            &context,
            &[landing.clone(), definition.clone()],
            &witness,
        )
        .expect("an earlier exact landing supplies the affine sibling literal");
        assert_eq!(checked.coefficient(), 1);
        assert_eq!(checked.offset(), 7);

        let missing_alignment = IntegerAffineWitness {
            literal_axioms: Vec::new(),
            ..witness.clone()
        };
        assert_eq!(
            check_integer_affine_witness(
                &context,
                &[landing.clone(), definition.clone()],
                &missing_alignment,
            ),
            Err(IntegerAffineWitnessError::LiteralAxiomCountMismatch),
        );

        let late_landing = IntegerAffineWitness {
            definition_axioms: vec![0],
            literal_axioms: vec![Some(1)],
            ..witness.clone()
        };
        assert_eq!(
            check_integer_affine_witness(&context, &[definition, landing], &late_landing),
            Err(IntegerAffineWitnessError::LiteralAxiomNotPrior {
                definition: 0,
                literal: 1,
            }),
        );

        let inline_definition = Proposition::Equal(
            target,
            ScalarTerm::exact_integer_add(integer_type, root, literal(integer_type, 7)).unwrap(),
        );
        assert_eq!(
            check_integer_affine_witness(
                &context,
                &[
                    Proposition::Equal(sibling, literal(integer_type, 7)),
                    inline_definition
                ],
                &witness,
            ),
            Err(IntegerAffineWitnessError::UnusedLiteralAxiom(1)),
        );
    }

    #[test]
    fn rejects_reordered_stale_and_non_affine_definitions() {
        let integer_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let root = value(1, integer_type);
        let target = value(2, integer_type);
        let context = PropositionContext::from_value_types([
            (ValueId::new(1).unwrap(), ScalarType::Integer(integer_type)),
            (ValueId::new(2).unwrap(), ScalarType::Integer(integer_type)),
        ])
        .unwrap();
        let axiom = Proposition::Equal(
            target.clone(),
            ScalarTerm::exact_integer_add(integer_type, root.clone(), literal(integer_type, 1))
                .unwrap(),
        );
        let witness = |definition_axioms: Vec<usize>| IntegerAffineWitness {
            root: root.clone(),
            target: target.clone(),
            literal_axioms: vec![None; definition_axioms.len()],
            definition_axioms,
        };
        assert_eq!(
            check_integer_affine_witness(&context, std::slice::from_ref(&axiom), &witness(vec![1])),
            Err(IntegerAffineWitnessError::UnknownSemanticAxiom(1)),
        );
        assert_eq!(
            check_integer_affine_witness(&context, &[axiom.clone(), axiom], &witness(vec![1, 0])),
            Err(IntegerAffineWitnessError::NonCanonicalDefinitionOrder),
        );
        assert_eq!(
            check_integer_affine_witness(
                &context,
                &[Proposition::Equal(target.clone(), root.clone())],
                &witness(vec![0]),
            ),
            Err(IntegerAffineWitnessError::DefinitionShapeMismatch(0)),
        );
    }

    #[test]
    fn rejects_non_value_roots_stale_unsigned_words_and_checked_overflow() {
        let i8_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let i8_target = value(1, i8_type);
        let i8_context = PropositionContext::from_value_types([(
            ValueId::new(1).unwrap(),
            ScalarType::Integer(i8_type),
        )])
        .unwrap();
        assert_eq!(
            check_integer_affine_witness(
                &i8_context,
                &[Proposition::Equal(
                    i8_target.clone(),
                    ScalarTerm::exact_integer_add(
                        i8_type,
                        literal(i8_type, 0),
                        literal(i8_type, 1),
                    )
                    .unwrap(),
                )],
                &IntegerAffineWitness {
                    root: literal(i8_type, 0),
                    target: i8_target,
                    definition_axioms: vec![0],
                    literal_axioms: vec![None],
                },
            ),
            Err(IntegerAffineWitnessError::RootNotValue),
        );

        let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let u8_root = value(1, u8_type);
        let u8_target = value(2, u8_type);
        let u8_context = PropositionContext::from_value_types([
            (ValueId::new(1).unwrap(), ScalarType::Integer(u8_type)),
            (ValueId::new(2).unwrap(), ScalarType::Integer(u8_type)),
        ])
        .unwrap();
        assert_eq!(
            check_integer_affine_witness(
                &u8_context,
                &[],
                &IntegerAffineWitness {
                    root: u8_root,
                    target: u8_target,
                    definition_axioms: vec![0],
                    literal_axioms: vec![None],
                },
            ),
            Err(IntegerAffineWitnessError::UnknownSemanticAxiom(0)),
        );

        let i128_type = IntegerType::new(IntegerSign::Signed, 128).expect("i128");
        let root = value(1, i128_type);
        let intermediate = value(2, i128_type);
        let target = value(3, i128_type);
        let context = PropositionContext::from_value_types(
            (1..=3).map(|id| (ValueId::new(id).unwrap(), ScalarType::Integer(i128_type))),
        )
        .unwrap();
        let axioms = [
            Proposition::Equal(
                intermediate.clone(),
                ScalarTerm::exact_integer_multiply(
                    i128_type,
                    root.clone(),
                    literal(i128_type, i128::MAX),
                )
                .unwrap(),
            ),
            Proposition::Equal(
                target.clone(),
                ScalarTerm::exact_integer_multiply(i128_type, intermediate, literal(i128_type, 2))
                    .unwrap(),
            ),
        ];
        assert_eq!(
            check_integer_affine_witness(
                &context,
                &axioms,
                &IntegerAffineWitness {
                    root,
                    target,
                    definition_axioms: vec![0, 1],
                    literal_axioms: vec![None, None],
                },
            ),
            Err(IntegerAffineWitnessError::CoefficientOverflow),
        );
    }

    #[test]
    fn maps_upper_and_lower_bounds_across_every_coefficient_sign() {
        let integer_type = IntegerType::new(IntegerSign::Signed, 16).expect("i16");
        let root = value(1, integer_type);
        let target = value(2, integer_type);
        let literal = |value| literal(integer_type, value);
        let form = |coefficient, offset| CheckedIntegerAffineForm {
            root: root.clone(),
            target: target.clone(),
            integer_type,
            coefficient,
            offset,
            endpoint_steps: vec![
                CheckedIntegerEndpointStep::Multiply(coefficient),
                CheckedIntegerEndpointStep::Add(offset),
            ],
        };
        let upper = Proposition::LessOrEqual(root.clone(), literal(4));
        let lower = Proposition::LessOrEqual(literal(-3), root.clone());

        assert_eq!(
            check_integer_affine_bound_conversion(
                &form(2, 1),
                &upper,
                &Proposition::LessOrEqual(target.clone(), literal(9)),
            ),
            Ok(()),
        );
        assert_eq!(
            check_integer_affine_bound_conversion(
                &form(2, 1),
                &lower,
                &Proposition::LessOrEqual(literal(-5), target.clone()),
            ),
            Ok(()),
        );
        assert_eq!(
            check_integer_affine_bound_conversion(
                &form(-2, 1),
                &upper,
                &Proposition::LessOrEqual(literal(-7), target.clone()),
            ),
            Ok(()),
        );
        assert_eq!(
            check_integer_affine_bound_conversion(
                &form(-2, 1),
                &lower,
                &Proposition::LessOrEqual(target.clone(), literal(7)),
            ),
            Ok(()),
        );
        assert_eq!(
            check_integer_affine_bound_conversion(
                &form(0, 5),
                &upper,
                &Proposition::LessOrEqual(target.clone(), literal(5)),
            ),
            Ok(()),
        );
        assert_eq!(
            check_integer_affine_bound_conversion(
                &form(0, 5),
                &lower,
                &Proposition::LessOrEqual(literal(5), target.clone()),
            ),
            Ok(()),
        );
        assert_eq!(
            integer_affine_truth_bounds(&form(0, 5)),
            Ok(vec![
                Proposition::LessOrEqual(literal(5), target.clone()),
                Proposition::LessOrEqual(target, literal(5)),
            ]),
        );
    }

    #[test]
    fn affine_bound_mapping_rejects_shape_direction_and_arithmetic_drift() {
        let integer_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let root = value(1, integer_type);
        let target = value(2, integer_type);
        let literal = |value| literal(integer_type, value);
        let form = CheckedIntegerAffineForm {
            root: root.clone(),
            target: target.clone(),
            integer_type,
            coefficient: 2,
            offset: 1,
            endpoint_steps: vec![
                CheckedIntegerEndpointStep::Multiply(2),
                CheckedIntegerEndpointStep::Add(1),
            ],
        };
        assert_eq!(
            check_integer_affine_bound_conversion(
                &form,
                &Proposition::Equal(root.clone(), literal(4)),
                &Proposition::LessOrEqual(target.clone(), literal(9)),
            ),
            Err(IntegerAffineBoundConversionError::RootBoundNotLessOrEqual),
        );
        assert_eq!(
            check_integer_affine_bound_conversion(
                &form,
                &Proposition::LessOrEqual(value(3, integer_type), literal(4)),
                &Proposition::LessOrEqual(target.clone(), literal(9)),
            ),
            Err(IntegerAffineBoundConversionError::RootBoundMismatch),
        );
        assert_eq!(
            check_integer_affine_bound_conversion(
                &form,
                &Proposition::LessOrEqual(root.clone(), value(3, integer_type)),
                &Proposition::LessOrEqual(target.clone(), literal(9)),
            ),
            Err(IntegerAffineBoundConversionError::RootBoundNotTypedLiteral),
        );
        assert_eq!(
            check_integer_affine_bound_conversion(
                &form,
                &Proposition::LessOrEqual(root.clone(), literal(4)),
                &Proposition::LessOrEqual(literal(9), target.clone()),
            ),
            Err(IntegerAffineBoundConversionError::ConclusionMismatch),
        );
        assert_eq!(
            check_integer_affine_bound_conversion(
                &CheckedIntegerAffineForm {
                    endpoint_steps: vec![
                        CheckedIntegerEndpointStep::Multiply(i128::MAX),
                        CheckedIntegerEndpointStep::Add(i128::MAX),
                    ],
                    ..form
                },
                &Proposition::LessOrEqual(root, literal(2)),
                &Proposition::LessOrEqual(target, literal(1)),
            ),
            Err(IntegerAffineBoundConversionError::MappedBoundOverflow),
        );
        assert_eq!(
            check_integer_affine_bound_conversion(
                &CheckedIntegerAffineForm {
                    root: value(1, integer_type),
                    target: value(2, integer_type),
                    integer_type,
                    coefficient: 2,
                    offset: 1,
                    endpoint_steps: vec![
                        CheckedIntegerEndpointStep::Multiply(2),
                        CheckedIntegerEndpointStep::Add(1),
                    ],
                },
                &Proposition::LessOrEqual(value(1, integer_type), literal(100)),
                &Proposition::LessOrEqual(value(2, integer_type), literal(127)),
            ),
            Err(IntegerAffineBoundConversionError::MappedBoundOutsideCarrier),
        );
    }

    #[test]
    fn maps_mixed_exact_shift_endpoints_and_rejects_witness_tamper() {
        let value_type = IntegerType::new(IntegerSign::Unsigned, 16).expect("u16");
        let i8_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let u16_type = IntegerType::new(IntegerSign::Unsigned, 16).expect("u16");
        let i32_type = IntegerType::new(IntegerSign::Signed, 32).expect("i32");
        let root = value(1, value_type);
        let first = value(2, value_type);
        let second = value(3, value_type);
        let target = value(4, value_type);
        let landed_count = value(5, i8_type);
        let integer = |integer_type: IntegerType, value: i128| {
            let value = match integer_type.sign() {
                IntegerSign::Signed => IntegerValue::Signed(value),
                IntegerSign::Unsigned => IntegerValue::Unsigned(value.try_into().unwrap()),
            };
            ScalarTerm::integer(integer_type, value).unwrap()
        };
        let axioms = vec![
            Proposition::Equal(landed_count.clone(), integer(i8_type, 1)),
            Proposition::Equal(
                first.clone(),
                ScalarTerm::exact_integer_shift_left(
                    value_type,
                    i8_type,
                    root.clone(),
                    landed_count,
                )
                .unwrap(),
            ),
            Proposition::Equal(
                second.clone(),
                ScalarTerm::exact_integer_shift_right(
                    value_type,
                    u16_type,
                    first,
                    integer(u16_type, 2),
                )
                .unwrap(),
            ),
            Proposition::Equal(
                target.clone(),
                ScalarTerm::exact_integer_shift_left(
                    value_type,
                    i32_type,
                    second,
                    integer(i32_type, 3),
                )
                .unwrap(),
            ),
        ];
        let context = PropositionContext::from_value_types(
            (1..=4)
                .map(|id| (ValueId::new(id).unwrap(), ScalarType::Integer(value_type)))
                .chain([(ValueId::new(5).unwrap(), ScalarType::Integer(i8_type))]),
        )
        .unwrap();
        let witness = IntegerAffineWitness {
            root: root.clone(),
            target: target.clone(),
            definition_axioms: vec![1, 2, 3],
            literal_axioms: vec![Some(0), None, None],
        };
        let checked = check_integer_affine_witness(&context, &axioms, &witness)
            .expect("ordered mixed shift witness");
        assert_eq!(
            map_integer_affine_bound(
                &checked,
                &Proposition::LessOrEqual(root.clone(), integer(value_type, 63)),
            ),
            Ok(Proposition::LessOrEqual(target, integer(value_type, 248),)),
        );
        assert_eq!(
            check_integer_affine_witness(
                &context,
                &axioms,
                &IntegerAffineWitness {
                    definition_axioms: vec![1, 3, 2],
                    ..witness.clone()
                },
            ),
            Err(IntegerAffineWitnessError::NonCanonicalDefinitionOrder),
        );
        assert_eq!(
            check_integer_affine_witness(
                &context,
                &axioms,
                &IntegerAffineWitness {
                    literal_axioms: vec![None, None, None],
                    ..witness
                },
            ),
            Err(IntegerAffineWitnessError::ShiftCountNotLanded(1)),
        );
    }

    #[test]
    fn direct_add_bound_replays_both_ordered_endpoints_and_rejects_mutations() {
        let integer_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let left = value(1, integer_type);
        let right = value(2, integer_type);
        let target = ScalarTerm::exact_integer_add(integer_type, left.clone(), right.clone())
            .expect("exact add");
        let context = PropositionContext::from_value_types([
            (ValueId::new(1).unwrap(), ScalarType::Integer(integer_type)),
            (ValueId::new(2).unwrap(), ScalarType::Integer(integer_type)),
            (ValueId::new(3).unwrap(), ScalarType::Integer(integer_type)),
        ])
        .unwrap();
        let checked = check_integer_affine_witness(
            &context,
            &[],
            &IntegerAffineWitness {
                root: left.clone(),
                target,
                definition_axioms: Vec::new(),
                literal_axioms: Vec::new(),
            },
        )
        .expect("direct mathematical add endpoint");
        let sum = IntegerMathTerm::Add(
            Box::new(IntegerMathTerm::MathValue {
                source_type: integer_type,
                value: ValueId::new(1).unwrap(),
            }),
            Box::new(IntegerMathTerm::MathValue {
                source_type: integer_type,
                value: ValueId::new(2).unwrap(),
            }),
        );
        assert_eq!(
            map_integer_affine_bound(
                &checked,
                &Proposition::Conjunction(vec![
                    Proposition::LessOrEqual(literal(integer_type, -100), left.clone()),
                    Proposition::LessOrEqual(literal(integer_type, 20), right.clone()),
                ]),
            ),
            Ok(Proposition::IntegerMathLessOrEqual(
                IntegerMathTerm::literal(IntegerValue::Signed(-80)),
                sum.clone(),
            )),
        );
        assert_eq!(
            map_integer_affine_bound(
                &checked,
                &Proposition::Conjunction(vec![
                    Proposition::LessOrEqual(left.clone(), literal(integer_type, 100)),
                    Proposition::LessOrEqual(right.clone(), literal(integer_type, 20)),
                ]),
            ),
            Ok(Proposition::IntegerMathLessOrEqual(
                sum.clone(),
                IntegerMathTerm::literal(IntegerValue::Signed(120)),
            )),
        );
        assert_eq!(
            map_integer_affine_bound(
                &checked,
                &Proposition::Conjunction(vec![
                    Proposition::Equal(left.clone(), literal(integer_type, 7)),
                    Proposition::LessOrEqual(right.clone(), literal(integer_type, 100)),
                ]),
            ),
            Ok(Proposition::IntegerMathLessOrEqual(
                sum.clone(),
                IntegerMathTerm::literal(IntegerValue::Signed(107)),
            )),
        );
        assert_eq!(
            map_integer_affine_bound(
                &checked,
                &Proposition::Conjunction(vec![
                    Proposition::Truth,
                    Proposition::LessOrEqual(literal(integer_type, 0), right.clone()),
                ]),
            ),
            Ok(Proposition::IntegerMathLessOrEqual(
                IntegerMathTerm::literal(IntegerValue::Signed(-128)),
                sum.clone(),
            )),
        );
        assert_eq!(
            map_integer_affine_bound(
                &checked,
                &Proposition::Conjunction(vec![
                    Proposition::Truth,
                    Proposition::LessOrEqual(right.clone(), literal(integer_type, 0)),
                ]),
            ),
            Ok(Proposition::IntegerMathLessOrEqual(
                sum.clone(),
                IntegerMathTerm::literal(IntegerValue::Signed(127)),
            )),
        );
        assert_eq!(
            map_integer_affine_bound(
                &checked,
                &Proposition::Conjunction(vec![Proposition::Truth, Proposition::Truth]),
            ),
            Err(IntegerAffineBoundConversionError::DirectAddEvidenceMismatch),
        );

        let seven = literal(integer_type, 7);
        for (target, root, evidence, expected_sum) in [
            (
                ScalarTerm::exact_integer_add(integer_type, left.clone(), seven.clone()).unwrap(),
                left.clone(),
                Proposition::Conjunction(vec![
                    Proposition::LessOrEqual(left.clone(), literal(integer_type, 100)),
                    Proposition::Truth,
                ]),
                IntegerMathTerm::Add(
                    Box::new(IntegerMathTerm::MathValue {
                        source_type: integer_type,
                        value: ValueId::new(1).unwrap(),
                    }),
                    Box::new(IntegerMathTerm::literal(IntegerValue::Signed(7))),
                ),
            ),
            (
                ScalarTerm::exact_integer_add(integer_type, seven.clone(), right.clone()).unwrap(),
                seven,
                Proposition::Conjunction(vec![
                    Proposition::Truth,
                    Proposition::LessOrEqual(right.clone(), literal(integer_type, 100)),
                ]),
                IntegerMathTerm::Add(
                    Box::new(IntegerMathTerm::literal(IntegerValue::Signed(7))),
                    Box::new(IntegerMathTerm::MathValue {
                        source_type: integer_type,
                        value: ValueId::new(2).unwrap(),
                    }),
                ),
            ),
        ] {
            let literal_checked = check_integer_affine_witness(
                &context,
                &[],
                &IntegerAffineWitness {
                    root,
                    target,
                    definition_axioms: Vec::new(),
                    literal_axioms: Vec::new(),
                },
            )
            .expect("direct add with embedded literal");
            assert_eq!(
                map_integer_affine_bound(&literal_checked, &evidence),
                Ok(Proposition::IntegerMathLessOrEqual(
                    expected_sum,
                    IntegerMathTerm::literal(IntegerValue::Signed(107)),
                )),
            );
        }

        for malformed in [
            Proposition::LessOrEqual(left.clone(), literal(integer_type, 100)),
            Proposition::Conjunction(vec![
                Proposition::LessOrEqual(literal(integer_type, -100), left.clone()),
                Proposition::LessOrEqual(right.clone(), literal(integer_type, 20)),
            ]),
            Proposition::Conjunction(vec![
                Proposition::LessOrEqual(value(3, integer_type), literal(integer_type, 100)),
                Proposition::LessOrEqual(right.clone(), literal(integer_type, 20)),
            ]),
            Proposition::Conjunction(vec![
                Proposition::LessOrEqual(right.clone(), literal(integer_type, 20)),
                Proposition::LessOrEqual(left.clone(), literal(integer_type, 100)),
            ]),
        ] {
            assert_eq!(
                map_integer_affine_bound(&checked, &malformed),
                Err(IntegerAffineBoundConversionError::DirectAddEvidenceMismatch),
            );
        }

        let i128_type = IntegerType::new(IntegerSign::Signed, 128).expect("i128");
        let i128_left = value(4, i128_type);
        let i128_right = value(5, i128_type);
        let i128_context = PropositionContext::from_value_types([
            (ValueId::new(4).unwrap(), ScalarType::Integer(i128_type)),
            (ValueId::new(5).unwrap(), ScalarType::Integer(i128_type)),
        ])
        .unwrap();
        let i128_checked = check_integer_affine_witness(
            &i128_context,
            &[],
            &IntegerAffineWitness {
                root: i128_left.clone(),
                target: ScalarTerm::exact_integer_add(
                    i128_type,
                    i128_left.clone(),
                    i128_right.clone(),
                )
                .unwrap(),
                definition_axioms: Vec::new(),
                literal_axioms: Vec::new(),
            },
        )
        .unwrap();
        assert_eq!(
            map_integer_affine_bound(
                &i128_checked,
                &Proposition::Conjunction(vec![
                    Proposition::LessOrEqual(literal(i128_type, i128::MIN), i128_left,),
                    Proposition::LessOrEqual(literal(i128_type, i128::MIN), i128_right,),
                ]),
            ),
            Err(IntegerAffineBoundConversionError::DirectAddBoundOverflow),
        );
    }

    #[test]
    fn correlated_add_bound_replays_exact_complement_identity_and_relation() {
        let integer_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let left = value(1, integer_type);
        let right = value(2, integer_type);
        let correlated = value(3, integer_type);
        let redirected = value(4, integer_type);
        let target = ScalarTerm::exact_integer_add(integer_type, left.clone(), right.clone())
            .expect("exact add");
        let context = PropositionContext::from_value_types(
            (1..=5).map(|id| (ValueId::new(id).unwrap(), ScalarType::Integer(integer_type))),
        )
        .unwrap();
        for (endpoint, evidence, expected) in [
            (
                127,
                Proposition::LessOrEqual(left.clone(), correlated.clone()),
                Proposition::IntegerMathLessOrEqual(
                    IntegerMathTerm::Add(
                        Box::new(IntegerMathTerm::MathValue {
                            source_type: integer_type,
                            value: ValueId::new(1).unwrap(),
                        }),
                        Box::new(IntegerMathTerm::MathValue {
                            source_type: integer_type,
                            value: ValueId::new(2).unwrap(),
                        }),
                    ),
                    IntegerMathTerm::literal(IntegerValue::Signed(127)),
                ),
            ),
            (
                -128,
                Proposition::LessOrEqual(correlated.clone(), left.clone()),
                Proposition::IntegerMathLessOrEqual(
                    IntegerMathTerm::literal(IntegerValue::Signed(-128)),
                    IntegerMathTerm::Add(
                        Box::new(IntegerMathTerm::MathValue {
                            source_type: integer_type,
                            value: ValueId::new(1).unwrap(),
                        }),
                        Box::new(IntegerMathTerm::MathValue {
                            source_type: integer_type,
                            value: ValueId::new(2).unwrap(),
                        }),
                    ),
                ),
            ),
        ] {
            let axiom = Proposition::Equal(
                correlated.clone(),
                ScalarTerm::exact_integer_subtract(
                    integer_type,
                    literal(integer_type, endpoint),
                    right.clone(),
                )
                .unwrap(),
            );
            let witness = IntegerAffineWitness {
                root: correlated.clone(),
                target: target.clone(),
                definition_axioms: vec![0],
                literal_axioms: vec![None],
            };
            let checked =
                check_integer_affine_witness(&context, std::slice::from_ref(&axiom), &witness)
                    .expect("correlated add witness");
            assert_eq!(map_integer_affine_bound(&checked, &evidence), Ok(expected));
            assert_eq!(
                map_integer_affine_bound(
                    &checked,
                    &Proposition::LessOrEqual(left.clone(), redirected.clone()),
                ),
                Err(IntegerAffineBoundConversionError::DirectAddEvidenceMismatch),
            );
            assert!(
                check_integer_affine_witness(
                    &context,
                    &[Proposition::Equal(
                        correlated.clone(),
                        ScalarTerm::exact_integer_subtract(
                            integer_type,
                            literal(integer_type, endpoint),
                            redirected.clone(),
                        )
                        .unwrap(),
                    )],
                    &witness,
                )
                .is_err(),
                "redirecting the complement operand invalidates the witness",
            );
            assert!(
                check_integer_affine_witness(
                    &context,
                    std::slice::from_ref(&axiom),
                    &IntegerAffineWitness {
                        definition_axioms: Vec::new(),
                        literal_axioms: Vec::new(),
                        ..witness
                    },
                )
                .is_err(),
                "omitting correlated definition custody invalidates the witness",
            );
        }

        let landed_endpoint = value(5, integer_type);
        let landing = Proposition::Equal(landed_endpoint.clone(), literal(integer_type, 127));
        let complement = Proposition::Equal(
            correlated.clone(),
            ScalarTerm::exact_integer_subtract(
                integer_type,
                landed_endpoint.clone(),
                right.clone(),
            )
            .unwrap(),
        );
        let landed_witness = IntegerAffineWitness {
            root: correlated.clone(),
            target: target.clone(),
            definition_axioms: vec![1],
            literal_axioms: vec![Some(0)],
        };
        let landed = check_integer_affine_witness(
            &context,
            &[landing.clone(), complement.clone()],
            &landed_witness,
        )
        .expect("landed carrier endpoint and complement are replayed in order");
        assert_eq!(
            map_integer_affine_bound(
                &landed,
                &Proposition::LessOrEqual(left.clone(), correlated.clone()),
            ),
            Ok(Proposition::IntegerMathLessOrEqual(
                IntegerMathTerm::Add(
                    Box::new(IntegerMathTerm::MathValue {
                        source_type: integer_type,
                        value: ValueId::new(1).unwrap(),
                    }),
                    Box::new(IntegerMathTerm::MathValue {
                        source_type: integer_type,
                        value: ValueId::new(2).unwrap(),
                    }),
                ),
                IntegerMathTerm::literal(IntegerValue::Signed(127)),
            )),
        );
        let redirected_complement = Proposition::Equal(
            correlated.clone(),
            ScalarTerm::exact_integer_subtract(
                integer_type,
                landed_endpoint.clone(),
                redirected.clone(),
            )
            .unwrap(),
        );
        for (mutation, (axioms, witness)) in [
            (
                vec![landing.clone(), complement.clone()],
                IntegerAffineWitness {
                    literal_axioms: vec![None],
                    ..landed_witness.clone()
                },
            ),
            (
                vec![complement.clone(), landing.clone()],
                IntegerAffineWitness {
                    definition_axioms: vec![0],
                    literal_axioms: vec![Some(1)],
                    ..landed_witness.clone()
                },
            ),
            (
                vec![
                    Proposition::Equal(landed_endpoint.clone(), literal(integer_type, 126)),
                    complement.clone(),
                ],
                landed_witness.clone(),
            ),
            (
                vec![
                    Proposition::Equal(landed_endpoint.clone(), ScalarTerm::boolean(true)),
                    complement.clone(),
                ],
                landed_witness.clone(),
            ),
            (
                vec![
                    Proposition::Equal(redirected, literal(integer_type, 127)),
                    complement,
                ],
                landed_witness.clone(),
            ),
            (vec![landing, redirected_complement], landed_witness),
        ]
        .into_iter()
        .enumerate()
        {
            let result = check_integer_affine_witness(&context, &axioms, &witness);
            assert!(
                result.is_err(),
                "reordered, omitted, or stale endpoint landing custody rejects: {mutation}: {result:?}",
            );
        }
    }

    #[test]
    fn direct_subtract_bound_replays_opposite_ordered_endpoints_and_rejects_mutations() {
        let integer_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let left = value(1, integer_type);
        let right = value(2, integer_type);
        let target =
            ScalarTerm::exact_integer_subtract(integer_type, left.clone(), right.clone()).unwrap();
        let context = PropositionContext::from_value_types(
            (1..=5).map(|id| (ValueId::new(id).unwrap(), ScalarType::Integer(integer_type))),
        )
        .unwrap();
        let checked = check_integer_affine_witness(
            &context,
            &[],
            &IntegerAffineWitness {
                root: left.clone(),
                target,
                definition_axioms: Vec::new(),
                literal_axioms: Vec::new(),
            },
        )
        .expect("direct subtract witness");
        let lower = Proposition::Conjunction(vec![
            Proposition::LessOrEqual(literal(integer_type, -100), left.clone()),
            Proposition::LessOrEqual(right.clone(), literal(integer_type, 20)),
        ]);
        assert_eq!(
            map_integer_affine_bound(&checked, &lower),
            Ok(Proposition::IntegerMathLessOrEqual(
                IntegerMathTerm::literal(IntegerValue::Signed(-120)),
                IntegerMathTerm::Subtract(
                    Box::new(IntegerMathTerm::MathValue {
                        source_type: integer_type,
                        value: ValueId::new(1).unwrap(),
                    }),
                    Box::new(IntegerMathTerm::MathValue {
                        source_type: integer_type,
                        value: ValueId::new(2).unwrap(),
                    }),
                ),
            )),
        );
        let upper = Proposition::Conjunction(vec![
            Proposition::LessOrEqual(left.clone(), literal(integer_type, 100)),
            Proposition::LessOrEqual(literal(integer_type, -20), right.clone()),
        ]);
        assert!(map_integer_affine_bound(&checked, &upper).is_ok());
        for malformed in [
            Proposition::Conjunction(vec![
                Proposition::LessOrEqual(literal(integer_type, -100), left.clone()),
                Proposition::LessOrEqual(literal(integer_type, -20), right.clone()),
            ]),
            Proposition::Conjunction(vec![
                Proposition::LessOrEqual(right.clone(), literal(integer_type, 20)),
                Proposition::LessOrEqual(literal(integer_type, -100), left.clone()),
            ]),
            Proposition::Conjunction(vec![
                Proposition::LessOrEqual(literal(integer_type, -100), value(3, integer_type)),
                Proposition::LessOrEqual(right.clone(), literal(integer_type, 20)),
            ]),
        ] {
            assert!(
                map_integer_affine_bound(&checked, &malformed).is_err(),
                "mixed orientation, reordered evidence, and redirected operands reject",
            );
        }
        assert!(
            map_integer_affine_bound(
                &checked,
                &Proposition::Conjunction(vec![
                    Proposition::Equal(left.clone(), literal(integer_type, 127)),
                    Proposition::Truth,
                ]),
            )
            .is_ok(),
            "an exact MAX minuend and carrier-wide subtrahend derive the lower endpoint",
        );
        assert!(
            map_integer_affine_bound(
                &checked,
                &Proposition::Conjunction(vec![
                    Proposition::Equal(left, literal(integer_type, 100)),
                    Proposition::Truth,
                ]),
            )
            .is_err(),
            "a noncarrier exact minuend cannot orient bare subtrahend inclusion",
        );
    }

    #[test]
    fn correlated_subtract_replays_complement_definition_and_endpoint_landing() {
        let integer_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let left = value(1, integer_type);
        let right = value(2, integer_type);
        let correlated = value(3, integer_type);
        let landed_endpoint = value(4, integer_type);
        let target =
            ScalarTerm::exact_integer_subtract(integer_type, left.clone(), right.clone()).unwrap();
        let context = PropositionContext::from_value_types(
            (1..=5).map(|id| (ValueId::new(id).unwrap(), ScalarType::Integer(integer_type))),
        )
        .unwrap();
        let landing = Proposition::Equal(landed_endpoint.clone(), literal(integer_type, -128));
        let complement = Proposition::Equal(
            correlated.clone(),
            ScalarTerm::exact_integer_add(integer_type, landed_endpoint.clone(), right.clone())
                .unwrap(),
        );
        let witness = IntegerAffineWitness {
            root: correlated.clone(),
            target: target.clone(),
            definition_axioms: vec![1],
            literal_axioms: vec![Some(0)],
        };
        let checked = check_integer_affine_witness(
            &context,
            &[landing.clone(), complement.clone()],
            &witness,
        )
        .expect("landed MIN plus right complement");
        assert_eq!(
            map_integer_affine_bound(
                &checked,
                &Proposition::LessOrEqual(correlated.clone(), left.clone()),
            ),
            Ok(Proposition::IntegerMathLessOrEqual(
                IntegerMathTerm::literal(IntegerValue::Signed(-128)),
                IntegerMathTerm::Subtract(
                    Box::new(IntegerMathTerm::MathValue {
                        source_type: integer_type,
                        value: ValueId::new(1).unwrap(),
                    }),
                    Box::new(IntegerMathTerm::MathValue {
                        source_type: integer_type,
                        value: ValueId::new(2).unwrap(),
                    }),
                ),
            )),
        );
        assert!(
            check_integer_affine_witness(
                &context,
                &[landing.clone(), complement.clone()],
                &IntegerAffineWitness {
                    literal_axioms: vec![None],
                    ..witness.clone()
                },
            )
            .is_err(),
            "omitting the endpoint landing rejects",
        );
        let stale = Proposition::Equal(value(5, integer_type), literal(integer_type, -128));
        assert!(
            check_integer_affine_witness(&context, &[stale, complement], &witness).is_err(),
            "redirecting the endpoint landing rejects",
        );
        assert!(
            map_integer_affine_bound(&checked, &Proposition::LessOrEqual(left, correlated),)
                .is_err(),
            "reversing the authored guard rejects",
        );
    }

    #[test]
    fn unsigned_correlated_subtract_replays_exact_operand_order() {
        let integer_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let left = value(1, integer_type);
        let right = value(2, integer_type);
        let context = PropositionContext::from_value_types([
            (ValueId::new(1).unwrap(), ScalarType::Integer(integer_type)),
            (ValueId::new(2).unwrap(), ScalarType::Integer(integer_type)),
        ])
        .unwrap();
        let checked = check_integer_affine_witness(
            &context,
            &[],
            &IntegerAffineWitness {
                root: right.clone(),
                target: ScalarTerm::exact_integer_subtract(
                    integer_type,
                    left.clone(),
                    right.clone(),
                )
                .unwrap(),
                definition_axioms: Vec::new(),
                literal_axioms: Vec::new(),
            },
        )
        .expect("unsigned joint guard witness");
        assert!(
            map_integer_affine_bound(
                &checked,
                &Proposition::LessOrEqual(right.clone(), left.clone()),
            )
            .is_ok(),
        );
        assert!(
            map_integer_affine_bound(&checked, &Proposition::LessOrEqual(left, right)).is_err(),
            "reversed unsigned guard rejects",
        );
    }

    #[test]
    fn direct_multiply_replays_four_corners_and_rejects_order_mutations() {
        let integer_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let left = value(1, integer_type);
        let right = value(2, integer_type);
        let context = PropositionContext::from_value_types([
            (ValueId::new(1).unwrap(), ScalarType::Integer(integer_type)),
            (ValueId::new(2).unwrap(), ScalarType::Integer(integer_type)),
            (ValueId::new(3).unwrap(), ScalarType::Integer(integer_type)),
        ])
        .unwrap();
        let checked = check_integer_affine_witness(
            &context,
            &[],
            &IntegerAffineWitness {
                root: left.clone(),
                target: ScalarTerm::exact_integer_multiply(
                    integer_type,
                    left.clone(),
                    right.clone(),
                )
                .unwrap(),
                definition_axioms: Vec::new(),
                literal_axioms: Vec::new(),
            },
        )
        .expect("direct multiply witness");
        let lower = Proposition::Conjunction(vec![
            Proposition::LessOrEqual(literal(integer_type, -4), left.clone()),
            Proposition::LessOrEqual(left.clone(), literal(integer_type, 5)),
            Proposition::LessOrEqual(literal(integer_type, -3), right.clone()),
            Proposition::LessOrEqual(right.clone(), literal(integer_type, 2)),
        ]);
        let product = IntegerMathTerm::Multiply(
            Box::new(IntegerMathTerm::MathValue {
                source_type: integer_type,
                value: ValueId::new(1).unwrap(),
            }),
            Box::new(IntegerMathTerm::MathValue {
                source_type: integer_type,
                value: ValueId::new(2).unwrap(),
            }),
        );
        assert_eq!(
            map_integer_affine_bound(&checked, &lower),
            Ok(Proposition::IntegerMathLessOrEqual(
                IntegerMathTerm::literal(IntegerValue::Signed(-15)),
                product,
            )),
        );
        for malformed in [
            Proposition::Conjunction(vec![
                Proposition::LessOrEqual(left.clone(), literal(integer_type, 5)),
                Proposition::LessOrEqual(literal(integer_type, -4), left.clone()),
                Proposition::LessOrEqual(literal(integer_type, -3), right.clone()),
                Proposition::LessOrEqual(right.clone(), literal(integer_type, 2)),
            ]),
            Proposition::Conjunction(vec![
                Proposition::LessOrEqual(literal(integer_type, -4), left.clone()),
                Proposition::LessOrEqual(left.clone(), literal(integer_type, 5)),
                Proposition::LessOrEqual(literal(integer_type, -3), value(3, integer_type)),
                Proposition::LessOrEqual(right.clone(), literal(integer_type, 2)),
            ]),
            Proposition::Conjunction(vec![
                Proposition::LessOrEqual(literal(integer_type, -4), left.clone()),
                Proposition::LessOrEqual(left, literal(integer_type, 5)),
                Proposition::LessOrEqual(literal(integer_type, -3), right),
            ]),
        ] {
            assert!(map_integer_affine_bound(&checked, &malformed).is_err());
        }
    }

    #[test]
    fn direct_multiply_requires_oriented_zero_evidence_for_exact_zero_bounds() {
        let integer_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let left = value(1, integer_type);
        let right = value(2, integer_type);
        let context = PropositionContext::from_value_types([
            (ValueId::new(1).unwrap(), ScalarType::Integer(integer_type)),
            (ValueId::new(2).unwrap(), ScalarType::Integer(integer_type)),
        ])
        .unwrap();
        let checked = check_integer_affine_witness(
            &context,
            &[],
            &IntegerAffineWitness {
                root: left.clone(),
                target: ScalarTerm::exact_integer_multiply(
                    integer_type,
                    left.clone(),
                    right.clone(),
                )
                .unwrap(),
                definition_axioms: Vec::new(),
                literal_axioms: Vec::new(),
            },
        )
        .expect("direct multiply witness");
        let zero = literal(integer_type, 0);
        let product = IntegerMathTerm::Multiply(
            Box::new(IntegerMathTerm::MathValue {
                source_type: integer_type,
                value: ValueId::new(1).unwrap(),
            }),
            Box::new(IntegerMathTerm::MathValue {
                source_type: integer_type,
                value: ValueId::new(2).unwrap(),
            }),
        );
        let lower = Proposition::Conjunction(vec![
            Proposition::Truth,
            Proposition::Truth,
            Proposition::LessOrEqual(zero.clone(), right.clone()),
            Proposition::LessOrEqual(right.clone(), zero.clone()),
        ]);
        assert_eq!(
            map_integer_affine_bound(&checked, &lower),
            Ok(Proposition::IntegerMathLessOrEqual(
                IntegerMathTerm::literal(IntegerValue::Signed(0)),
                product.clone(),
            )),
        );
        let upper = Proposition::Conjunction(vec![
            Proposition::Truth,
            Proposition::Truth,
            Proposition::LessOrEqual(right.clone(), zero.clone()),
            Proposition::LessOrEqual(zero, right.clone()),
        ]);
        assert_eq!(
            map_integer_affine_bound(&checked, &upper),
            Ok(Proposition::IntegerMathLessOrEqual(
                product,
                IntegerMathTerm::literal(IntegerValue::Signed(0)),
            )),
        );

        let one_equality = Proposition::Equal(right, literal(integer_type, 1));
        let unoriented_nonzero = Proposition::Conjunction(vec![
            Proposition::Truth,
            Proposition::Truth,
            one_equality.clone(),
            one_equality,
        ]);
        assert!(map_integer_affine_bound(&checked, &unoriented_nonzero).is_err());
    }

    #[test]
    fn correlated_negative_multiply_uses_target_endpoint_and_sign_orientation() {
        let integer_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let left = value(1, integer_type);
        let right = value(2, integer_type);
        let quotient = value(3, integer_type);
        let context = PropositionContext::from_value_types(
            (1..=5).map(|id| (ValueId::new(id).unwrap(), ScalarType::Integer(integer_type))),
        )
        .unwrap();
        let target =
            ScalarTerm::exact_integer_multiply(integer_type, left.clone(), right.clone()).unwrap();
        let negative = Proposition::LessOrEqual(right.clone(), literal(integer_type, -2));
        for (endpoint, comparison, expected) in [
            (
                -128,
                Proposition::LessOrEqual(left.clone(), quotient.clone()),
                Proposition::IntegerMathLessOrEqual(
                    IntegerMathTerm::literal(IntegerValue::Signed(-128)),
                    IntegerMathTerm::Multiply(
                        Box::new(IntegerMathTerm::MathValue {
                            source_type: integer_type,
                            value: ValueId::new(1).unwrap(),
                        }),
                        Box::new(IntegerMathTerm::MathValue {
                            source_type: integer_type,
                            value: ValueId::new(2).unwrap(),
                        }),
                    ),
                ),
            ),
            (
                127,
                Proposition::LessOrEqual(quotient.clone(), left.clone()),
                Proposition::IntegerMathLessOrEqual(
                    IntegerMathTerm::Multiply(
                        Box::new(IntegerMathTerm::MathValue {
                            source_type: integer_type,
                            value: ValueId::new(1).unwrap(),
                        }),
                        Box::new(IntegerMathTerm::MathValue {
                            source_type: integer_type,
                            value: ValueId::new(2).unwrap(),
                        }),
                    ),
                    IntegerMathTerm::literal(IntegerValue::Signed(127)),
                ),
            ),
        ] {
            let axiom = Proposition::Equal(
                quotient.clone(),
                ScalarTerm::exact_integer_divide(
                    integer_type,
                    literal(integer_type, endpoint),
                    right.clone(),
                )
                .unwrap(),
            );
            let checked = check_integer_affine_witness(
                &context,
                std::slice::from_ref(&axiom),
                &IntegerAffineWitness {
                    root: quotient.clone(),
                    target: target.clone(),
                    definition_axioms: vec![0],
                    literal_axioms: vec![None],
                },
            )
            .expect("negative quotient witness");
            let evidence = Proposition::Conjunction(vec![negative.clone(), comparison.clone()]);
            assert_eq!(map_integer_affine_bound(&checked, &evidence), Ok(expected));
            let Proposition::LessOrEqual(comparison_left, comparison_right) = &comparison else {
                unreachable!("correlated comparison is ordered")
            };
            assert_eq!(
                map_integer_affine_bound(
                    &checked,
                    &Proposition::Conjunction(vec![
                        negative.clone(),
                        Proposition::LessOrEqual(comparison_right.clone(), comparison_left.clone(),),
                    ]),
                ),
                Err(IntegerAffineBoundConversionError::DirectMultiplyEvidenceMismatch),
                "for right=-2, lower uses left<=MIN/right (64) and upper uses MAX/right (-63)<=left",
            );
        }

        let landed_endpoint = value(4, integer_type);
        let landing = Proposition::Equal(landed_endpoint.clone(), literal(integer_type, -128));
        let definition = Proposition::Equal(
            quotient.clone(),
            ScalarTerm::exact_integer_divide(integer_type, landed_endpoint.clone(), right.clone())
                .unwrap(),
        );
        let landed_witness = IntegerAffineWitness {
            root: quotient,
            target,
            definition_axioms: vec![1],
            literal_axioms: vec![Some(0)],
        };
        assert!(
            check_integer_affine_witness(
                &context,
                &[landing.clone(), definition.clone()],
                &landed_witness,
            )
            .is_ok(),
            "an exact earlier endpoint landing is replayed",
        );
        for (axioms, witness) in [
            (
                vec![landing.clone(), definition.clone()],
                IntegerAffineWitness {
                    literal_axioms: vec![None],
                    ..landed_witness.clone()
                },
            ),
            (
                vec![definition.clone(), landing.clone()],
                IntegerAffineWitness {
                    definition_axioms: vec![0],
                    literal_axioms: vec![Some(1)],
                    ..landed_witness.clone()
                },
            ),
            (
                vec![
                    Proposition::Equal(landed_endpoint.clone(), literal(integer_type, -127)),
                    definition.clone(),
                ],
                landed_witness.clone(),
            ),
            (
                vec![
                    landing,
                    Proposition::Equal(
                        landed_witness.root.clone(),
                        ScalarTerm::exact_integer_divide(
                            integer_type,
                            landed_endpoint,
                            value(5, integer_type),
                        )
                        .unwrap(),
                    ),
                ],
                landed_witness,
            ),
        ] {
            assert!(check_integer_affine_witness(&context, &axioms, &witness).is_err());
        }
    }

    #[test]
    fn direct_shift_bound_replays_count_range_and_sign_oriented_endpoint() {
        let integer_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let count_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8 count");
        let root = value(1, integer_type);
        let count = value(2, count_type);
        let target = ScalarTerm::exact_integer_shift_left(
            integer_type,
            count_type,
            root.clone(),
            count.clone(),
        )
        .unwrap();
        let context = PropositionContext::from_value_types([
            (ValueId::new(1).unwrap(), ScalarType::Integer(integer_type)),
            (ValueId::new(2).unwrap(), ScalarType::Integer(count_type)),
        ])
        .unwrap();
        let checked = check_integer_affine_witness(
            &context,
            &[],
            &IntegerAffineWitness {
                root: root.clone(),
                target,
                definition_axioms: Vec::new(),
                literal_axioms: Vec::new(),
            },
        )
        .expect("direct mathematical shift endpoint");
        let lower_count = Proposition::LessOrEqual(literal(count_type, 0), count.clone());
        let upper_count = Proposition::LessOrEqual(count.clone(), literal(count_type, 3));
        let shifted = IntegerMathTerm::ShiftLeft {
            value: Box::new(IntegerMathTerm::MathValue {
                source_type: integer_type,
                value: ValueId::new(1).unwrap(),
            }),
            count: Box::new(IntegerMathTerm::MathValue {
                source_type: count_type,
                value: ValueId::new(2).unwrap(),
            }),
        };
        let mapped = |root_bound| {
            map_integer_affine_bound(
                &checked,
                &Proposition::Conjunction(vec![
                    root_bound,
                    lower_count.clone(),
                    upper_count.clone(),
                ]),
            )
        };
        assert_eq!(
            mapped(Proposition::LessOrEqual(
                literal(integer_type, -16),
                root.clone(),
            )),
            Ok(Proposition::IntegerMathLessOrEqual(
                IntegerMathTerm::literal(IntegerValue::Signed(-128)),
                shifted.clone(),
            )),
        );
        assert_eq!(
            mapped(Proposition::LessOrEqual(
                root.clone(),
                literal(integer_type, 15),
            )),
            Ok(Proposition::IntegerMathLessOrEqual(
                shifted.clone(),
                IntegerMathTerm::literal(IntegerValue::Signed(120)),
            )),
        );
        assert_eq!(
            mapped(Proposition::LessOrEqual(
                literal(integer_type, 2),
                root.clone(),
            )),
            Ok(Proposition::IntegerMathLessOrEqual(
                IntegerMathTerm::literal(IntegerValue::Signed(2)),
                shifted.clone(),
            )),
        );
        assert_eq!(
            mapped(Proposition::LessOrEqual(
                root.clone(),
                literal(integer_type, -2),
            )),
            Ok(Proposition::IntegerMathLessOrEqual(
                shifted,
                IntegerMathTerm::literal(IntegerValue::Signed(-2)),
            )),
        );
        assert_eq!(
            map_integer_affine_bound(
                &checked,
                &Proposition::Conjunction(vec![
                    Proposition::LessOrEqual(root.clone(), literal(integer_type, 15)),
                    upper_count,
                ]),
            ),
            Err(IntegerAffineBoundConversionError::DirectShiftCountLowerMissing),
        );
        assert_eq!(
            map_integer_affine_bound(
                &checked,
                &Proposition::Conjunction(vec![
                    Proposition::LessOrEqual(root, literal(integer_type, 15)),
                    lower_count,
                    Proposition::LessOrEqual(count, literal(count_type, 8)),
                ]),
            ),
            Err(IntegerAffineBoundConversionError::DirectShiftCountOutsideValueWidth),
        );
    }
}
