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
    Some(scalar_type_refinements(program, reference, subject)?.predicates)
}

/// A scalar result can use the ordinary value-return route when every
/// refinement has a complete predicate meaning and no semantic atom needs
/// separate result custody. This describes the contract; returning paths
/// still owe every predicate. In particular, finite floats and integer ranges
/// share the same decoder that normal-return guarantee emission consumes.
pub fn scalar_result_contract_is_represented(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
) -> bool {
    let Some(primitive_type) = program.primitive_type_reference(reference) else {
        return false;
    };
    scalar_type_refinements(
        program,
        reference,
        CheckedScalarExpression::Parameter {
            position: 0,
            primitive_type,
        },
    )
    .is_some_and(|refinements| !refinements.has_semantic_atoms)
}

struct ScalarTypeRefinements {
    predicates: Vec<CheckedBooleanExpression>,
    has_semantic_atoms: bool,
}

fn scalar_type_refinements(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
    subject: CheckedScalarExpression,
) -> Option<ScalarTypeRefinements> {
    let primitive = subject.primitive_type()?;
    if program.primitive_type_reference(reference) != Some(primitive) {
        return None;
    }
    let mut current = reference;
    let mut active = Vec::new();
    let mut predicates = Vec::new();
    let mut has_semantic_atoms = false;
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
            // A selector's trait application refines nothing about its word.
            TypeReferenceNode::Named { .. } | TypeReferenceNode::DynamicTrait { .. } => {
                return Some(ScalarTypeRefinements {
                    predicates,
                    has_semantic_atoms,
                });
            }
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
                            has_semantic_atoms = true;
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
