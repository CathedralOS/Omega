//! Bounded byte owner storage at invocation, borrowed or passed by value.
//!
//! A domain-qualified fixed byte array is a bounded owner: a live length
//! word at offset 0 and up to `N` bytes after it (wiki/spec/language/domains.md,
//! "Byte containers and encoding domains"). A loan of that owner is
//! carried as the owner's address, exactly the layout an owned bounded
//! concatenation result has in its own activation: the length is read at
//! offset 0 and the bytes start at offset 8. Shared and mutable loans have the
//! same address carrier, not the same effects: only a shared loan freezes its
//! referent. Mutable stores must retain current lengths and preserve declared
//! predicates, which verification replays independently of this layout rule.
//!
//! This is a type rule, not producer evidence. Each stage still replays its
//! own edge sources, availability and origin custody.
//! The separate owned-parameter classifier below describes the by-value
//! `{length, bytes}` payload, never a loan or descriptor ABI. Keeping those
//! roles distinct prevents readable owner storage from authorizing an address
//! join or changing an owned argument into a borrowed pointer. A shared
//! borrowed byte view keeps its declared domain under the same rule.

use semantic_vocabulary::StructuralTypeId;
use terminal_psi::{
    ByteSequenceCarrier, StructuralAccess, StructuralMultiplicity, StructuralParameterDeclaration,
    StructuralTypeDeclaration, StructuralTypeShape,
};

/// Whether this parameter is a shared loan of a bounded byte owner.
/// `lookup` resolves one structural identity in the caller's own catalog.
pub fn is_shared_bounded_owner_loan<'a>(
    parameter: &StructuralParameterDeclaration,
    lookup: impl Fn(StructuralTypeId) -> Option<&'a StructuralTypeDeclaration>,
) -> bool {
    parameter.access == StructuralAccess::SharedBorrow && is_bounded_owner_loan(parameter, lookup)
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
        StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
    ) && parameter.multiplicity == StructuralMultiplicity::Unrestricted
        && !parameter.is_self
        && parameter.projected_qualifications.is_empty()
        && lookup(parameter.structural_type).is_some_and(|declaration| {
            matches!(
                declaration.shape,
                StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedOwned { .. })
            )
        })
}

/// [`is_bounded_owner_loan`] over independently unique type declarations.
pub fn is_bounded_owner_loan_in(
    parameter: &StructuralParameterDeclaration,
    types: &[StructuralTypeDeclaration],
) -> bool {
    is_bounded_owner_loan(parameter, |identity| {
        let mut declarations = types
            .iter()
            .filter(|declaration| declaration.id == identity);
        match (declarations.next(), declarations.next()) {
            (Some(declaration), None) => Some(declaration),
            _ => None,
        }
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
        && !parameter.is_self
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
        let mut declarations = types
            .iter()
            .filter(|declaration| declaration.id == identity);
        match (declarations.next(), declarations.next()) {
            (Some(declaration), None) => Some(declaration),
            _ => None,
        }
    })
}

/// [`is_shared_bounded_owner_loan`] over a declaration slice whose identities
/// are unique.
pub fn is_shared_bounded_owner_loan_in(
    parameter: &StructuralParameterDeclaration,
    types: &[StructuralTypeDeclaration],
) -> bool {
    is_shared_bounded_owner_loan(parameter, |identity| {
        let mut declarations = types
            .iter()
            .filter(|declaration| declaration.id == identity);
        match (declarations.next(), declarations.next()) {
            (Some(declaration), None) => Some(declaration),
            _ => None,
        }
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
        && !parameter.is_self
        && parameter.projected_qualifications.is_empty()
        && lookup(parameter.structural_type).is_some_and(|declaration| {
            matches!(
                declaration.shape,
                StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedOwned { .. })
            )
        })
}

/// Slice form with an independently unique type declaration.
pub fn is_owned_bounded_byte_parameter_in(
    parameter: &StructuralParameterDeclaration,
    types: &[StructuralTypeDeclaration],
) -> bool {
    is_owned_bounded_byte_parameter(parameter, |identity| {
        let mut declarations = types
            .iter()
            .filter(|declaration| declaration.id == identity);
        match (declarations.next(), declarations.next()) {
            (Some(declaration), None) => Some(declaration),
            _ => None,
        }
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
        && !parameter.is_self
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
        |identity| {
            let mut declarations = types
                .iter()
                .filter(|declaration| declaration.id == identity);
            match (declarations.next(), declarations.next()) {
                (Some(declaration), None) => Some(declaration),
                _ => None,
            }
        },
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
        StructuralTypeShape::Record { fields } | StructuralTypeShape::Mixed { fields, .. } => {
            fields.iter().all(|field| {
                field.relevance.is_erased()
                    && matches!(
                        field.field_type,
                        terminal_psi::StructuralFieldType::Erased { .. }
                    )
                    || (field.relevance == terminal_psi::BindingRelevance::Relevant
                        && member_valid(&field.field_type, types, active))
            })
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests;
