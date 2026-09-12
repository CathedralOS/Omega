//! Wire decoding and compact layout share declaration range semantics here.
//! The representation owns the authored endpoints; validation owns their
//! selected arithmetic, so consumers cannot use a syntax-only integer fold.

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
/// declaration-owned fact.
pub fn scalar_representation_range(
    program: &TypedTrees,
    handle: TypeReferenceHandle,
) -> Option<language_semantics::wire::WireScalarRange> {
    fn collect(
        program: &TypedTrees,
        handle: TypeReferenceHandle,
        minimum: &mut i64,
        maximum: &mut i64,
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
                    } = constraint
                    {
                        *minimum = (*minimum)
                            .max(crate::closed_integer_range_bound(program, *lower)?.to_i64()?);
                        *maximum = (*maximum)
                            .min(crate::closed_integer_range_bound(program, *upper)?.to_i64()?);
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
    if !primitive.accepts_range_constraint() {
        return None;
    }
    let (mut minimum, mut maximum, mut found) = match primitive {
        PrimitiveType::I32 => (i64::from(i32::MIN), i64::from(i32::MAX), true),
        PrimitiveType::U32 => (0, i64::from(u32::MAX), true),
        _ => (i64::MIN, i64::MAX, false),
    };
    collect(program, handle, &mut minimum, &mut maximum, &mut found)?;
    (found && minimum <= maximum).then_some(language_semantics::wire::WireScalarRange {
        minimum,
        maximum,
        signed: primitive.is_signed_integer(),
    })
}
