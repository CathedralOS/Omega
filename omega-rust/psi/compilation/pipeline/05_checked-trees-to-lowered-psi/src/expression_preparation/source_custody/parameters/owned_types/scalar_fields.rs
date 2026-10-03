//! Reconstruct scalar restrictions without consulting producer field catalogs.
//!
//! Type substitution can introduce another range or predicate-domain shell.
//! Intersect every restriction and the fixed carrier, retaining a bounded field even if its range
//! spans that carrier. Unsupported bounds or policies must not become raw scalars.
//! Endpoint intersection precedes conversion to the signed or unsigned carrier.
//! General predicates are independently projected from their exact declarations;
//! no retained producer predicate is authority for the receiving field catalog.

use checked_trees::types::{
    PrimitiveType, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
};
use checked_trees::{CheckedTrees, CheckedUnitStructuralFieldType};
use numerics::arithmetic::ArithmeticDomain;
use numerics::bignum::BigInt;
use semantic_vocabulary::{BoundedIntegerType, IntegerValue, ScalarType};
use symbols::SymbolHandle;

pub(super) fn reconstruct(
    checked: &CheckedTrees,
    mut reference: TypeReferenceHandle,
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    primitive: PrimitiveType,
) -> Option<CheckedUnitStructuralFieldType> {
    let mut bounds: Option<(BigInt, BigInt)> = None;
    let mut non_exact = false;
    let mut predicate = None;
    let mut visited = Vec::new();
    loop {
        if !checked
            .type_reference_table
            .contains_type_reference(reference)
            || visited.contains(&reference)
        {
            return None;
        }
        visited.push(reference);
        match checked.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => {
                let rows = checked.type_reference_table.constraints(*constraints);
                if rows.is_empty() || rows.len() != constraints.count() as usize {
                    return None;
                }
                for constraint in rows {
                    let interval = match constraint {
                        TypeConstraintNode::ArithmeticDomain(domain) => {
                            non_exact |= *domain != ArithmeticDomain::Exact;
                            continue;
                        }
                        TypeConstraintNode::Range {
                            minimum,
                            maximum,
                            end_inclusive,
                        } => (
                            validation::closed_integer_range_bound(&checked.typed, *minimum)?,
                            validation::closed_integer_range_maximum(
                                &checked.typed,
                                *maximum,
                                *end_inclusive,
                            )?,
                        ),
                        TypeConstraintNode::Domain(domain) => {
                            if let Some(interval) =
                                validation::exact_declared_domain_carrier_interval(
                                    &checked.typed,
                                    primitive,
                                    domain,
                                )
                            {
                                interval
                            } else {
                                let next = validation::scalar_field_domain_predicate(
                                    &checked.typed,
                                    domain,
                                    primitive,
                                )?;
                                predicate = Some(match predicate {
                                    None => next,
                                    Some(previous) => {
                                        checked_trees::CheckedBooleanExpression::And {
                                            left: Box::new(previous),
                                            right: Box::new(next),
                                        }
                                    }
                                });
                                continue;
                            }
                        }
                        _ => return None,
                    };
                    bounds = Some(match bounds {
                        Some((minimum, maximum)) => {
                            (minimum.max(interval.0), maximum.min(interval.1))
                        }
                        None => interval,
                    });
                }
                reference = *base_type;
            }
            TypeReferenceNode::Named { symbol, .. } => {
                let Some((_, replacement)) = substitutions
                    .iter()
                    .rev()
                    .find(|(parameter, _)| parameter == symbol)
                else {
                    break;
                };
                reference = *replacement;
            }
            _ => return None,
        }
    }
    if checked.primitive_type_reference(reference) != Some(primitive) {
        return None;
    }
    if non_exact && (bounds.is_some() || predicate.is_some()) {
        return None;
    }
    let Some((minimum, maximum)) = bounds else {
        return validation::scalar_field_restriction(primitive, None, predicate);
    };
    let ScalarType::Integer(integer) =
        crate::emission::scalar_types::integer_scalar_type(primitive).ok()?
    else {
        return None;
    };
    let (minimum, maximum) = match (integer.minimum_value(), integer.maximum_value()) {
        (IntegerValue::Signed(carrier_minimum), IntegerValue::Signed(carrier_maximum)) => (
            IntegerValue::Signed(i128::from(
                minimum.max(BigInt::from_i128(carrier_minimum)).to_i64()?,
            )),
            IntegerValue::Signed(i128::from(
                maximum.min(BigInt::from_i128(carrier_maximum)).to_i64()?,
            )),
        ),
        (IntegerValue::Unsigned(carrier_minimum), IntegerValue::Unsigned(carrier_maximum)) => (
            IntegerValue::Unsigned(u128::from(
                minimum.max(BigInt::from_u128(carrier_minimum)).to_u64()?,
            )),
            IntegerValue::Unsigned(u128::from(
                maximum.min(BigInt::from_u128(carrier_maximum)).to_u64()?,
            )),
        ),
        _ => return None,
    };
    validation::scalar_field_restriction(
        primitive,
        Some(BoundedIntegerType::new(integer, minimum, maximum).ok()?),
        predicate,
    )
}
