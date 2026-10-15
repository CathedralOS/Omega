//! Bounded byte owner storage at invocation, borrowed or passed by value.
//!
//! An explicit `BoundedOwned` byte carrier stores its live length at offset 0
//! and its capacity-sized payload at offset 8. A loan addresses that owner's
//! storage. This representation never describes `[u8; N] in D`, which remains
//! an exact fixed array with payload offset 0 and constant length N.
//! Shared and mutable loans share an address carrier while retaining distinct
//! effects and custody. Mutable stores preserve declared predicates, which
//! verification replays independently of the layout rule.
//!
//! This is a type rule, not producer evidence. Each stage still replays its
//! own edge sources, availability and origin custody.
//! The separate owned-parameter classifier below describes the by-value
//! `{length, bytes}` payload, never a loan or descriptor ABI. Keeping those
//! roles distinct prevents readable owner storage from authorizing an address
//! join or changing an owned argument into a borrowed pointer. A shared
//! borrowed byte view keeps its declared domain under the same rule.

use crate::abstract_operations::structural_type_catalog::unique_structural_type;
use semantic_vocabulary::StructuralTypeId;
use terminal_psi::{
    ByteSequenceCarrier, StructuralAccess, StructuralFieldType, StructuralMultiplicity,
    StructuralParameterDeclaration, StructuralTypeDeclaration, StructuralTypeShape,
};

/// The standalone shape of the storage a loan of this leaf field addresses.
/// A bounded byte owner field embeds its length word and bytes inline, the
/// exact storage an owner loan carries the address of
/// ([`is_bounded_owner_loan`]), so `&mut record.bytes` names the declaration
/// spelling that carrier. A byte view field stores a descriptor, not its
/// referent, and keeps `canonical_leaf_shape`'s refusal. Field predicates
/// bind to the field, never to this shape; the consuming loan's verification
/// still proves that they imply the borrower's declared domains.
pub fn loaned_field_type(field: &StructuralFieldType) -> Option<StructuralFieldType> {
    match field.byte_sequence_carrier() {
        Some(carrier @ ByteSequenceCarrier::BoundedOwned { .. }) => {
            Some(StructuralFieldType::ByteSequence(carrier))
        }
        _ => field.canonical_leaf_type(),
    }
}

/// A shared or mutable loan addresses the complete bounded owner's storage.
/// This classifies transport only; callers requiring a frozen referent must
/// use [`is_shared_bounded_owner_loan`] or check access explicitly.
pub fn is_bounded_owner_loan<'a>(
    parameter: &StructuralParameterDeclaration,
    lookup: impl Fn(StructuralTypeId) -> Option<&'a StructuralTypeDeclaration>,
) -> bool {
    matches!(
        parameter.access,
        terminal_psi::reading_borrow_pair_pattern!()
    ) && parameter.multiplicity == StructuralMultiplicity::Unrestricted
        && parameter.projected_qualifications.is_empty()
        && lookup(parameter.structural_type)
            .is_some_and(|declaration| declaration.shape.is_bounded_owned_byte_sequence())
}

/// [`is_bounded_owner_loan`] over independently unique type declarations.
pub fn is_bounded_owner_loan_in(
    parameter: &StructuralParameterDeclaration,
    types: &[StructuralTypeDeclaration],
) -> bool {
    is_bounded_owner_loan(parameter, |identity| {
        unique_structural_type(types.iter(), identity)
    })
}

/// Whether this parameter is a shared `&[u8]` view, with or without a declared
/// domain such as `&[u8] in Utf8`. The domain is a declaration-owned predicate
/// Terminal verification already replayed at every arrival; the descriptor
/// and its frozen referent are the same either way, so a view joined at a
/// block parameter needs no transport beyond the declaration. Writes, the
/// receiver and projected qualifications stay outside this rule.
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

/// [`is_shared_byte_view`] over a declaration slice whose identities are
/// unique.
pub fn is_shared_byte_view_in(
    parameter: &StructuralParameterDeclaration,
    types: &[StructuralTypeDeclaration],
) -> bool {
    is_shared_byte_view(parameter, |identity| {
        unique_structural_type(types.iter(), identity)
    })
}

/// An unrestricted bounded byte payload arriving by value. Qualifications
/// remain declaration-owned obligations; layout does not erase or establish
/// them. Consumers independently replay the signature and its aggregate ABI.
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

/// Slice form with an independently unique type declaration.
pub fn is_owned_bounded_byte_parameter_in(
    parameter: &StructuralParameterDeclaration,
    types: &[StructuralTypeDeclaration],
) -> bool {
    is_owned_bounded_byte_parameter(parameter, |identity| {
        unique_structural_type(types.iter(), identity)
    })
}

/// Whether this parameter is a shared loan of a record whose runtime members
/// are plain storage or shared-view descriptors (`&[T]`/`&[u8]` members). A
/// shared loan observes the stored descriptors without moving them: the
/// loan's referent is the record's own cells plus the referents those
/// descriptors name, all frozen for the loan's duration. This is a type
/// rule — each stage still replays its own edge sources and member custody.
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

/// [`is_shared_view_carrier_loan`] over a declaration slice whose identities
/// are unique.
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

/// A shared-view carrier's runtime members stay within plain storage and
/// descriptor shapes at every depth: a `Structural` member may itself be a
/// descriptor (`ElementView` or a borrowed byte view), a record recursing the
/// same contract, or a fixed array of plain leaves. Bounded owned bytes and
/// scalar members are plain storage; every other carrier declines.
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
            // A `&[u8]` member spells its shared descriptor directly on the
            // field type — the same loan an `ElementView` child declares.
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
