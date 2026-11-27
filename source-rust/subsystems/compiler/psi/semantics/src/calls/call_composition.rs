pub(crate) mod bounded_byte_owner;
pub(crate) mod fixed_byte_view;
pub use bounded_byte_owner::bounded_byte_owner_capacity;
pub use fixed_byte_view::{
    BorrowedReferentPresentation, FixedCollectionWindow, borrowed_referent_matches,
    borrowed_referent_presentation, fixed_byte_array_loan, fixed_byte_array_window,
    fixed_byte_extent, fixed_byte_storage_window, fixed_collection_storage_window,
    fixed_element_array_extent,
};

use semantic_vocabulary::StructuralTypeId;
use terminal_psi::{
    ByteSequenceCarrier, StructuralAccess, StructuralArgument, StructuralParameterDeclaration,
    StructuralTypeDeclaration, StructuralTypeShape,
};

use crate::runtime_structural_field;

pub fn boundary_buffer_capacity<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    root_type: StructuralTypeId,
    argument: &StructuralArgument,
    expected: &StructuralParameterDeclaration,
) -> Option<u64> {
    inline_byte_field_capacity(
        types,
        root_type,
        argument,
        expected,
        StructuralAccess::MutableBorrow,
    )
}

pub fn shared_boundary_buffer_capacity<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    root_type: StructuralTypeId,
    argument: &StructuralArgument,
    expected: &StructuralParameterDeclaration,
) -> Option<u64> {
    inline_byte_field_capacity(
        types,
        root_type,
        argument,
        expected,
        StructuralAccess::SharedBorrow,
    )
}

pub fn stored_view_field_presentation<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    root_type: StructuralTypeId,
    argument: &StructuralArgument,
    expected: &StructuralParameterDeclaration,
) -> Option<terminal_psi::ByteSequencePredicateSet> {
    if argument.access != StructuralAccess::SharedBorrow
        || expected.access != StructuralAccess::SharedBorrow
        || !expected.projected_qualifications.is_empty()
        || !types.clone().any(|declaration| {
            declaration.id == expected.structural_type
                && declaration.shape.is_shared_readable_byte_view()
        })
    {
        return None;
    }
    let field = runtime_structural_field(types, root_type, &argument.path)?;
    (field.field_type.byte_sequence_carrier()?
        == ByteSequenceCarrier::BorrowedView {
            access: Some(StructuralAccess::SharedBorrow),
        })
    .then(|| field.field_type.byte_predicates())
}

fn inline_byte_field_capacity<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    root_type: StructuralTypeId,
    argument: &StructuralArgument,
    expected: &StructuralParameterDeclaration,
    access: StructuralAccess,
) -> Option<u64> {
    if argument.access != access
        || expected.access != access
        || !expected.qualifications.is_empty()
        || !expected.projected_qualifications.is_empty()
        || !types.clone().any(|declaration| {
            declaration.id == expected.structural_type
                && matches!(
                    declaration.shape,
                    StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView { .. })
                )
        })
    {
        return None;
    }
    if access == StructuralAccess::SharedBorrow && argument.path.is_empty() {
        return types
            .clone()
            .find_map(|declaration| match declaration.shape {
                StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedOwned {
                    capacity,
                }) if declaration.id == root_type => Some(capacity),
                _ => None,
            });
    }
    let field = runtime_structural_field(types, root_type, &argument.path)?;
    if access != StructuralAccess::SharedBorrow
        && !field.field_type.byte_predicates().is_empty()
    {
        return None;
    }
    match field.field_type.byte_sequence_carrier()? {
        ByteSequenceCarrier::BoundedOwned { capacity } => Some(capacity),
        _ => None,
    }
}
