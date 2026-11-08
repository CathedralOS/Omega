// We classify bounded byte owner storage at invocation (borrowed as an owner
// loan or passed by value) and shared loans of records whose members are plain
// storage or shared-view descriptors. Keeping borrowed owner loans distinct
// from by-value `{length, bytes}` parameters prevents readable owner storage
// from authorizing an address join or turning an owned argument into a borrowed
// pointer.

use crate::structural_type_catalog::unique_structural_type;
use semantic_vocabulary::StructuralTypeId;
use terminal_psi::{
    ByteSequenceCarrier, StructuralAccess, StructuralFieldType, StructuralMultiplicity,
    StructuralParameterDeclaration, StructuralTypeDeclaration, StructuralTypeShape,
    reading_borrow_pair_pattern,
};

pub fn loaned_field_type(field: &StructuralFieldType) -> Option<StructuralFieldType> {
    match field.byte_sequence_carrier() {
        Some(carrier @ ByteSequenceCarrier::BoundedOwned { .. }) => {
            Some(StructuralFieldType::ByteSequence(carrier))
        }
        _ => field.canonical_leaf_type(),
    }
}

pub fn is_bounded_owner_loan<'a>(
    parameter: &StructuralParameterDeclaration,
    lookup: impl Fn(StructuralTypeId) -> Option<&'a StructuralTypeDeclaration>,
) -> bool {
    matches!(parameter.access, reading_borrow_pair_pattern!())
        && parameter.multiplicity == StructuralMultiplicity::Unrestricted
        && parameter.projected_qualifications.is_empty()
        && lookup(parameter.structural_type)
            .is_some_and(|declaration| declaration.shape.is_bounded_owned_byte_sequence())
}

pub fn is_bounded_owner_loan_in(
    parameter: &StructuralParameterDeclaration,
    types: &[StructuralTypeDeclaration],
) -> bool {
    is_bounded_owner_loan(parameter, |identity| {
        unique_structural_type(types.iter(), identity)
    })
}

pub fn is_shared_byte_view<'a>(
    parameter: &StructuralParameterDeclaration,
    lookup: impl Fn(StructuralTypeId) -> Option<&'a StructuralTypeDeclaration>,
) -> bool {
    parameter.access == StructuralAccess::SharedBorrow
        && parameter.multiplicity == StructuralMultiplicity::Unrestricted
        && parameter.projected_qualifications.is_empty()
        && lookup(parameter.structural_type).is_some_and(|declaration| {
            matches!(
                declaration.shape,
                StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView { .. })
            )
        })
}

pub fn is_owned_bounded_byte_parameter<'a>(
    parameter: &StructuralParameterDeclaration,
    lookup: impl Fn(StructuralTypeId) -> Option<&'a StructuralTypeDeclaration>,
) -> bool {
    parameter.access == StructuralAccess::Owned
        && parameter.multiplicity == StructuralMultiplicity::Unrestricted
        && parameter.projected_qualifications.is_empty()
        && lookup(parameter.structural_type)
            .is_some_and(|declaration| declaration.shape.is_bounded_owned_byte_sequence())
}

pub fn is_owned_bounded_byte_parameter_in(
    parameter: &StructuralParameterDeclaration,
    types: &[StructuralTypeDeclaration],
) -> bool {
    is_owned_bounded_byte_parameter(parameter, |identity| {
        unique_structural_type(types.iter(), identity)
    })
}

pub fn is_shared_view_carrier_loan<'a>(
    parameter: &StructuralParameterDeclaration,
    lookup: impl Fn(StructuralTypeId) -> Option<&'a StructuralTypeDeclaration> + Copy,
    types: &'a [StructuralTypeDeclaration],
) -> bool {
    parameter.access == StructuralAccess::SharedBorrow
        && parameter.multiplicity == StructuralMultiplicity::Unrestricted
        && parameter.projected_qualifications.is_empty()
        && lookup(parameter.structural_type)
            .is_some_and(|declaration| view_carrier_shape(declaration, types, &mut Vec::new()))
}

pub fn is_shared_view_carrier_loan_in(
    parameter: &StructuralParameterDeclaration,
    types: &[StructuralTypeDeclaration],
) -> bool {
    is_shared_view_carrier_loan(
        parameter,
        |identity| unique_structural_type(types.iter(), identity),
        types,
    )
}

fn view_carrier_shape(
    declaration: &StructuralTypeDeclaration,
    types: &[StructuralTypeDeclaration],
    active: &mut Vec<StructuralTypeId>,
) -> bool {
    fn member_valid(
        field_type: &terminal_psi::StructuralFieldType,
        types: &[StructuralTypeDeclaration],
        active: &mut Vec<StructuralTypeId>,
    ) -> bool {
        match field_type {
            terminal_psi::StructuralFieldType::Structural(child) => {
                let mut declarations = types.iter().filter(|candidate| candidate.id == *child);
                let Some(child_declaration) = declarations.next() else {
                    return false;
                };
                if declarations.next().is_some() || active.contains(&child_declaration.id) {
                    return false;
                }
                match &child_declaration.shape {
                    StructuralTypeShape::ElementView { .. }
                    | StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView {
                        ..
                    }) => true,
                    _ => {
                        active.push(child_declaration.id);
                        let valid = view_carrier_shape(child_declaration, types, active);
                        active.pop();
                        valid
                    }
                }
            }
            terminal_psi::StructuralFieldType::ByteSequence(
                ByteSequenceCarrier::BoundedOwned { .. },
            )
            | terminal_psi::StructuralFieldType::PredicateByteSequence {
                carrier: ByteSequenceCarrier::BoundedOwned { .. },
                ..
            } => true,
            terminal_psi::StructuralFieldType::ByteSequence(
                ByteSequenceCarrier::BorrowedView { .. },
            )
            | terminal_psi::StructuralFieldType::PredicateByteSequence {
                carrier: ByteSequenceCarrier::BorrowedView { .. },
                ..
            } => true,
            terminal_psi::StructuralFieldType::Erased { .. } => true,
            _ => field_type.scalar_type().is_some(),
        }
    }
    match &declaration.shape {
        terminal_psi::record_or_mixed_shape_pattern!(fields) => fields.iter().all(|field| {
            field.relevance.is_erased()
                && matches!(
                    field.field_type,
                    terminal_psi::StructuralFieldType::Erased { .. }
                )
                || (field.relevance == terminal_psi::BindingRelevance::Relevant
                    && member_valid(&field.field_type, types, active))
        }),
        _ => false,
    }
}
