//! Exact fixed-array windows for shared and mutable byte-view call loans.

use crate::runtime_structural_path_tip;
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

/// How the same borrowed referent is presented in the destination carrier.
/// This is geometry, not authority to read, lend, or retain the source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BorrowedReferentPresentation {
    /// The existing carrier already has the destination representation.
    Unchanged,
    /// The destination stores a pointer to these inline backing bytes.
    AddressOfBacking { byte_count: u64 },
}

/// Compare borrowed referents without equating their storage representations.
/// A bounded byte view may observe exactly the corresponding fixed u8 backing.
/// This judgment supplies geometry only: every caller must independently check
/// access, qualifications, lifetime, and aliasing before presenting the loan.
pub fn borrowed_referent_matches<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    actual: semantic_vocabulary::StructuralTypeId,
    expected: semantic_vocabulary::StructuralTypeId,
) -> bool {
    borrowed_referent_presentation(types, actual, expected).is_some()
}

/// Reconstruct the representation step separately from loan admission. Copying
/// the first word of an inline array does not construct a view; conversely,
/// taking the address of an existing pointer cell adds an unwanted indirection.
pub fn borrowed_referent_presentation<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    actual: semantic_vocabulary::StructuralTypeId,
    expected: semantic_vocabulary::StructuralTypeId,
) -> Option<BorrowedReferentPresentation> {
    let Some(expected_type) = types.clone().find(|row| row.id == expected) else {
        return None;
    };
    if actual == expected {
        return Some(BorrowedReferentPresentation::Unchanged);
    }
    // Stored element descriptors retain a declared access ceiling; borrowed
    // formals and result continuations can present either stored or canonical
    // identity for that exact element type.
    // This is the same physical pointer/count pair, not another reference
    // indirection. Access admission must still check the selected source.
    if let StructuralTypeShape::ElementView {
        element: expected_element,
        ..
    } = expected_type.shape
    {
        return matches!(types.clone().find(|row| row.id == actual)?.shape,
            StructuralTypeShape::ElementView { element, .. } if element == expected_element)
        .then_some(BorrowedReferentPresentation::Unchanged);
    }
    let StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedView { capacity, .. }) =
        expected_type.shape
    else {
        return None;
    };
    if capacity == 0 || fixed_byte_extent(types.clone(), actual) != Some(capacity) {
        return None;
    }
    match types.clone().find(|row| row.id == actual)?.shape {
        StructuralTypeShape::FixedArray { .. } => {
            Some(BorrowedReferentPresentation::AddressOfBacking {
                byte_count: capacity,
            })
        }
        StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedView { .. }) => {
            Some(BorrowedReferentPresentation::Unchanged)
        }
        _ => None,
    }
}

fn fixed_byte_extent<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    identity: semantic_vocabulary::StructuralTypeId,
) -> Option<u64> {
    match types.clone().find(|row| row.id == identity)?.shape {
        StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedView {
            capacity, ..
        }) => Some(capacity),
        StructuralTypeShape::FixedArray { element, length } => {
            let element = types.clone().find(|row| row.id == element)?;
            matches!(element.shape,
                StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(integer))
                    if integer.sign() == IntegerSign::Unsigned
                        && integer.bits() == 8 && !integer.is_address())
            .then_some(length)
        }
        _ => None,
    }
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
    if !readable_collection_loan(actual, argument, expected)
        || !types.clone().any(|declaration| {
            declaration.id == expected.structural_type
                && matches!(
                    declaration.shape,
                    StructuralTypeShape::ByteSequence(
                        terminal_psi::byte_sequence_view_carrier_pattern!()
                    )
                )
        })
    {
        return None;
    }
    let bounded_capacity = types.clone().find_map(|declaration| {
        match (
            declaration.id == expected.structural_type,
            &declaration.shape,
        ) {
            (
                true,
                StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedView {
                    capacity,
                    ..
                }),
            ) => Some(*capacity),
            _ => None,
        }
    });
    // A `&mut [u8; N]` borrows the whole fixed backing — a ranged
    // presentation or a different declared extent can never satisfy its
    // bound.
    if bounded_capacity.is_some()
        && matches!(
            argument.path.last(),
            Some(StructuralPathSegment::FixedByteRange { .. })
        )
    {
        return None;
    }
    let window = fixed_byte_storage_window(types, actual.structural_type, &argument.path)?;
    if bounded_capacity.is_some_and(|capacity| capacity != window.length) {
        return None;
    }
    Some(window)
}

/// Resolve a region of fixed byte backing without inventing a standalone type
/// for the region. This is the geometry shared by call presentation and custody
/// reconstruction, not permission to form a reference, copy, or access bytes.
pub fn fixed_byte_storage_window<'types, 'path>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    root: semantic_vocabulary::StructuralTypeId,
    path: &'path [StructuralPathSegment],
) -> Option<FixedByteArrayWindow<'path>> {
    let (backing_path, range) = match path.split_last() {
        Some((StructuralPathSegment::FixedByteRange { start, end }, backing)) => {
            (backing, Some((*start, *end)))
        }
        _ => (path, None),
    };
    if !backing_path
        .iter()
        .all(|segment| matches!(segment, StructuralPathSegment::Field(_)))
    {
        return None;
    }
    let declaration = runtime_structural_path_tip(types.clone(), root, backing_path)?;
    // Inline arrays and already borrowed backing expose the same extent.
    // Window admission is independent of how native code stores the carrier;
    // transport must preserve that separate residence distinction.
    let length = fixed_byte_extent(types, declaration.id)?;
    // This borrowed-backing presentation currently requires a positive extent.
    if length == 0 {
        return None;
    }
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

/// Rejoin a borrowed element-view argument to initialized fixed-array storage
/// of the same element type — the array-to-slice presentation of the whole
/// backing. Same custody rules as the byte window: the backing keeps its type
/// and path, and the returned length grants neither storage nor resize.
pub fn fixed_element_array_extent<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    actual: &StructuralParameterDeclaration,
    argument: &StructuralArgument,
    expected: &StructuralParameterDeclaration,
) -> Option<u64> {
    if !readable_collection_loan(actual, argument, expected)
        || !argument
            .path
            .iter()
            .all(|segment| matches!(segment, StructuralPathSegment::Field(_)))
    {
        return None;
    }
    let expected_element = types.clone().find_map(|declaration| {
        match (
            declaration.id == expected.structural_type,
            &declaration.shape,
        ) {
            (true, StructuralTypeShape::ElementView { element, .. }) => Some(*element),
            _ => None,
        }
    })?;
    let declaration =
        runtime_structural_path_tip(types.clone(), actual.structural_type, &argument.path)?;
    let StructuralTypeShape::FixedArray { element, length } = declaration.shape else {
        return None;
    };
    // An empty backing still forms a typed view with extent zero. Its
    // element declaration and loan custody remain mandatory; the extent
    // grants no indexed access and does not fabricate an element value.
    if element != expected_element {
        return None;
    }
    Some(length)
}

/// The borrowed presentation does not copy or transfer its backing owner.
/// Keep the owner's actual multiplicity distinct from the unrestricted loan
/// parameter. Callers separately authenticate that source home, its live
/// custody, qualifications, claims and exclusive overlap.
fn readable_collection_loan(
    actual: &StructuralParameterDeclaration,
    argument: &StructuralArgument,
    expected: &StructuralParameterDeclaration,
) -> bool {
    actual.place == argument.place
        && argument.access == expected.access
        && matches!(
            (actual.access, argument.access),
            (
                terminal_psi::mutating_access_pair_pattern!(),
                terminal_psi::reading_borrow_pair_pattern!()
            ) | (
                StructuralAccess::SharedBorrow,
                StructuralAccess::SharedBorrow
            )
        )
        && matches!(
            actual.multiplicity,
            terminal_psi::nonlinear_multiplicity_pair_pattern!()
        )
        && expected.multiplicity == StructuralMultiplicity::Unrestricted
        && [actual, expected].iter().all(|parameter| {
            parameter.qualifications.is_empty() && parameter.projected_qualifications.is_empty()
        })
}
