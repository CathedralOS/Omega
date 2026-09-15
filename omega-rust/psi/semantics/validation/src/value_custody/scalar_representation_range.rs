//! Wire decoding and compact layout share declaration range semantics here.
//! The typed representation owns authored endpoints and the shared exact numeric
//! query. Consumers cannot replace that query with a syntax-only integer fold.

use numerics::bignum::BigInt;
use typed_trees::TypedTrees;
use typed_trees::types::{
    PrimitiveType, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
};

/// Normalize a scalar type's declared representation invariants into one
/// inclusive interval. This includes authored integer range shells, `bool`'s
/// intrinsic `{0, 1}` representation, and the finite carrier bounds of
/// `i32`/`u32`.
/// Callers must validate declared ranges before treating `None` as the full
/// decoder value width (`i64`/`u64`); an unevaluated range is not unconstrained.
/// Wire decoding and compact bit-layout validation deliberately share this
/// declaration-owned fact. A proven empty intersection is represented by
/// `1..=0`, never `None`: decoding and initialization must reject every value,
/// including when an exclusive endpoint's predecessor lies below `i64::MIN`.
pub fn scalar_representation_range(
    program: &TypedTrees,
    handle: TypeReferenceHandle,
) -> Option<language_semantics::wire::WireScalarRange> {
    fn collect(
        program: &TypedTrees,
        handle: TypeReferenceHandle,
        minimum: &mut BigInt,
        maximum: &mut BigInt,
        found: &mut bool,
    ) -> Option<()> {
        match program.type_reference_table.type_reference(handle) {
            TypeReferenceNode::Reference { referee, .. } => {
                collect(program, *referee, minimum, maximum, found)
            }
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => {
                for constraint in program.type_reference_table.constraints(*constraints) {
                    if let TypeConstraintNode::Range {
                        minimum: lower,
                        maximum: upper,
                        end_inclusive,
                    } = constraint
                    {
                        let lower = crate::closed_integer_range_bound(program, *lower)?;
                        let upper =
                            crate::closed_integer_range_maximum(program, *upper, *end_inclusive)?;
                        if lower > *minimum {
                            *minimum = lower;
                        }
                        if upper < *maximum {
                            *maximum = upper;
                        }
                        *found = true;
                    }
                }
                collect(program, *base_type, minimum, maximum, found)
            }
            _ => Some(()),
        }
    }

    let primitive = program.primitive_type_reference(handle)?;
    if primitive == PrimitiveType::Bool {
        return Some(language_semantics::wire::WireScalarRange {
            minimum: 0,
            maximum: 1,
            signed: false,
        });
    }
    let (carrier_minimum, carrier_maximum) = match primitive {
        PrimitiveType::I8 => (i64::from(i8::MIN), i128::from(i8::MAX)),
        PrimitiveType::I16 => (i64::from(i16::MIN), i128::from(i16::MAX)),
        PrimitiveType::I32 => (i64::from(i32::MIN), i128::from(i32::MAX)),
        PrimitiveType::I64 => (i64::MIN, i128::from(i64::MAX)),
        PrimitiveType::U8 => (0, i128::from(u8::MAX)),
        PrimitiveType::U16 => (0, i128::from(u16::MAX)),
        PrimitiveType::U32 => (0, i128::from(u32::MAX)),
        PrimitiveType::U64 => (0, i128::from(u64::MAX)),
        _ => return None,
    };
    let mut minimum = BigInt::from_i64(carrier_minimum);
    let mut maximum = BigInt::from_u128(u128::try_from(carrier_maximum).ok()?);
    let mut found = matches!(primitive, PrimitiveType::I32 | PrimitiveType::U32);
    collect(program, handle, &mut minimum, &mut maximum, &mut found)?;
    if !found {
        return None;
    }
    let (minimum, maximum) = if minimum > maximum {
        // Nonnegative bottom bounds also stay empty under the unsigned wire
        // decoder's comparison algebra; casting a negative bound would not.
        (1, 0)
    } else {
        (minimum.to_i64()?, maximum.to_i64()?)
    };
    Some(language_semantics::wire::WireScalarRange {
        minimum,
        maximum,
        signed: primitive.is_signed_integer(),
    })
}
