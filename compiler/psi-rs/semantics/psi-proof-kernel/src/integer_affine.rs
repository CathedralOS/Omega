//! Independently checked normalization for ordered same-carrier affine facts.
//!
//! This is a certificate prerequisite, not an arithmetic proof rule. It binds
//! a producer's normalized `A * root + B` claim to exact, prior semantic-axiom
//! rows so later proof rules do not need to trust an analyzer's coefficients.

use psi_core::{
    IntegerCarrier, IntegerSign, IntegerType, IntegerValue, Proposition, PropositionContext,
    ScalarTerm, ScalarType,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntegerAffineWitness {
    pub root: ScalarTerm,
    pub target: ScalarTerm,
    pub definition_axioms: Vec<usize>,
    /// One optional, earlier equality landing the non-chain operand at each
    /// affine definition. The vector is position-aligned with
    /// `definition_axioms`; `None` means that definition embeds its literal.
    pub literal_axioms: Vec<Option<usize>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedIntegerAffineForm {
    root: ScalarTerm,
    target: ScalarTerm,
    integer_type: IntegerType,
    coefficient: i128,
    offset: i128,
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
    if !matches!(witness.root, ScalarTerm::Value { .. }) {
        return Err(IntegerAffineWitnessError::RootNotValue);
    }
    let ScalarType::Integer(integer_type) = witness.root.scalar_type() else {
        return Err(IntegerAffineWitnessError::RootNotInteger);
    };
    if integer_type.carrier() != IntegerCarrier::Fixed || integer_type.sign() != IntegerSign::Signed
    {
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

    let mut current = witness.root.clone();
    let mut coefficient = 1_i128;
    let mut offset = 0_i128;
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
            .map(|literal_index| {
                landed_literal(context, semantic_axioms, integer_type, index, literal_index)
            })
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
        let (next, next_coefficient, next_offset, used_landing) = match (forward, reverse) {
            (Some(next), None) | (None, Some(next)) => next?,
            (None, None) => return Err(IntegerAffineWitnessError::DefinitionShapeMismatch(index)),
            (Some(_), Some(_)) => {
                return Err(IntegerAffineWitnessError::AmbiguousDefinition(index));
            }
        };
        if landed.is_some() != used_landing {
            return Err(IntegerAffineWitnessError::UnusedLiteralAxiom(index));
        }
        current = next;
        coefficient = next_coefficient;
        offset = next_offset;
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
    })
}

fn apply_definition(
    target: &ScalarTerm,
    expression: &ScalarTerm,
    current: &ScalarTerm,
    integer_type: IntegerType,
    coefficient: i128,
    offset: i128,
    landed: Option<&(ScalarTerm, i128)>,
) -> Option<Result<(ScalarTerm, i128, i128, bool), IntegerAffineWitnessError>> {
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
            (Some(coefficient), offset.checked_add(literal), used_landing)
        }
        ScalarTerm::ExactIntegerAdd {
            scalar_type,
            left,
            right,
        } if *scalar_type == integer_type && right.as_ref() == current => {
            let (literal, used_landing) = signed_literal(left, integer_type, landed)?;
            (Some(coefficient), offset.checked_add(literal), used_landing)
        }
        ScalarTerm::ExactIntegerSubtract {
            scalar_type,
            left,
            right,
        } if *scalar_type == integer_type && left.as_ref() == current => {
            let (literal, used_landing) = signed_literal(right, integer_type, landed)?;
            (Some(coefficient), offset.checked_sub(literal), used_landing)
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
            )
        }
        _ => return None,
    };
    Some(match transformed {
        (Some(coefficient), Some(offset), used_landing) => {
            Ok((target.clone(), coefficient, offset, used_landing))
        }
        _ => Err(IntegerAffineWitnessError::CoefficientOverflow),
    })
}

fn signed_literal(
    term: &ScalarTerm,
    integer_type: IntegerType,
    landed: Option<&(ScalarTerm, i128)>,
) -> Option<(i128, bool)> {
    if let Some((actual_type, IntegerValue::Signed(value))) = term.integer_value()
        && actual_type == integer_type
    {
        return Some((value, false));
    }
    landed
        .filter(|(value, _)| value == term)
        .map(|(_, literal)| (*literal, true))
}

fn landed_literal(
    context: &PropositionContext,
    semantic_axioms: &[Proposition],
    integer_type: IntegerType,
    definition_index: usize,
    literal_index: usize,
) -> Result<(ScalarTerm, i128), IntegerAffineWitnessError> {
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
            && value.scalar_type() == ScalarType::Integer(integer_type)
            && let Some((literal, false)) = signed_literal(literal, integer_type, None)
        {
            return Ok((value.clone(), literal));
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
    MalformedProposition(psi_core::PropositionError),
    DefinitionNotEquality(usize),
    LiteralAxiomNotPrior { definition: usize, literal: usize },
    LiteralAxiomNotEquality(usize),
    LiteralAxiomShapeMismatch(usize),
    UnusedLiteralAxiom(usize),
    DefinitionShapeMismatch(usize),
    AmbiguousDefinition(usize),
    CoefficientOverflow,
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
/// only that the already-normalized affine form maps that exact bound to the
/// claimed target relation.
pub fn check_integer_affine_bound_conversion(
    form: &CheckedIntegerAffineForm,
    root_bound: &Proposition,
    conclusion: &Proposition,
) -> Result<(), IntegerAffineBoundConversionError> {
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
    let Some((bound, false)) = signed_literal(bound, form.integer_type(), None) else {
        return Err(IntegerAffineBoundConversionError::RootBoundNotTypedLiteral);
    };
    let mapped = form
        .coefficient()
        .checked_mul(bound)
        .and_then(|value| value.checked_add(form.offset()))
        .ok_or(IntegerAffineBoundConversionError::MappedBoundOverflow)?;
    let mapped = ScalarTerm::integer(form.integer_type(), IntegerValue::Signed(mapped))
        .map_err(|_| IntegerAffineBoundConversionError::MappedBoundOutsideCarrier)?;

    // Positive forms preserve order, negative forms reverse it. A constant
    // form can soundly provide either orientation; retaining the root bound's
    // orientation makes that choice deterministic.
    let target_is_left = if form.coefficient() < 0 {
        root_is_lower_endpoint
    } else {
        !root_is_lower_endpoint
    };
    let expected = if target_is_left {
        Proposition::LessOrEqual(form.target().clone(), mapped)
    } else {
        Proposition::LessOrEqual(mapped, form.target().clone())
    };
    if conclusion != &expected {
        return Err(IntegerAffineBoundConversionError::ConclusionMismatch);
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IntegerAffineBoundConversionError {
    RootBoundNotLessOrEqual,
    RootBoundMismatch,
    RootBoundNotTypedLiteral,
    MappedBoundOverflow,
    MappedBoundOutsideCarrier,
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
    use psi_core::{IntegerSign, ValueId};

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
            (1..=3).map(|id| (ValueId::new(id).unwrap(), ScalarType::Integer(integer_type))),
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
    fn rejects_non_value_roots_unsupported_carriers_and_checked_overflow() {
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
            Err(IntegerAffineWitnessError::UnsupportedCarrier(u8_type)),
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
                    coefficient: i128::MAX,
                    offset: i128::MAX,
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
                },
                &Proposition::LessOrEqual(value(1, integer_type), literal(100)),
                &Proposition::LessOrEqual(value(2, integer_type), literal(127)),
            ),
            Err(IntegerAffineBoundConversionError::MappedBoundOutsideCarrier),
        );
    }
}
