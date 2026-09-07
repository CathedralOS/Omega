//! Materialized runtime bounds over selected operations, never source syntax.

use checked_trees::{
    CheckedIntegerBinaryKind, CheckedScalarExpression, CheckedStructuralPredicatePathSegment,
};
use facts::IntegerRange;
use numerics::{bignum::BigInt, literals::LandedIntegerType};
use symbols::SymbolHandle;
use typed_trees::types::PrimitiveType;

mod sources;
#[cfg(test)]
mod tests;
pub(crate) use sources::PlaceIntegerBounds;

pub(crate) trait IntegerBoundsSource {
    fn binding(&mut self, position: usize) -> Option<IntegerRange>;
    fn storage(&mut self, symbol: SymbolHandle) -> Option<IntegerRange>;
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
        Expression::Parameter {
            position,
            primitive_type,
        }
        | Expression::Local {
            position,
            primitive_type,
        } => (*primitive_type, source.binding(*position)?),
        Expression::StorageRead {
            symbol,
            primitive_type,
        } => (*primitive_type, source.storage(*symbol)?),
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
            if !contains(range, &bounds) {
                return None;
            }
            (*primitive_type, bounds)
        }
        Expression::IntegerTrappingCast {
            primitive_type,
            operand,
        } => {
            // This only describes successful results; it does not prove that
            // the conversion or its enclosing invocation returns normally.
            let (_, bounds) = integer(operand, source)?;
            (*primitive_type, bounds)
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

pub(crate) fn contains(outer: &IntegerRange, inner: &IntegerRange) -> bool {
    outer.minimum <= inner.minimum
        && inner.minimum <= inner.maximum
        && inner.maximum <= outer.maximum
}

fn primitive_range(primitive: PrimitiveType) -> Option<IntegerRange> {
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
