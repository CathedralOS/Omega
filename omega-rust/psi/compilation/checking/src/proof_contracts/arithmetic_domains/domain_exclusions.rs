//! Excluded integer points in an established scalar declaration.
//!
//! A nonconvex predicate cannot be replaced by its interval hull. Reuse the
//! declared-domain projector and substitute one candidate into its closed
//! predicate; only a false result excludes that candidate. This grants no
//! membership or place availability: the caller must resolve an admitted read's
//! exact declaration before consulting it.

use checked_trees::{
    CheckedBooleanExpression as Boolean, CheckedIntegerComparisonKind as Comparison,
    CheckedScalarExpression as Scalar,
};
use numerics::bignum::BigInt;
use typed_trees::TypedTrees;
use typed_trees::types::{
    PrimitiveType, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
};

pub(super) fn type_excludes_integer(
    program: &TypedTrees,
    mut reference: TypeReferenceHandle,
    value: i64,
) -> bool {
    let mut visited = Vec::new();
    let mut domains = Vec::new();
    let primitive = loop {
        if !program
            .type_reference_table
            .contains_type_reference(reference)
            || visited.contains(&reference)
        {
            return false;
        }
        visited.push(reference);
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Reference { referee, .. } => reference = *referee,
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => {
                let Some(constraints) = program.type_reference_table.constraint_span(*constraints)
                else {
                    return false;
                };
                for constraint in constraints {
                    if let TypeConstraintNode::Domain(domain) = constraint {
                        domains.push(domain);
                    }
                }
                reference = *base_type;
            }
            TypeReferenceNode::Named { .. } => {
                let Some(primitive) =
                    crate::value_custody::recasts::exact_primitive_type(program, reference)
                else {
                    return false;
                };
                break primitive;
            }
            _ => return false,
        }
    };
    if !matches!(
        primitive,
        PrimitiveType::I8
            | PrimitiveType::I16
            | PrimitiveType::I32
            | PrimitiveType::I64
            | PrimitiveType::U8
            | PrimitiveType::U16
            | PrimitiveType::U32
            | PrimitiveType::U64
    ) {
        return false;
    }
    domains.into_iter().any(|domain| {
        crate::scalar_field_domain_predicate(program, domain, primitive)
            .is_some_and(|predicate| predicate_value(&predicate, primitive, value) == Some(false))
    })
}

/// Interpret only the closed vocabulary produced by the declaration projector.
/// Unknown terms stay unknown, including beneath negation or disjunction.
fn predicate_value(predicate: &Boolean, primitive: PrimitiveType, value: i64) -> Option<bool> {
    match predicate {
        Boolean::Constant(value) => Some(*value),
        Boolean::Not(inner) => predicate_value(inner, primitive, value).map(|value| !value),
        Boolean::Equal { left, right } => Some(
            predicate_value(left, primitive, value)? == predicate_value(right, primitive, value)?,
        ),
        Boolean::And { left, right } => match (
            predicate_value(left, primitive, value),
            predicate_value(right, primitive, value),
        ) {
            (Some(false), _) | (_, Some(false)) => Some(false),
            (Some(true), Some(true)) => Some(true),
            _ => None,
        },
        Boolean::Or { left, right } => match (
            predicate_value(left, primitive, value),
            predicate_value(right, primitive, value),
        ) {
            (Some(true), _) | (_, Some(true)) => Some(true),
            (Some(false), Some(false)) => Some(false),
            _ => None,
        },
        Boolean::IntegerComparison { kind, left, right } => {
            let left = scalar_value(left, primitive, value)?;
            let right = scalar_value(right, primitive, value)?;
            Some(match kind {
                Comparison::Equal => left == right,
                Comparison::LessThan => left < right,
                Comparison::LessOrEqual => left <= right,
            })
        }
        _ => None,
    }
}

fn scalar_value(expression: &Scalar, primitive: PrimitiveType, value: i64) -> Option<BigInt> {
    match expression {
        Scalar::Parameter {
            position: 0,
            primitive_type,
        } if *primitive_type == primitive => Some(BigInt::from_i64(value)),
        Scalar::IntegerLiteral { literal } => literal.value_bignum(),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn declared_scalar_exclusions_preserve_predicate_boolean_meaning() {
        for (predicate, zero_excluded, one_excluded) in [
            ("value != 0", true, false),
            ("value < 0 || value > 0", true, false),
            ("value != 0 || value == 0", false, false),
            ("value != 0 && value != 1", true, true),
        ] {
            let program = crate::front_end::typed_program(&format!(
                "domain i32::Allowed(value) requires {predicate};
                 machine inspect(value: &i32 in Allowed) {{}}"
            ));
            let machine = &program.machines()[0];
            let state = &program.machine_states(machine)[0];
            let reference = program.state_parameters(state)[0].type_reference;
            assert_eq!(
                type_excludes_integer(&program, reference, 0),
                zero_excluded,
                "{predicate}"
            );
            assert_eq!(
                type_excludes_integer(&program, reference, 1),
                one_excluded,
                "{predicate}"
            );
        }
    }

    #[test]
    fn unrelated_types_and_unknown_predicates_supply_no_exclusion() {
        let program = crate::front_end::typed_program(
            "machine inspect(value: i32 in Wrapping, other: bool) {}",
        );
        let machine = &program.machines()[0];
        let state = &program.machine_states(machine)[0];
        for parameter in program.state_parameters(state) {
            assert!(!type_excludes_integer(
                &program,
                parameter.type_reference,
                0
            ));
        }
        let unknown = Boolean::Parameter { position: 1 };
        assert_eq!(predicate_value(&unknown, PrimitiveType::I32, 0), None);
        assert_eq!(
            predicate_value(
                &Boolean::Not(Box::new(unknown.clone())),
                PrimitiveType::I32,
                0
            ),
            None
        );
        assert_eq!(
            predicate_value(
                &Boolean::Or {
                    left: Box::new(Boolean::Constant(false)),
                    right: Box::new(unknown),
                },
                PrimitiveType::I32,
                0
            ),
            None
        );
    }
}
