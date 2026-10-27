//! Checking an integer affine witness: normalizing the ordered endpoint
//! chain, replaying each correlated definition, and landing literals.

use crate::integer_rules::fixed_integer::{fixed_integer_as_i128, integer_value_as_i128};
use crate::integer_rules::integer_affine::bound_mapping::direct_math_leaf;
use crate::term_matchers::*;
use semantic_vocabulary::{
    IntegerCarrier, IntegerSign, IntegerType, IntegerValue, Proposition, PropositionContext,
    ScalarTerm, ScalarType,
};
use terminal_psi::IntegerAffineWitness;

mod correlated;
mod definitions;
use correlated::*;
use definitions::*;

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
    /// Forward saturation is a monotone clamped translation. It maps
    /// inclusive endpoints, but a plateau cannot preserve a strict bound or
    /// be inverted as an ordinary translation.
    SaturatingAdd(i128),
    /// `target = wrapping_add(operand, literal)` traversed toward the defined
    /// result. For a positive literal an upper mapped bound is unconditional
    /// because reduction modulo the width can only lower the result; a lower
    /// mapped bound is sound only when the same definition carries the
    /// checked no-wrap conjunct `operand <= maximum - literal`. A negative
    /// literal (a signed addend, or a wrapping subtraction of a literal on
    /// either carrier sign) mirrors both sides with
    /// `minimum - literal <= operand`.
    WrappingAdd {
        operand: ScalarTerm,
        literal: i128,
    },
    /// The same equation traversed toward the operand: for a positive literal
    /// a lower mapped bound is unconditional, while an upper mapped bound is
    /// sound only under the checked `operand <= maximum - literal` evidence;
    /// a negative literal mirrors both sides.
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
    /// The same reading when the mask operand is not a literal: an AND
    /// still keeps only mask bits, so once prior axioms prove
    /// `0 <= mask` and `mask <= upper` — on the operand itself or on a
    /// term an `Equal` axiom aliases it to — the result image is
    /// `[0, upper]` whatever range the operand carried.
    BitwiseAndBoundedMask {
        mask: ScalarTerm,
        upper: i128,
    },
    /// `x | literal` traversed toward the defined result, with the literal a
    /// checked non-negative spelling. An OR never clears bits, so the result
    /// stays at or above its operand; and it sets no bit above the wider
    /// operand bound, so `0 <= x <= upper` maps the result to
    /// `[x, all_ones(bit_length(max(upper, literal)))]`. A negative signed
    /// literal forces the sign bit and leaves the image unordered, so the
    /// step rejects while the witness is checked.
    BitwiseOrLiteralImage(i128),
    /// The same reading when the sibling operand is not a literal: prior
    /// axioms proving `0 <= sibling` and `sibling <= upper` — on the operand
    /// itself or on a term an `Equal` axiom aliases it to — admit the image
    /// `[x, all_ones(bit_length(max(upper_x, upper)))]`.
    BitwiseOrBoundedImage {
        operand: ScalarTerm,
        upper: i128,
    },
    CorrelatedAddLower,
    CorrelatedAddUpper,
    CorrelatedSubtractLower,
    CorrelatedSubtractUpper,
    CorrelatedUnsignedSubtract,
    /// A positive runtime divisor bounds the exact unsigned remainder image.
    CorrelatedUnsignedRemainder,
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

/// The degree-1/offset-0 form returned whenever the witness root meets a
/// direct fixed-integer leaf pair: no axioms or endpoint steps are needed
/// and the target is retained whole.
fn trivial_affine_form(
    witness: &IntegerAffineWitness,
    integer_type: IntegerType,
) -> CheckedIntegerAffineForm {
    CheckedIntegerAffineForm {
        root: witness.root.clone(),
        target: witness.target.clone(),
        integer_type,
        coefficient: 1,
        offset: 0,
        endpoint_steps: Vec::new(),
    }
}

pub fn check_integer_affine_witness(
    context: &PropositionContext,
    semantic_axioms: &[Proposition],
    witness: &IntegerAffineWitness,
) -> Result<CheckedIntegerAffineForm, IntegerAffineWitnessError> {
    if let Some(form) = super::remainder::check_witness(context, semantic_axioms, witness)? {
        return Ok(form);
    }
    if let Some(form) = check_correlated_add_witness(context, semantic_axioms, witness)? {
        return Ok(form);
    }
    if let Some(form) = check_correlated_subtract_witness(context, semantic_axioms, witness)? {
        return Ok(form);
    }
    if let Some(form) = check_correlated_multiply_witness(context, semantic_axioms, witness)? {
        return Ok(form);
    }
    // A contract can retain the remainder expression itself instead of an
    // intermediate SSA definition. Its literal divisor and exact dividend
    // identify the same endpoint transform without an invented axiom. Signed
    // division by -1 needs a separate MIN exclusion; type validation alone
    // does not supply that formation evidence, so leave it to named chains.
    if witness.definition_axioms.is_empty()
        && witness.literal_axioms.is_empty()
        && let ScalarTerm::ExactIntegerRemainder {
            scalar_type,
            left,
            right,
        } = &witness.target
        && scalar_type.carrier() == IntegerCarrier::Fixed
        && matches!(witness.root, ScalarTerm::Value { .. })
        && left.as_ref() == &witness.root
        && matches!(right.as_ref(), ScalarTerm::Integer { .. })
        && let Some((divisor, _)) = signed_literal(right, *scalar_type, None)
        && (scalar_type.sign() == IntegerSign::Unsigned || divisor != -1)
    {
        context
            .validate(&Proposition::Equal(
                witness.root.clone(),
                witness.target.clone(),
            ))
            .map_err(IntegerAffineWitnessError::MalformedProposition)?;
        if divisor == 0 {
            return Err(IntegerAffineWitnessError::ZeroDivisionLiteral);
        }
        return Ok(CheckedIntegerAffineForm {
            root: witness.root.clone(),
            target: witness.target.clone(),
            integer_type: *scalar_type,
            coefficient: 1,
            offset: 0,
            endpoint_steps: vec![CheckedIntegerEndpointStep::Remainder(divisor)],
        });
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
        }
        | ScalarTerm::WrappingIntegerMultiply {
            scalar_type,
            left,
            right,
        } = &witness.target
        && (left.as_ref() == &witness.root || right.as_ref() == &witness.root)
        && scalar_type.carrier() == IntegerCarrier::Fixed
        && direct_math_leaf(left, *scalar_type).is_some()
        && direct_math_leaf(right, *scalar_type).is_some()
    {
        return Ok(trivial_affine_form(witness, *scalar_type));
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
        return Ok(trivial_affine_form(witness, *scalar_type));
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
        return Ok(trivial_affine_form(witness, *scalar_type));
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
    let Some(integer_type) = witness.root.scalar_type().as_integer() else {
        return Err(IntegerAffineWitnessError::RootNotInteger);
    };
    if integer_type.carrier() != IntegerCarrier::Fixed {
        return Err(IntegerAffineWitnessError::UnsupportedCarrier(integer_type));
    }
    if witness.target.scalar_type() != ScalarType::Integer(integer_type) {
        return Err(IntegerAffineWitnessError::TargetTypeMismatch);
    }
    if witness.definition_axioms.is_empty()
        && apply_policy_expression_target(&witness.target, &witness.root, integer_type, 1, 0)
            .is_none()
    {
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
    for (&index, &literal_index) in witness
        .definition_axioms
        .iter()
        .zip(&witness.literal_axioms)
    {
        let proposition = checked_axiom(context, semantic_axioms, index)?;
        let Proposition::Equal(left, right) = proposition else {
            return Err(IntegerAffineWitnessError::DefinitionNotEquality(index));
        };
        let landed = literal_index
            .map(|literal_index| landed_integer(context, semantic_axioms, index, literal_index))
            .transpose()?;
        let candidates = [
            both_orientations(left, right, |first, second| {
                apply_definition(
                    first,
                    second,
                    &current,
                    integer_type,
                    coefficient,
                    offset,
                    landed.as_ref(),
                    &semantic_axioms[..index],
                )
            }),
            both_orientations(left, right, |first, second| {
                apply_shift_definition(
                    first,
                    second,
                    &current,
                    integer_type,
                    landed.as_ref(),
                    index,
                )
            }),
            both_orientations(left, right, |first, second| {
                apply_wrapping_add_inverse(
                    first,
                    second,
                    &current,
                    integer_type,
                    coefficient,
                    offset,
                    landed.as_ref(),
                    index,
                )
            }),
            both_orientations(left, right, |first, second| {
                apply_exact_add_inverse(
                    first,
                    second,
                    &current,
                    integer_type,
                    coefficient,
                    offset,
                    landed.as_ref(),
                    index,
                )
            }),
        ]
        .into_iter()
        .flat_map(|orientations| orientations.into_iter().flatten())
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
        // The target may itself be one wrapping step over the chain's last
        // value, spelled in place rather than defined by an SSA row: a
        // guarantee naming `self.length - 1`. The term is its own
        // definition, so the step is read from it and cites no axiom.
        let (next_coefficient, next_offset, endpoint_step) = apply_policy_expression_target(
            &witness.target,
            &current,
            integer_type,
            coefficient,
            offset,
        )
        .ok_or(IntegerAffineWitnessError::TargetMismatch)??;
        coefficient = next_coefficient;
        offset = next_offset;
        endpoint_steps.push(endpoint_step);
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

fn policy_add_step(
    expression: &ScalarTerm,
    operand: &ScalarTerm,
    literal: i128,
) -> CheckedIntegerEndpointStep {
    if matches!(
        expression,
        ScalarTerm::SaturatingIntegerAdd { .. } | ScalarTerm::SaturatingIntegerSubtract { .. }
    ) {
        CheckedIntegerEndpointStep::SaturatingAdd(literal)
    } else {
        CheckedIntegerEndpointStep::WrappingAdd {
            operand: operand.clone(),
            literal,
        }
    }
}

/// Read an in-place policy sum or difference through the same step that a
/// defining SSA row uses. Subtraction negates its literal; the selected
/// policy still determines endpoint mapping and preservation of strictness.
fn apply_policy_expression_target(
    target: &ScalarTerm,
    current: &ScalarTerm,
    integer_type: IntegerType,
    coefficient: i128,
    offset: i128,
) -> Option<Result<(i128, i128, CheckedIntegerEndpointStep), IntegerAffineWitnessError>> {
    let (operand, literal) = match target {
        ScalarTerm::WrappingIntegerAdd {
            scalar_type,
            left,
            right,
        }
        | ScalarTerm::SaturatingIntegerAdd {
            scalar_type,
            left,
            right,
        } if *scalar_type == integer_type && left.as_ref() == current => {
            (left, signed_literal(right, integer_type, None)?.0)
        }
        ScalarTerm::WrappingIntegerAdd {
            scalar_type,
            left,
            right,
        }
        | ScalarTerm::SaturatingIntegerAdd {
            scalar_type,
            left,
            right,
        } if *scalar_type == integer_type && right.as_ref() == current => {
            (right, signed_literal(left, integer_type, None)?.0)
        }
        ScalarTerm::WrappingIntegerSubtract {
            scalar_type,
            left,
            right,
        }
        | ScalarTerm::SaturatingIntegerSubtract {
            scalar_type,
            left,
            right,
        } if *scalar_type == integer_type && left.as_ref() == current => {
            let (literal, _) = signed_literal(right, integer_type, None)?;
            (left, literal.checked_neg()?)
        }
        _ => return None,
    };
    Some(
        offset
            .checked_add(literal)
            .map(|offset| {
                (
                    coefficient,
                    offset,
                    policy_add_step(target, operand, literal),
                )
            })
            .ok_or(IntegerAffineWitnessError::CoefficientOverflow),
    )
}

/// The exact literal a term carries: its own closed value in the expected
/// carrier, or the value a landed axiom pinned to that exact term. The flag
/// records which of the two the caller consumed.
fn signed_literal(
    term: &ScalarTerm,
    integer_type: IntegerType,
    landed: Option<&LandedInteger>,
) -> Option<(i128, bool)> {
    if let Some(value) = fixed_integer_as_i128(term, integer_type) {
        return Some((value, false));
    }
    landed
        .filter(|landed| landed.term == *term && landed.integer_type == integer_type)
        .and_then(|landed| integer_value_as_i128(landed.value).map(|value| (value, true)))
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
    let proposition = checked_axiom(context, semantic_axioms, literal_index)?;
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
    NegativeBitwiseOrSibling,
    ShiftCountNotLanded(usize),
    ShiftCountOutsideValueWidth(usize),
    TargetMismatch,
}

crate::debug_display!(IntegerAffineWitnessError);
crate::checked_axiom!(IntegerAffineWitnessError);

impl std::error::Error for IntegerAffineWitnessError {}
