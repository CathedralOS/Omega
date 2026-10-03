//! Declaration-owned scalar field invariants, instantiated at an exact value.
//!
//! Record construction, case construction and replacement stores owe the same
//! requirement. Neither the initializer's source arrangement nor an observation
//! of the overwritten field changes it. This function describes the goal; the
//! verifier still authenticates the declaration, operand carrier and proof.

use semantic_vocabulary::{Proposition, PropositionContext, ScalarTerm, ScalarType, ValueId};
use terminal_psi::StructuralFieldType;

/// Whether the declaration needs a scalar proof operand. Formation and emission
/// can check the obligation roster without allocating an instantiated formula.
pub fn scalar_field_requires_evidence(field_type: &StructuralFieldType) -> bool {
    matches!(
        field_type,
        StructuralFieldType::BoundedInteger(_) | StructuralFieldType::RefinedScalar(_)
    )
}

/// The restriction a scalar initializer or replacement must establish, beyond
/// its carrier type. `None` means the declaration has no scalar refinement;
/// byte predicates and structural custody have their own obligation owners.
pub fn scalar_field_requirement(
    field_type: &StructuralFieldType,
    value: ValueId,
) -> Option<Proposition> {
    if let StructuralFieldType::RefinedScalar(refinement) = field_type {
        return Some(refinement.instantiate(value));
    }
    let StructuralFieldType::BoundedInteger(bounds) = field_type else {
        return None;
    };
    let integer_type = bounds.integer_type();
    let subject = ScalarTerm::value(value, ScalarType::Integer(integer_type));
    let endpoint = |value| ScalarTerm::Integer {
        scalar_type: integer_type,
        value,
    };
    let mut clauses = vec![
        Proposition::LessOrEqual(endpoint(bounds.minimum()), subject.clone()),
        Proposition::LessOrEqual(subject, endpoint(bounds.maximum())),
    ];
    clauses.sort();
    Some(Proposition::Conjunction(clauses))
}

/// Check supplied contents without creating a premise from their declared type.
/// The sole substitution equation comes from an actual literal. Ordinary checked
/// predicate denotation owns arithmetic and Boolean meaning; an undecided or
/// oversized predicate rejects rather than acquiring truth from a type label.
pub fn scalar_field_accepts_literal(
    field_type: &StructuralFieldType,
    literal: &ScalarTerm,
) -> bool {
    if !matches!(
        literal,
        ScalarTerm::Boolean(_) | ScalarTerm::Integer { .. } | ScalarTerm::IeeeFloatLiteral(_)
    ) || field_type.scalar_type() != Some(literal.scalar_type())
    {
        return false;
    }
    let Ok(context) =
        PropositionContext::from_value_types([(ValueId::FIRST, literal.scalar_type())])
    else {
        return false;
    };
    let equation = Proposition::Equal(
        ScalarTerm::value(ValueId::FIRST, literal.scalar_type()),
        literal.clone(),
    );
    if context.validate(&equation).is_err() {
        return false;
    }
    let Some(requirement) = scalar_field_requirement(field_type, ValueId::FIRST) else {
        return true;
    };
    proof_admission::check_predicate_denotations_with_value_equalities(
        &context,
        &requirement,
        &[],
        &[equation],
    )
    .is_ok_and(|checked| *checked.goal() == Proposition::Truth)
}

/// Zero provisioning is value admission, not a property of the scalar layout.
pub fn scalar_field_accepts_zero(field_type: &StructuralFieldType) -> bool {
    use semantic_vocabulary::{IeeeFloatFormat, IeeeFloatValue, IntegerSign, IntegerValue};
    let Some(carrier) = field_type.scalar_type() else {
        return false;
    };
    let zero = match carrier {
        ScalarType::Boolean => ScalarTerm::Boolean(false),
        ScalarType::Integer(integer) => ScalarTerm::Integer {
            scalar_type: integer,
            value: match integer.sign() {
                IntegerSign::Signed => IntegerValue::Signed(0),
                IntegerSign::Unsigned => IntegerValue::Unsigned(0),
            },
        },
        ScalarType::IeeeFloat(IeeeFloatFormat::Binary32) => {
            ScalarTerm::IeeeFloatLiteral(IeeeFloatValue::Binary32(0))
        }
        ScalarType::IeeeFloat(IeeeFloatFormat::Binary64) => {
            ScalarTerm::IeeeFloatLiteral(IeeeFloatValue::Binary64(0))
        }
    };
    scalar_field_accepts_literal(field_type, &zero)
}

#[cfg(test)]
mod tests {
    use super::*;
    use semantic_vocabulary::{BoundedIntegerType, IntegerSign, IntegerType, IntegerValue};

    #[test]
    fn predicate_contents_are_checked_from_literals_not_declaration_names() {
        let integer = IntegerType::new(IntegerSign::Signed, 32).unwrap();
        let carrier = ScalarType::Integer(integer);
        let subject = ScalarTerm::value(ValueId::FIRST, carrier);
        let zero = ScalarTerm::integer(integer, IntegerValue::Signed(0)).unwrap();
        let nonzero = StructuralFieldType::RefinedScalar(
            semantic_vocabulary::ScalarRefinement::new(
                carrier,
                Proposition::Disjunction(vec![
                    Proposition::LessThan(subject.clone(), zero.clone()),
                    Proposition::LessThan(zero.clone(), subject.clone()),
                ]),
            )
            .unwrap(),
        );
        assert!(!scalar_field_accepts_zero(&nonzero));
        assert!(!scalar_field_accepts_literal(&nonzero, &subject));
        for value in [-7, 7] {
            assert!(scalar_field_accepts_literal(
                &nonzero,
                &ScalarTerm::integer(integer, IntegerValue::Signed(value)).unwrap()
            ));
        }
        let nonnegative = StructuralFieldType::RefinedScalar(
            semantic_vocabulary::ScalarRefinement::new(
                carrier,
                Proposition::LessOrEqual(zero, subject),
            )
            .unwrap(),
        );
        assert!(scalar_field_accepts_zero(&nonnegative));
        assert!(!scalar_field_accepts_literal(
            &nonnegative,
            &ScalarTerm::Boolean(false)
        ));
    }

    #[test]
    fn boolean_predicate_zero_admission_keeps_the_declared_truth() {
        for value in [false, true] {
            let field = StructuralFieldType::RefinedScalar(
                semantic_vocabulary::ScalarRefinement::new(
                    ScalarType::Boolean,
                    Proposition::Equal(
                        ScalarTerm::value(ValueId::FIRST, ScalarType::Boolean),
                        ScalarTerm::Boolean(value),
                    ),
                )
                .unwrap(),
            );
            assert_eq!(scalar_field_accepts_zero(&field), !value);
        }
    }

    #[test]
    fn requirement_keeps_the_declaration_and_replacement_identity() {
        let integer = IntegerType::new(IntegerSign::Signed, 32).unwrap();
        let bounds =
            BoundedIntegerType::new(integer, IntegerValue::Signed(-3), IntegerValue::Signed(7))
                .unwrap();
        let field = StructuralFieldType::BoundedInteger(bounds);
        assert!(scalar_field_requires_evidence(&field));
        assert!(!scalar_field_requires_evidence(
            &StructuralFieldType::Scalar(ScalarType::Integer(integer),)
        ));
        let first = ValueId::new(1).unwrap();
        let replacement = ValueId::new(2).unwrap();
        let requirement = scalar_field_requirement(&field, replacement).unwrap();
        assert!(requirement.any_value_id(|value| value == replacement));
        assert!(!requirement.any_value_id(|value| value == first));
        let mut expected = vec![
            Proposition::LessOrEqual(
                ScalarTerm::integer(integer, IntegerValue::Signed(-3)).unwrap(),
                ScalarTerm::value(replacement, ScalarType::Integer(integer)),
            ),
            Proposition::LessOrEqual(
                ScalarTerm::value(replacement, ScalarType::Integer(integer)),
                ScalarTerm::integer(integer, IntegerValue::Signed(7)).unwrap(),
            ),
        ];
        expected.sort();
        assert_eq!(requirement, Proposition::Conjunction(expected));
        assert_eq!(
            scalar_field_requirement(
                &StructuralFieldType::Scalar(ScalarType::Integer(integer)),
                replacement,
            ),
            None,
        );
    }
}
