//! Exact fixed-array extent for mutable byte-view borrowing at admitted calls.

use crate::static_path::runtime_structural_path_tip;
use semantic_vocabulary::{IntegerSign, ScalarType};
use terminal_psi::{
    ByteSequenceCarrier, StructuralAccess, StructuralArgument, StructuralMultiplicity,
    StructuralParameterDeclaration, StructuralPathSegment, StructuralTypeDeclaration,
    StructuralTypeShape,
};

/// Rejoin a mutable byte-view argument to initialized fixed-u8-array storage.
/// This preserves the actual type and field path; the returned length grants
/// neither new storage nor resize permission. Callers separately check claims,
/// exclusive aliasing, availability, and the admitted ordinary/boundary call site.
pub fn mutable_fixed_byte_array_extent<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    actual: &StructuralParameterDeclaration,
    argument: &StructuralArgument,
    expected: &StructuralParameterDeclaration,
) -> Option<u64> {
    if actual.place != argument.place
        || argument.access != StructuralAccess::MutableBorrow
        || [actual, expected].iter().any(|parameter| {
            parameter.access != StructuralAccess::MutableBorrow
                || parameter.multiplicity != StructuralMultiplicity::Unrestricted
                || !parameter.qualifications.is_empty()
                || !parameter.projected_qualifications.is_empty()
        })
        || !types.clone().any(|declaration| {
            declaration.id == expected.structural_type
                && declaration.shape
                    == StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView)
        })
    {
        return None;
    }
    if !argument
        .path
        .iter()
        .all(|segment| matches!(segment, StructuralPathSegment::Field(_)))
    {
        return None;
    }
    let declaration =
        runtime_structural_path_tip(types.clone(), actual.structural_type, &argument.path)?;
    let StructuralTypeShape::FixedArray { element, length } = declaration.shape else {
        return None;
    };
    // This borrowed-backing presentation currently requires a positive extent.
    if length == 0 {
        return None;
    }
    let element = types.clone().find(|row| row.id == element)?;
    match element.shape {
        StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(integer))
            if integer.sign() == IntegerSign::Unsigned
                && integer.bits() == 8
                && !integer.is_address() =>
        {
            Some(length)
        }
        _ => None,
    }
}
