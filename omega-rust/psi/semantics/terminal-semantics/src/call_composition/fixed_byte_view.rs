//! Exact fixed-array windows for shared and mutable byte-view call loans.

use crate::static_path::runtime_structural_path_tip;
use semantic_vocabulary::{IntegerSign, ScalarType};
use terminal_psi::{
    ByteSequenceCarrier, StructuralAccess, StructuralArgument, StructuralMultiplicity,
    StructuralParameterDeclaration, StructuralPathSegment, StructuralTypeDeclaration,
    StructuralTypeShape,
};

/// A call-scoped window of the original fixed-array backing, not a new owner.
/// The backing path excludes the final range presentation. Empty windows keep
/// the same backing custody and merely expose zero writable bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FixedByteArrayWindow<'path> {
    pub backing_path: &'path [StructuralPathSegment],
    pub backing_length: u64,
    pub offset: u64,
    pub length: u64,
}

/// Rejoin a borrowed byte-view argument to initialized fixed-u8-array storage.
/// This preserves the actual type and field path; the returned length grants
/// neither new storage nor resize permission. Callers separately check claims,
/// exclusive aliasing, availability, and the admitted ordinary/boundary call site.
pub fn fixed_byte_array_extent<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    actual: &StructuralParameterDeclaration,
    argument: &StructuralArgument,
    expected: &StructuralParameterDeclaration,
) -> Option<u64> {
    fixed_byte_array_window(types, actual, argument, expected).map(|window| window.length)
}

/// Resolve the exact backing and constant window for a borrowed call argument.
/// Availability, exclusive overlap and restoration remain call obligations.
pub fn fixed_byte_array_window<'types, 'path>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    actual: &StructuralParameterDeclaration,
    argument: &'path StructuralArgument,
    expected: &StructuralParameterDeclaration,
) -> Option<FixedByteArrayWindow<'path>> {
    if actual.place != argument.place
        || argument.access != expected.access
        || !matches!(
            (actual.access, argument.access),
            (
                StructuralAccess::MutableBorrow,
                StructuralAccess::MutableBorrow | StructuralAccess::SharedBorrow
            ) | (
                StructuralAccess::SharedBorrow,
                StructuralAccess::SharedBorrow
            )
        )
        || [actual, expected].iter().any(|parameter| {
            parameter.multiplicity != StructuralMultiplicity::Unrestricted
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
    let (backing_path, range) = match argument.path.split_last() {
        Some((StructuralPathSegment::FixedByteRange { start, end }, backing)) => {
            (backing, Some((*start, *end)))
        }
        _ => (argument.path.as_slice(), None),
    };
    if !backing_path
        .iter()
        .all(|segment| matches!(segment, StructuralPathSegment::Field(_)))
    {
        return None;
    }
    let declaration =
        runtime_structural_path_tip(types.clone(), actual.structural_type, backing_path)?;
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
            let (start, end) = range.unwrap_or((0, length));
            if start > end || end > length {
                return None;
            }
            Some(FixedByteArrayWindow {
                backing_path,
                backing_length: length,
                offset: start,
                length: end - start,
            })
        }
        _ => None,
    }
}
