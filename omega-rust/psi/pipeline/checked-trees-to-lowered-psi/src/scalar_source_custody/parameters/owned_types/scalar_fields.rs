//! Reconstruct scalar restrictions without consulting producer field catalogs.
//!
//! Type substitution can introduce another range shell. Intersect all declared
//! intervals and the fixed carrier, retaining a bounded field even if its range
//! spans that carrier. Unsupported bounds or policies must not become raw scalars.
//! Endpoint intersection precedes conversion to the signed or unsigned carrier.

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
                    match constraint {
                        TypeConstraintNode::ArithmeticDomain(domain) => {
                            non_exact |= *domain != ArithmeticDomain::Exact;
                        }
                        TypeConstraintNode::Range {
                            minimum,
                            maximum,
                            end_inclusive,
                        } => {
                            let minimum =
                                validation::closed_integer_range_bound(&checked.typed, *minimum)?;
                            let maximum = validation::closed_integer_range_maximum(
                                &checked.typed,
                                *maximum,
                                *end_inclusive,
                            )?;
                            bounds = Some(match bounds {
                                Some((prior_minimum, prior_maximum)) => {
                                    (minimum.max(prior_minimum), maximum.min(prior_maximum))
                                }
                                None => (minimum, maximum),
                            });
                        }
                        _ => return None,
                    }
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
    let Some((minimum, maximum)) = bounds else {
        return Some(CheckedUnitStructuralFieldType::Scalar(primitive));
    };
    if non_exact {
        return None;
    }
    let ScalarType::Integer(integer) =
        crate::scalar_graph_lowering::integer_scalar_type(primitive).ok()?
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
    Some(CheckedUnitStructuralFieldType::BoundedInteger(
        BoundedIntegerType::new(integer, minimum, maximum).ok()?,
    ))
}
