//! A recast's source-selected interpretation, shared by reference formation and
//! returned-origin publication. This classifies an already checked view; it
//! grants no representation proof, access, lifetime, or native address.

use checked_trees::CheckedReferenceStorage;
use semantic_vocabulary::{IeeeFloatFormat, IntegerSign, IntegerType, ScalarType};
use typed_trees::types::{PrimitiveType, TypeReferenceHandle};

pub fn reference_view_storage(
    program: &dyn crate::CheckingInput,
    source: TypeReferenceHandle,
    referent: TypeReferenceHandle,
) -> Option<CheckedReferenceStorage> {
    // A qualification-only change lends the existing declaration. Routing it
    // through a byte interpretation would discard declaration-owned protocols
    // such as atomic fields even though no representation changed. Use the
    // canonical storage identity, which retains scalar receiving restrictions.
    if crate::stored_structural_type_identity(program, source, &[], &[])
        == crate::stored_structural_type_identity(program, referent, &[], &[])
    {
        return Some(CheckedReferenceStorage::Declared);
    }
    // Qualification affects evidence and access, not the physical schema of
    // this already-checked view. Keep it on the reference's type; geometry
    // reads the underlying carrier without pretending its bytes prove a route.
    let referent = program
        .type_reference_table
        .unconstrained_type_reference(referent);
    let stored_scalar = if let Some(primitive) = program.primitive_type_reference(referent) {
        let geometry = super::recast_scalar_path_projection(program, referent, &[])?;
        Some(match geometry.stored_integer {
            Some(integer) => ScalarType::Integer(integer),
            None => primitive_storage_scalar(primitive)?,
        })
    } else {
        // Aggregate geometry stays declaration-owned until target closure.
        // Classification cannot substitute a selected target's byte offsets.
        for target in crate::layout_checking_targets(program, [referent]).ok()? {
            let context = crate::LayoutCheckingContext::new(program, target);
            super::recast_scalar_layout(&context, referent)?;
        }
        None
    };
    Some(CheckedReferenceStorage::View { stored_scalar })
}

fn primitive_storage_scalar(primitive: PrimitiveType) -> Option<ScalarType> {
    match primitive {
        PrimitiveType::Bool => Some(ScalarType::Boolean),
        PrimitiveType::F32 => Some(ScalarType::IeeeFloat(IeeeFloatFormat::Binary32)),
        PrimitiveType::F64 => Some(ScalarType::IeeeFloat(IeeeFloatFormat::Binary64)),
        primitive if primitive.is_fixed_integer() => {
            let sign = if primitive.is_signed_integer() {
                IntegerSign::Signed
            } else {
                IntegerSign::Unsigned
            };
            let bits = u16::try_from(primitive.scalar_byte_size()?.checked_mul(8)?).ok()?;
            IntegerType::new(sign, bits).ok().map(ScalarType::Integer)
        }
        _ => None,
    }
}
