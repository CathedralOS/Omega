//! Bounded byte owner storage at invocation, borrowed or passed by value.
//!
//! A domain-qualified fixed byte array is a bounded owner: a live length
//! word at offset 0 and up to `N` bytes after it (wiki/spec/language/domains.md,
//! "Byte containers and encoding domains"). A shared loan of that owner is
//! carried as the owner's address, exactly the layout an owned bounded
//! concatenation result has in its own activation: the length is read at
//! offset 0 and the bytes start at offset 8. The loan freezes its referent,
//! so the length observed once stays current and the declared byte
//! predicates, which Terminal verification already replayed, need no
//! transport beyond the declaration.
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
    parameter.access == StructuralAccess::SharedBorrow
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

#[cfg(test)]
mod tests;
