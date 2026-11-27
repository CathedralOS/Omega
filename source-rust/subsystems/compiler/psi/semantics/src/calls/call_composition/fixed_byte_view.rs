use crate::runtime_structural_path_tip;
use semantic_vocabulary::{IntegerSign, ScalarType};
use terminal_psi::{
    ByteSequenceCarrier, StructuralAccess, StructuralArgument, StructuralMultiplicity,
    StructuralParameterDeclaration, StructuralPathSegment, StructuralTypeDeclaration,
    StructuralTypeShape,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FixedCollectionWindow<'path> {
    pub backing_path: &'path [StructuralPathSegment],
    pub backing_length: u64,
    pub offset: u64,
    pub length: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BorrowedReferentPresentation {
    Unchanged,
    AddressOfBacking { byte_count: u64 },
}

pub fn borrowed_referent_matches<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    actual: semantic_vocabulary::StructuralTypeId,
    expected: semantic_vocabulary::StructuralTypeId,
) -> bool {
    borrowed_referent_presentation(types, actual, expected).is_some()
}

pub fn borrowed_referent_presentation<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    actual: semantic_vocabulary::StructuralTypeId,
    expected: semantic_vocabulary::StructuralTypeId,
) -> Option<BorrowedReferentPresentation> {
    let expected_type = types.clone().find(|row| row.id == expected)?;
    if actual == expected {
        return Some(BorrowedReferentPresentation::Unchanged);
    }
    if crate::references::reference_views::descriptor_geometry_matches(
        &types.clone().find(|row| row.id == actual)?.shape,
        &expected_type.shape,
    ) {
        return Some(BorrowedReferentPresentation::Unchanged);
    }
    let StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedView {
        capacity,
        ..
    }) = expected_type.shape
    else {
        return None;
    };
    if fixed_byte_extent(types.clone(), actual) != Some(capacity) {
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

pub fn fixed_byte_extent<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    identity: semantic_vocabulary::StructuralTypeId,
) -> Option<u64> {
    match types.clone().find(|row| row.id == identity)?.shape {
        StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedView {
            capacity,
            ..
        }) => Some(capacity),
        StructuralTypeShape::FixedArray {
            element,
            length: terminal_psi::ArrayExtent::Literal(length),
        } => {
            let element = types.clone().find(|row| row.id == element)?;
            matches!(element.shape,
                StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(integer))
                    if integer.sign() == IntegerSign::Unsigned
                        && integer.bits() == Some(8) && !integer.is_address())
            .then_some(length)
        }
        _ => None,
    }
}

pub fn fixed_byte_array_window<'types, 'path>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    actual: &StructuralParameterDeclaration,
    argument: &'path StructuralArgument,
    expected: &StructuralParameterDeclaration,
) -> Option<FixedCollectionWindow<'path>> {
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
    if bounded_capacity.is_some()
        && matches!(
            argument.path.last(),
            Some(StructuralPathSegment::FixedRange { .. })
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

pub fn fixed_byte_storage_window<'types, 'path>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    root: semantic_vocabulary::StructuralTypeId,
    path: &'path [StructuralPathSegment],
) -> Option<FixedCollectionWindow<'path>> {
    let window = fixed_collection_storage_window(types.clone(), root, path)?;
    let backing = runtime_structural_path_tip(types.clone(), root, window.backing_path)?;
    // An empty range can borrow existing backing, but an erased empty
    // array has no backing to lend (byte_views.md, fixed-array loans).
    if matches!(
        backing.shape,
        StructuralTypeShape::FixedArray {
            length: terminal_psi::ArrayExtent::Literal(0),
            ..
        }
    ) {
        return None;
    }
    (fixed_byte_extent(types, backing.id)? == window.backing_length).then_some(window)
}

pub fn fixed_collection_storage_window<'types, 'path>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    root: semantic_vocabulary::StructuralTypeId,
    path: &'path [StructuralPathSegment],
) -> Option<FixedCollectionWindow<'path>> {
    let (backing_path, range) = match path.split_last() {
        Some((StructuralPathSegment::FixedRange { start, end }, backing)) => {
            (backing, Some((*start, *end)))
        }
        _ => (path, None),
    };
    let declaration = runtime_structural_path_tip(types.clone(), root, backing_path)?;
    let length = match declaration.shape {
        StructuralTypeShape::FixedArray {
            length: terminal_psi::ArrayExtent::Literal(length),
            ..
        } => length,
        _ => fixed_byte_extent(types, declaration.id)?,
    };
    let (start, end) = range.unwrap_or((0, length));
    if start > end || end > length {
        return None;
    }
    Some(FixedCollectionWindow {
        backing_path,
        backing_length: length,
        offset: start,
        length: end - start,
    })
}

pub fn fixed_element_array_extent<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    actual: &StructuralParameterDeclaration,
    argument: &StructuralArgument,
    expected: &StructuralParameterDeclaration,
) -> Option<u64> {
    if !readable_collection_loan(actual, argument, expected)
        || [actual, expected].iter().any(|parameter| {
            !parameter.qualifications.is_empty()
                || !parameter.projected_qualifications.is_empty()
        })
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
    let StructuralTypeShape::FixedArray {
        element,
        length: terminal_psi::ArrayExtent::Literal(length),
    } = declaration.shape
    else {
        return None;
    };
    if element != expected_element {
        return None;
    }
    Some(length)
}

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
}

pub fn fixed_byte_array_loan<'types, 'domain, 'path>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    domains: impl Iterator<Item = &'domain terminal_psi::StructuralDomainDeclaration> + Clone,
    actual: &StructuralParameterDeclaration,
    argument: &'path StructuralArgument,
    expected: &StructuralParameterDeclaration,
) -> Option<FixedCollectionWindow<'path>> {
    let window = fixed_byte_array_window(types.clone(), actual, argument, expected)?;
    if !window.backing_path.is_empty() && !actual.qualifications.is_empty() {
        return None;
    }
    let carrier =
        runtime_structural_path_tip(types, actual.structural_type, window.backing_path)?.id;
    let (whole, projected) = terminal_psi::StructuralPathQualification::select(
        &actual.qualifications,
        &actual.projected_qualifications,
        window.backing_path,
    );
    if !projected.is_empty() {
        return None;
    }
    let original =
        crate::byte_qualification_predicates_in(domains.clone(), carrier, &whole)?;
    let required = crate::byte_parameter_predicates_in(domains, expected)?;
    let mut offered = original;
    if window.offset != 0 || window.length != window.backing_length {
        offered = terminal_psi::ByteSequencePredicateSet::EMPTY;
        for predicate in original.predicates() {
            if matches!(
                predicate,
                terminal_psi::ByteSequencePredicate::AsciiOnly
                    | terminal_psi::ByteSequencePredicate::NoNul
            ) {
                offered.insert(predicate);
            }
        }
    }
    (offered.implies(required)
        && (argument.access == StructuralAccess::SharedBorrow
            || required.implies(original)))
    .then_some(window)
}
