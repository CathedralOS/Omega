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
    Some(integer_range_requirement(*bounds, value))
}

/// Atomic validity is independent of ordinary scalar store eligibility.
pub fn atomic_resident_requirement(field: &StructuralFieldType, value: ValueId) -> Option<Proposition> {
    let StructuralFieldType::AtomicBoundedInteger(bounds) = field else { return None; };
    Some(integer_range_requirement(*bounds, value))
}

fn integer_range_requirement(bounds: semantic_vocabulary::BoundedIntegerType, value: ValueId) -> Proposition {
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
    Proposition::Conjunction(clauses)
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
    let Some(requirement) = scalar_field_requirement(field_type, ValueId::FIRST)
        .or_else(|| atomic_resident_requirement(field_type, ValueId::FIRST)) else {
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
    use semantic_vocabulary::{IeeeFloatFormat, IeeeFloatValue};
    let Some(carrier) = field_type.scalar_type() else {
        return false;
    };
    let zero = match carrier {
        ScalarType::Boolean => ScalarTerm::Boolean(false),
        ScalarType::Integer(integer) => ScalarTerm::Integer {
            scalar_type: integer,
            value: integer.zero(),
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
