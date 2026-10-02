//! Declared scalar refinements as subject-bound predicates. This query
//! describes obligations; call, return, state and local owners must prove them
//! against their actual values. Semantic tags and arithmetic policies retain
//! their separate qualification owners.

use checked_trees::{CheckedBooleanExpression, CheckedScalarExpression};
use typed_trees::TypedTrees;
use typed_trees::types::{
    DomainConstraintSubject, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
};

pub fn scalar_type_refinement_predicates(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
    subject: CheckedScalarExpression,
) -> Option<Vec<CheckedBooleanExpression>> {
    let primitive = subject.primitive_type()?;
    if program.primitive_type_reference(reference) != Some(primitive) {
        return None;
    }
    let mut current = reference;
    let mut active = Vec::new();
    let mut predicates = Vec::new();
    loop {
        if !program
            .type_reference_table
            .contains_type_reference(current)
            || active.contains(&current)
        {
            return None;
        }
        active.push(current);
        let (base_type, constraints) = match program.type_reference_table.type_reference(current) {
            TypeReferenceNode::Named { .. } => return Some(predicates),
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => (*base_type, *constraints),
            _ => return None,
        };
        for constraint in program.type_reference_table.constraint_span(constraints)? {
            let bounds = match constraint {
                TypeConstraintNode::ArithmeticDomain(_) => continue,
                TypeConstraintNode::Domain(domain) => match domain.subject {
                    DomainConstraintSubject::Value(domain) => {
                        predicates.push(scalar_value_domain_predicate(domain, subject.clone())?);
                        continue;
                    }
                    DomainConstraintSubject::Declared => {
                        if let Some(bounds) = crate::exact_declared_domain_carrier_interval(
                            program, primitive, domain,
                        ) {
                            bounds
                        } else if domain.predicate_body.is_present() {
                            return None;
                        } else {
                            continue;
                        }
                    }
                    _ => return None,
                },
                TypeConstraintNode::Range {
                    minimum,
                    maximum,
                    end_inclusive,
                } => (
                    crate::closed_integer_range_bound(program, *minimum)?,
                    crate::closed_integer_range_maximum(program, *maximum, *end_inclusive)?,
                ),
                _ => return None,
            };
            let minimum = crate::land_integer_value(&bounds.0, primitive)?;
            let maximum = crate::land_integer_value(&bounds.1, primitive)?;
            predicates.push(CheckedBooleanExpression::And {
                left: Box::new(CheckedBooleanExpression::IntegerComparison {
                    kind: checked_trees::CheckedIntegerComparisonKind::LessOrEqual,
                    left: Box::new(CheckedScalarExpression::IntegerLiteral { literal: minimum }),
                    right: Box::new(subject.clone()),
                }),
                right: Box::new(CheckedBooleanExpression::IntegerComparison {
                    kind: checked_trees::CheckedIntegerComparisonKind::LessOrEqual,
                    left: Box::new(subject.clone()),
                    right: Box::new(CheckedScalarExpression::IntegerLiteral { literal: maximum }),
                }),
            });
        }
        current = base_type;
    }
}

/// Interpret a scalar value domain as an ordinary predicate over its exact
/// subject. Call entries, return guarantees and state arrivals share this
/// meaning; constructing it is not evidence that the subject satisfies it.
pub fn scalar_value_domain_predicate(
    domain: language_semantics::value_domain::ValueDomain,
    subject: checked_trees::CheckedScalarExpression,
) -> Option<checked_trees::CheckedBooleanExpression> {
    use checked_trees::{
        CheckedBooleanExpression, CheckedIeeeFloatComparisonKind, CheckedScalarExpression,
    };
    use semantic_vocabulary::IeeeFloatValue;
    use typed_trees::types::PrimitiveType;
    let primitive_type = subject.primitive_type()?;
    match domain {
        language_semantics::value_domain::ValueDomain::Finite => {}
    }
    let (lowest, largest) = match primitive_type {
        PrimitiveType::F32 => (
            IeeeFloatValue::Binary32((-f32::MAX).to_bits()),
            IeeeFloatValue::Binary32(f32::MAX.to_bits()),
        ),
        PrimitiveType::F64 => (
            IeeeFloatValue::Binary64((-f64::MAX).to_bits()),
            IeeeFloatValue::Binary64(f64::MAX.to_bits()),
        ),
        _ => return None,
    };
    let subject = || Box::new(subject.clone());
    Some(CheckedBooleanExpression::And {
        left: Box::new(CheckedBooleanExpression::ScalarIeeeFloatComparison {
            kind: CheckedIeeeFloatComparisonKind::LessOrEqual,
            left: Box::new(CheckedScalarExpression::IeeeFloatLiteral { value: lowest }),
            right: subject(),
        }),
        right: Box::new(CheckedBooleanExpression::ScalarIeeeFloatComparison {
            kind: CheckedIeeeFloatComparisonKind::LessOrEqual,
            left: subject(),
            right: Box::new(CheckedScalarExpression::IeeeFloatLiteral { value: largest }),
        }),
    })
}
