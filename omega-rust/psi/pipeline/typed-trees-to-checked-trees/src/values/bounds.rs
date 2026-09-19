//! Materialized runtime bounds over selected operations, never source syntax.

use checked_trees::{
    CheckedIntegerBinaryKind, CheckedScalarExpression, CheckedStructuralPredicatePathSegment,
};
use facts::IntegerRange;
use numerics::{arithmetic::ArithmeticDomain, bignum::BigInt, literals::LandedIntegerType};
use symbols::SymbolHandle;
use typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

mod sources;
#[cfg(test)]
mod tests;
pub(crate) use sources::PlaceIntegerBounds;

pub(crate) trait IntegerBoundsSource {
    fn binding(&mut self, position: usize, primitive_type: PrimitiveType) -> Option<IntegerRange>;
    fn storage(
        &mut self,
        symbol: SymbolHandle,
        primitive_type: PrimitiveType,
    ) -> Option<IntegerRange>;
    fn structural_field(
        &mut self,
        position: u32,
        path: &[CheckedStructuralPredicatePathSegment],
    ) -> Option<IntegerRange>;
    fn indexed_field(
        &mut self,
        position: u32,
        path: &[CheckedStructuralPredicatePathSegment],
        index: Option<&IntegerRange>,
    ) -> Option<IntegerRange>;
    /// The live byte length of a resolved structural byte carrier. Only a
    /// completed whole-carrier snapshot supplies it; a source with no such
    /// snapshot keeps the full `u64` carrier, which bounds every length.
    fn byte_length(
        &mut self,
        _position: u32,
        _path: &[CheckedStructuralPredicatePathSegment],
    ) -> Option<IntegerRange> {
        primitive_range(PrimitiveType::U64)
    }
}

pub(crate) fn evaluate(
    expression: &CheckedScalarExpression,
    source: &mut impl IntegerBoundsSource,
) -> Option<IntegerRange> {
    integer(expression, source).map(|(_, bounds)| bounds)
}

fn integer(
    expression: &CheckedScalarExpression,
    source: &mut impl IntegerBoundsSource,
) -> Option<(PrimitiveType, IntegerRange)> {
    use CheckedScalarExpression as Expression;
    let (primitive, bounds) = match expression {
        Expression::StructuralParameterByteLength {
            parameter_position,
            path,
        } => (
            PrimitiveType::U64,
            source.byte_length(*parameter_position, path)?,
        ),
        Expression::Parameter {
            position,
            primitive_type,
        }
        | Expression::Local {
            position,
            primitive_type,
        } => (*primitive_type, source.binding(*position, *primitive_type)?),
        Expression::ErasedParameter { .. } => return None,
        Expression::StorageRead {
            symbol,
            primitive_type,
        } => (*primitive_type, source.storage(*symbol, *primitive_type)?),
        Expression::StructuralParameterField {
            parameter_position,
            path,
            primitive_type,
        } => (
            *primitive_type,
            source.structural_field(*parameter_position, path)?,
        ),
        Expression::StructuralParameterIndexedRead {
            parameter_position,
            path,
            index,
            primitive_type,
        } => {
            let index = evaluate(index, source);
            (
                *primitive_type,
                source.indexed_field(*parameter_position, path, index.as_ref())?,
            )
        }
        Expression::IntegerLiteral { literal } => {
            let primitive = match literal.landing()?.landed_type {
                LandedIntegerType::I8 => PrimitiveType::I8,
                LandedIntegerType::I16 => PrimitiveType::I16,
                LandedIntegerType::I32 => PrimitiveType::I32,
                LandedIntegerType::I64 => PrimitiveType::I64,
                LandedIntegerType::U8 => PrimitiveType::U8,
                LandedIntegerType::U16 => PrimitiveType::U16,
                LandedIntegerType::U32 => PrimitiveType::U32,
                LandedIntegerType::U64 => PrimitiveType::U64,
                LandedIntegerType::Addr => return None,
            };
            let value = literal.value_bignum()?;
            (
                primitive,
                IntegerRange {
                    minimum: value.clone(),
                    maximum: value,
                },
            )
        }
        Expression::IntegerBinary {
            kind,
            primitive_type,
            left,
            right,
        } => {
            let (left_type, left) = integer(left, source)?;
            let (right_type, right) = integer(right, source)?;
            if left_type != *primitive_type || right_type != *primitive_type {
                return None;
            }
            (
                *primitive_type,
                binary(*kind, *primitive_type, left, right)?,
            )
        }
        Expression::IntegerWiden {
            primitive_type,
            operand,
        } => {
            let (source_type, bounds) = integer(operand, source)?;
            if !contains(
                &primitive_range(*primitive_type)?,
                &primitive_range(source_type)?,
            ) {
                return None;
            }
            (*primitive_type, bounds)
        }
        Expression::IntegerExactCast {
            primitive_type,
            operand,
            range,
        } => {
            let (_, bounds) = integer(operand, source)?;
            // The retained occurrence fact proved the operand lands inside
            // `range`; flow bounds are only a coarser view of the same value.
            // The result is the meet of the two, not a failure when the flow
            // view is wider than the proved spelling range.
            (
                *primitive_type,
                IntegerRange {
                    minimum: bounds.minimum.max(range.minimum.clone()),
                    maximum: bounds.maximum.min(range.maximum.clone()),
                },
            )
        }
        Expression::IntegerWrappingCast {
            primitive_type,
            operand,
        } => {
            let (_, bounds) = integer(operand, source)?;
            let carrier = primitive_range(*primitive_type)?;
            // Modular conversion is not monotone across a wrapping boundary.
            (
                *primitive_type,
                if contains(&carrier, &bounds) {
                    bounds
                } else {
                    carrier
                },
            )
        }
        Expression::IntegerTrappingCast {
            primitive_type,
            operand,
        } => {
            // This only describes successful results; it does not prove that
            // the conversion or its enclosing invocation returns normally. A
            // result that did return is representable, so the carrier bounds
            // the operand range even when the operand's own interval is wider.
            let (_, bounds) = integer(operand, source)?;
            let carrier = primitive_range(*primitive_type)?;
            (
                *primitive_type,
                IntegerRange {
                    minimum: bounds.minimum.max(carrier.minimum.clone()),
                    maximum: bounds.maximum.min(carrier.maximum.clone()),
                },
            )
        }
        Expression::IntegerBitwiseNot { .. }
        | Expression::Boolean(_)
        | Expression::IeeeFloatLiteral { .. } => return None,
    };
    contains(&primitive_range(primitive)?, &bounds).then_some((primitive, bounds))
}

fn binary(
    kind: CheckedIntegerBinaryKind,
    primitive: PrimitiveType,
    left: IntegerRange,
    right: IntegerRange,
) -> Option<IntegerRange> {
    use CheckedIntegerBinaryKind as Kind;
    if matches!(
        kind,
        Kind::ExactRemainder | Kind::WrappingRemainder | Kind::SaturatingRemainder
    ) {
        // For nonnegative dividends and positive divisors, the remainder is
        // below the divisor and no greater than the dividend for every policy.
        if left.minimum < BigInt::zero() || right.minimum <= BigInt::zero() {
            return None;
        }
        return Some(IntegerRange {
            minimum: BigInt::zero(),
            maximum: left.maximum.min(right.maximum.sub(&BigInt::from_u64(1))),
        });
    }
    let mut bounds = match kind {
        Kind::ExactAdd | Kind::WrappingAdd | Kind::SaturatingAdd => IntegerRange {
            minimum: left.minimum.add(&right.minimum),
            maximum: left.maximum.add(&right.maximum),
        },
        Kind::ExactSubtract | Kind::WrappingSubtract | Kind::SaturatingSubtract => IntegerRange {
            minimum: left.minimum.add(&right.maximum.negate()),
            maximum: left.maximum.add(&right.minimum.negate()),
        },
        Kind::ExactMultiply | Kind::WrappingMultiply | Kind::SaturatingMultiply => {
            let corners = [
                left.minimum.mul(&right.minimum),
                left.minimum.mul(&right.maximum),
                left.maximum.mul(&right.minimum),
                left.maximum.mul(&right.maximum),
            ];
            IntegerRange {
                minimum: corners.iter().min()?.clone(),
                maximum: corners.iter().max()?.clone(),
            }
        }
        _ => return None,
    };
    let carrier = primitive_range(primitive)?;
    match kind {
        Kind::ExactAdd | Kind::ExactSubtract | Kind::ExactMultiply => {
            contains(&carrier, &bounds).then_some(bounds)
        }
        Kind::WrappingAdd | Kind::WrappingSubtract | Kind::WrappingMultiply => {
            // Crossing a wrap boundary is not a monotone endpoint mapping.
            // Conservatively retain the full carrier instead of a false range.
            Some(if contains(&carrier, &bounds) {
                bounds
            } else {
                carrier
            })
        }
        Kind::SaturatingAdd | Kind::SaturatingSubtract | Kind::SaturatingMultiply => {
            bounds.minimum = bounds
                .minimum
                .max(carrier.minimum.clone())
                .min(carrier.maximum.clone());
            bounds.maximum = bounds.maximum.max(carrier.minimum).min(carrier.maximum);
            Some(bounds)
        }
        _ => None,
    }
}

/// Read the declared `Range` constraints on an exact integer reference. Those
/// constraints are storage invariants: construction and every write must
/// satisfy them, so a read can use them even when no value snapshot is live.
/// A non-Exact arithmetic domain does not range-check its stores, so such a
/// reference supplies no declared bound and the caller keeps the raw carrier.
/// `None` also means "no tightening available"; it is never an error.
pub(crate) fn declared_bounds(
    program: &typed_trees::TypedTrees,
    mut reference: TypeReferenceHandle,
    primitive_type: PrimitiveType,
) -> Option<IntegerRange> {
    let mut declared: Option<IntegerRange> = None;
    loop {
        match program.type_reference_table.type_reference(reference) {
            // An exclusive borrow can be reborrowed through a pointee type
            // that drops declared constraints; a shared borrow keeps its
            // referent frozen and preserves them.
            TypeReferenceNode::Reference {
                referee, access, ..
            } => {
                if access.is_exclusive() {
                    return None;
                }
                reference = *referee;
            }
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => {
                for constraint in program.type_reference_table.constraints(*constraints) {
                    match constraint {
                        typed_trees::types::TypeConstraintNode::Range {
                            minimum,
                            maximum,
                            end_inclusive,
                        } => {
                            // The same closed-endpoint evaluation as source
                            // range validation; failure is no bound, never a
                            // guessed one.
                            let low = validation::closed_integer_range_bound(program, *minimum)?;
                            let high = validation::closed_integer_range_maximum(
                                program,
                                *maximum,
                                *end_inclusive,
                            )?;
                            declared = Some(match declared {
                                Some(previous) => IntegerRange {
                                    minimum: previous.minimum.max(low),
                                    maximum: previous.maximum.min(high),
                                },
                                None => IntegerRange {
                                    minimum: low,
                                    maximum: high,
                                },
                            });
                        }
                        typed_trees::types::TypeConstraintNode::ArithmeticDomain(domain)
                            if *domain != ArithmeticDomain::Exact =>
                        {
                            return None;
                        }
                        _ => {}
                    }
                }
                reference = *base_type;
            }
            _ => break,
        }
    }
    let declared = declared?;
    let carrier = primitive_range(primitive_type)?;
    let minimum = declared.minimum.max(carrier.minimum.clone());
    let maximum = declared.maximum.min(carrier.maximum);
    (minimum <= maximum).then_some(IntegerRange { minimum, maximum })
}

pub(crate) fn contains(outer: &IntegerRange, inner: &IntegerRange) -> bool {
    outer.minimum <= inner.minimum
        && inner.minimum <= inner.maximum
        && inner.maximum <= outer.maximum
}

pub(crate) fn primitive_range(primitive: PrimitiveType) -> Option<IntegerRange> {
    let (signed, bits) = match primitive {
        PrimitiveType::I8 => (true, 8),
        PrimitiveType::I16 => (true, 16),
        PrimitiveType::I32 => (true, 32),
        PrimitiveType::I64 => (true, 64),
        PrimitiveType::U8 => (false, 8),
        PrimitiveType::U16 => (false, 16),
        PrimitiveType::U32 => (false, 32),
        PrimitiveType::U64 => (false, 64),
        _ => return None,
    };
    let (minimum, maximum) = if signed {
        (-(1i128 << (bits - 1)), (1i128 << (bits - 1)) - 1)
    } else {
        (0, (1i128 << bits) - 1)
    };
    Some(IntegerRange {
        minimum: BigInt::from_i128(minimum),
        maximum: BigInt::from_i128(maximum),
    })
}
