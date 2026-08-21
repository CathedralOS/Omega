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

    let mut current = witness.root.clone();
    let mut coefficient = 1_i128;
    let mut offset = 0_i128;
    let mut previous = None;
    for &index in &witness.definition_axioms {
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
        let forward = apply_definition(left, right, &current, integer_type, coefficient, offset);
        let reverse = apply_definition(right, left, &current, integer_type, coefficient, offset);
        let (next, next_coefficient, next_offset) = match (forward, reverse) {
            (Some(next), None) | (None, Some(next)) => next?,
            (None, None) => return Err(IntegerAffineWitnessError::DefinitionShapeMismatch(index)),
            (Some(_), Some(_)) => {
                return Err(IntegerAffineWitnessError::AmbiguousDefinition(index));
            }
        };
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
) -> Option<Result<(ScalarTerm, i128, i128), IntegerAffineWitnessError>> {
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
            let literal = signed_literal(right, integer_type)?;
            (Some(coefficient), offset.checked_add(literal))
        }
        ScalarTerm::ExactIntegerAdd {
            scalar_type,
            left,
            right,
        } if *scalar_type == integer_type && right.as_ref() == current => {
            let literal = signed_literal(left, integer_type)?;
            (Some(coefficient), offset.checked_add(literal))
        }
        ScalarTerm::ExactIntegerSubtract {
            scalar_type,
            left,
            right,
        } if *scalar_type == integer_type && left.as_ref() == current => {
            let literal = signed_literal(right, integer_type)?;
            (Some(coefficient), offset.checked_sub(literal))
        }
        ScalarTerm::ExactIntegerMultiply {
            scalar_type,
            left,
            right,
        } if *scalar_type == integer_type && left.as_ref() == current => {
            let literal = signed_literal(right, integer_type)?;
            (
                coefficient.checked_mul(literal),
                offset.checked_mul(literal),
            )
        }
        ScalarTerm::ExactIntegerMultiply {
            scalar_type,
            left,
            right,
        } if *scalar_type == integer_type && right.as_ref() == current => {
            let literal = signed_literal(left, integer_type)?;
            (
                coefficient.checked_mul(literal),
                offset.checked_mul(literal),
            )
        }
        _ => return None,
    };
    Some(match transformed {
        (Some(coefficient), Some(offset)) => Ok((target.clone(), coefficient, offset)),
        _ => Err(IntegerAffineWitnessError::CoefficientOverflow),
    })
}

fn signed_literal(term: &ScalarTerm, integer_type: IntegerType) -> Option<i128> {
    match term.integer_value()? {
        (actual_type, IntegerValue::Signed(value)) if actual_type == integer_type => Some(value),
        _ => None,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IntegerAffineWitnessError {
    RootNotValue,
    RootNotInteger,
    UnsupportedCarrier(IntegerType),
    TargetTypeMismatch,
    EmptyDefinitionChain,
    NonCanonicalDefinitionOrder,
    UnknownSemanticAxiom(usize),
    MalformedProposition(psi_core::PropositionError),
    DefinitionNotEquality(usize),
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
            },
        )
        .expect("ordered affine witness");
        assert_eq!(checked.root(), &root);
        assert_eq!(checked.target(), &target);
        assert_eq!(checked.coefficient(), -3);
        assert_eq!(checked.offset(), -15);
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
        let witness = |definition_axioms| IntegerAffineWitness {
            root: root.clone(),
            target: target.clone(),
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
                },
            ),
            Err(IntegerAffineWitnessError::CoefficientOverflow),
        );
    }
}
