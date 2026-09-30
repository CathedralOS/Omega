//! Shared loans of a bounded byte owner (`&[u8; N] in Utf8`).
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
