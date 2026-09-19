//! Checking an integer affine witness: normalizing the ordered endpoint
//! chain, replaying each correlated definition, and landing literals.

use crate::integer_rules::integer_affine::bound_mapping::direct_math_leaf;
use semantic_vocabulary::{
    IntegerCarrier, IntegerSign, IntegerType, IntegerValue, Proposition, PropositionContext,
    ScalarTerm, ScalarType,
};
use terminal_psi::IntegerAffineWitness;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedIntegerAffineForm {
    pub(crate) root: ScalarTerm,
    pub(crate) target: ScalarTerm,
    pub(crate) integer_type: IntegerType,
    pub(crate) coefficient: i128,
    pub(crate) offset: i128,
    pub(crate) endpoint_steps: Vec<CheckedIntegerEndpointStep>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CheckedIntegerEndpointStep {
    Add(i128),
    Subtract(i128),
    Multiply(i128),
    Divide(i128),
    Remainder(i128),
    ShiftLeft(u32),
    ShiftRight(u32),
    /// `target = wrapping_add(operand, literal)` traversed toward the defined
    /// result, unsigned fixed carriers only. An upper mapped bound is
    /// unconditional because reduction modulo the width can only lower the
    /// result; a lower mapped bound is sound only when the same definition
    /// carries the checked no-wrap conjunct `operand <= maximum - literal`.
    WrappingAdd {
        operand: ScalarTerm,
        literal: i128,
    },
    /// The same equation traversed toward the operand: a lower mapped bound
    /// is unconditional, while an upper mapped bound is sound only under the
    /// checked `operand <= maximum - literal` evidence.
    WrappingAddBackward {
        operand: ScalarTerm,
        literal: i128,
    },
    /// `x & mask` traversed toward the defined result, with `mask` a checked
    /// non-negative literal or landed literal. An AND never sets a bit absent
    /// from the mask, so the result image is `[0, mask]` whatever range the
    /// operand carried. A negative signed mask leaves the sign bit reachable
    /// and the step is rejected while the witness is checked.
    BitwiseAndMask(i128),
    CorrelatedAddLower,
    CorrelatedAddUpper,
    CorrelatedSubtractLower,
    CorrelatedSubtractUpper,
    CorrelatedUnsignedSubtract,
    CorrelatedMultiplyMinimum,
    CorrelatedMultiplyMaximum,
}

impl CheckedIntegerEndpointStep {
    /// Strict endpoints survive only pure translations: scaling, division,
    /// remainder, and shift steps change the strict relation's strength.
    pub(crate) fn preserves_strict_endpoint(&self) -> bool {
        matches!(
            self,
            Self::Add(_)
                | Self::Subtract(_)
                | Self::WrappingAdd { .. }
                | Self::WrappingAddBackward { .. }
        )
    }
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
        let wrapping_backward = apply_wrapping_add_inverse(
            left,
            right,
            &current,
            integer_type,
            coefficient,
            offset,
            landed.as_ref(),
            index,
        );
        let wrapping_backward_reverse = apply_wrapping_add_inverse(
            right,
            left,
            &current,
            integer_type,
            coefficient,
            offset,
            landed.as_ref(),
            index,
        );
        let exact_backward = apply_exact_add_inverse(
            left,
            right,
            &current,
            integer_type,
            coefficient,
            offset,
            landed.as_ref(),
            index,
        );
        let exact_backward_reverse = apply_exact_add_inverse(
            right,
            left,
            &current,
            integer_type,
            coefficient,
            offset,
            landed.as_ref(),
            index,
        );
        let candidates = [
            forward,
            reverse,
            shift_forward,
            shift_reverse,
            wrapping_backward,
            wrapping_backward_reverse,
            exact_backward,
            exact_backward_reverse,
        ]
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
        // `x & mask` carries no affine slope, but a non-negative mask pins
        // the whole image to `[0, mask]`: the result keeps only mask bits.
        // A negative signed mask leaves the sign bit settable, so the step
        // rejects rather than pretend the image is ordered.
        ScalarTerm::IntegerBitwiseAnd {
            scalar_type,
            left,
            right,
        } if *scalar_type == integer_type && left.as_ref() == current => {
            let (mask, used_landing) = signed_literal(right, integer_type, landed)?;
            if mask < 0 {
                return Some(Err(IntegerAffineWitnessError::NegativeBitwiseAndMask));
            }
            (
                Some(coefficient),
                Some(offset),
                used_landing,
                CheckedIntegerEndpointStep::BitwiseAndMask(mask),
            )
        }
        ScalarTerm::IntegerBitwiseAnd {
            scalar_type,
            left,
            right,
        } if *scalar_type == integer_type && right.as_ref() == current => {
            let (mask, used_landing) = signed_literal(left, integer_type, landed)?;
            if mask < 0 {
                return Some(Err(IntegerAffineWitnessError::NegativeBitwiseAndMask));
            }
            (
                Some(coefficient),
                Some(offset),
                used_landing,
                CheckedIntegerEndpointStep::BitwiseAndMask(mask),
            )
        }
        // Only unsigned fixed carriers traverse a wrapping definition: signed
        // wrapping arithmetic is not monotone around its reduced endpoints,
        // and address carriers have no literal order evidence here.
        ScalarTerm::WrappingIntegerAdd {
            scalar_type,
            left,
            right,
        } if *scalar_type == integer_type
            && integer_type.sign() == IntegerSign::Unsigned
            && left.as_ref() == current =>
        {
            let (literal, used_landing) = signed_literal(right, integer_type, landed)?;
            (
                Some(coefficient),
                offset.checked_add(literal),
                used_landing,
                CheckedIntegerEndpointStep::WrappingAdd {
                    operand: left.as_ref().clone(),
                    literal,
                },
            )
        }
        ScalarTerm::WrappingIntegerAdd {
            scalar_type,
            left,
            right,
        } if *scalar_type == integer_type
            && integer_type.sign() == IntegerSign::Unsigned
            && right.as_ref() == current =>
        {
            let (literal, used_landing) = signed_literal(left, integer_type, landed)?;
            (
                Some(coefficient),
                offset.checked_add(literal),
                used_landing,
                CheckedIntegerEndpointStep::WrappingAdd {
                    operand: right.as_ref().clone(),
                    literal,
                },
            )
        }
        // Unsigned wrapping division by a landed nonzero literal is exact
        // floor division: both bound directions are monotone and need no
        // evidence conjunct. A zero divisor still rejects so the chain never
        // pretends a trapping operation has an image.
        ScalarTerm::WrappingIntegerDivide {
            scalar_type,
            left,
            right,
        } if *scalar_type == integer_type
            && integer_type.sign() == IntegerSign::Unsigned
            && left.as_ref() == current =>
        {
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

/// Traverse an `Equal(defined, wrapping_add(operand, literal))` row toward its
/// operand. Only unsigned fixed carriers qualify, and exactly one addend may
/// be the chain operand: if both addends land as literals the backward step is
/// ambiguous and rejects rather than guessing which value produced the sum.
#[allow(clippy::too_many_arguments)]
fn apply_wrapping_add_inverse(
    defined: &ScalarTerm,
    expression: &ScalarTerm,
    current: &ScalarTerm,
    integer_type: IntegerType,
    coefficient: i128,
    offset: i128,
    landed: Option<&LandedInteger>,
    definition_index: usize,
) -> Option<
    Result<(ScalarTerm, i128, i128, bool, CheckedIntegerEndpointStep), IntegerAffineWitnessError>,
> {
    if !matches!(defined, ScalarTerm::Value { .. }) || defined != current {
        return None;
    }
    let ScalarTerm::WrappingIntegerAdd {
        scalar_type,
        left,
        right,
    } = expression
    else {
        return None;
    };
    if *scalar_type != integer_type || integer_type.sign() != IntegerSign::Unsigned {
        return None;
    }
    let left_operand = if matches!(left.as_ref(), ScalarTerm::Value { .. }) {
        signed_literal(right, integer_type, landed)
    } else {
        None
    };
    let right_operand = if matches!(right.as_ref(), ScalarTerm::Value { .. }) {
        signed_literal(left, integer_type, landed)
    } else {
        None
    };
    let (operand, literal, used_landing) = match (left_operand, right_operand) {
        (Some((literal, used)), None) => (left.as_ref(), literal, used),
        (None, Some((literal, used))) => (right.as_ref(), literal, used),
        (None, None) => return None,
        (Some(_), Some(_)) => {
            return Some(Err(IntegerAffineWitnessError::AmbiguousDefinition(
                definition_index,
            )));
        }
    };
    // `x = defined - literal (mod 2^w)` keeps the coefficient; the mapped
    // offset moves opposite the forward step.
    Some(match offset.checked_sub(literal) {
        Some(offset) => Ok((
            operand.clone(),
            coefficient,
            offset,
            used_landing,
            CheckedIntegerEndpointStep::WrappingAddBackward {
                operand: operand.clone(),
                literal,
            },
        )),
        None => Err(IntegerAffineWitnessError::CoefficientOverflow),
    })
}

/// Traverse an `Equal(defined, exact_add(operand, literal))` row toward its
/// operand. The exact add already carries a checked no-overflow obligation,
/// so `operand = defined - literal` is a true integer equation in either
/// sign: the plain `Subtract` step maps the bound without wrap evidence.
/// Exactly one addend may be the chain operand — two literal addends make the
/// backward step ambiguous and reject rather than guess which produced it.
#[allow(clippy::too_many_arguments)]
fn apply_exact_add_inverse(
    defined: &ScalarTerm,
    expression: &ScalarTerm,
    current: &ScalarTerm,
    integer_type: IntegerType,
    coefficient: i128,
    offset: i128,
    landed: Option<&LandedInteger>,
    definition_index: usize,
) -> Option<
    Result<(ScalarTerm, i128, i128, bool, CheckedIntegerEndpointStep), IntegerAffineWitnessError>,
> {
    if !matches!(defined, ScalarTerm::Value { .. }) || defined != current {
        return None;
    }
    let ScalarTerm::ExactIntegerAdd {
        scalar_type,
        left,
        right,
    } = expression
    else {
        return None;
    };
    if *scalar_type != integer_type {
        return None;
    }
    let left_operand = if matches!(left.as_ref(), ScalarTerm::Value { .. }) {
        signed_literal(right, integer_type, landed)
    } else {
        None
    };
    let right_operand = if matches!(right.as_ref(), ScalarTerm::Value { .. }) {
        signed_literal(left, integer_type, landed)
    } else {
        None
    };
    let (operand, literal, used_landing) = match (left_operand, right_operand) {
        (Some((literal, used)), None) => (left.as_ref(), literal, used),
        (None, Some((literal, used))) => (right.as_ref(), literal, used),
        (None, None) => return None,
        (Some(_), Some(_)) => {
            return Some(Err(IntegerAffineWitnessError::AmbiguousDefinition(
                definition_index,
            )));
        }
    };
    // `operand = defined - literal` keeps the coefficient; the mapped offset
    // moves opposite the forward step, and the `Subtract` endpoint step maps
    // the bound's literal the same way.
    Some(match offset.checked_sub(literal) {
        Some(offset) => Ok((
            operand.clone(),
            coefficient,
            offset,
            used_landing,
            CheckedIntegerEndpointStep::Subtract(literal),
        )),
        None => Err(IntegerAffineWitnessError::CoefficientOverflow),
    })
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
    NegativeBitwiseAndMask,
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
