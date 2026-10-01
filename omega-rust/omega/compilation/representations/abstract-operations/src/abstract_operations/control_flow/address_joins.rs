//! Borrowed joins whose runtime carrier is the referent's address.
//!
//! Shared joins admit primitive scalars, service binding leaves and plain records
//! through static projections. Mutable joins admit records, plain sums and binding
//! leaves, whole or statically projected, while transferring the exact incoming
//! mutable permission. Neither carrier copies the referent.
//! A shared bounded byte owner loan (`owner_loans`) is also carried as its
//! owner's address; it keeps the owner's plain byte qualifications, and each
//! stage requires its edges to forward an identically qualified loan whole.
//! Byte and element views keep their `{base, extent}` descriptor carriers.
//!
//! This is a type rule, not producer evidence. Each stage independently replays
//! the sources, type, access and origin custody. Exclusive name availability
//! and shared-loan origin pinning remain separate receiving checks. Write-only
//! joins and referents containing loans or descriptors remain unsupported.

use semantic_vocabulary::StructuralTypeId;
use terminal_psi::{
    StructuralAccess, StructuralFieldDeclaration, StructuralFieldType, StructuralMultiplicity,
    StructuralParameterDeclaration, StructuralPathSegment, StructuralTypeDeclaration,
    StructuralTypeShape,
};

/// Whether this block parameter is carried as the address of its referent.
/// `lookup` resolves one structural identity in the caller's own catalog.
pub fn is_address_join<'a>(
    parameter: &StructuralParameterDeclaration,
    lookup: impl Fn(StructuralTypeId) -> Option<&'a StructuralTypeDeclaration> + Copy,
) -> bool {
    super::owner_loans::is_shared_bounded_owner_loan(parameter, lookup)
        || is_plain_address_join(parameter, lookup)
}

/// Unqualified address loans over plain referents, including existing bindings.
fn is_plain_address_join<'a>(
    parameter: &StructuralParameterDeclaration,
    lookup: impl Fn(StructuralTypeId) -> Option<&'a StructuralTypeDeclaration> + Copy,
) -> bool {
    (parameter.access == StructuralAccess::SharedBorrow
        || parameter.access == StructuralAccess::MutableBorrow
            && lookup(parameter.structural_type).is_some_and(|declaration| {
                matches!(
                    declaration.shape,
                    StructuralTypeShape::Record { .. }
                        | StructuralTypeShape::FusedServiceBinding
                        | StructuralTypeShape::Sum { .. }
                        | StructuralTypeShape::Mixed { .. }
                )
            }))
        && parameter.multiplicity == StructuralMultiplicity::Unrestricted
        && !parameter.is_self
        && parameter.qualifications.is_empty()
        && parameter.projected_qualifications.is_empty()
        && is_plain_referent(parameter.structural_type, lookup, &mut Vec::new())
}

/// [`is_address_join`] over a declaration slice whose identities are unique.
pub fn is_address_join_in(
    parameter: &StructuralParameterDeclaration,
    types: &[StructuralTypeDeclaration],
) -> bool {
    is_address_join(parameter, |identity| {
        let mut declarations = types
            .iter()
            .filter(|declaration| declaration.id == identity);
        match (declarations.next(), declarations.next()) {
            (Some(declaration), None) => Some(declaration),
            _ => None,
        }
    })
}

/// Whether an address-join edge argument names a static place beneath its
/// root. Byte windows, runtime indexes and reference hops have no single
/// address fixed by the path alone.
pub fn is_static_projection(path: &[StructuralPathSegment]) -> bool {
    path.iter().all(|segment| {
        matches!(
            segment,
            StructuralPathSegment::Field(_) | StructuralPathSegment::FixedIndex(_)
        )
    })
}

/// The referent type an address-join argument names: `path` resolved from
/// `root` through record fields and in-bounds fixed indexes, where an element
/// view contributes its own live extent as the index bound. A scalar leaf
/// field resolves to the catalog's unique standalone declaration of that
/// leaf's canonical shape, the same identity a borrowed `&u64` parameter
/// declares. Erased fields, duplicate identities and every other segment
/// fail closed.
pub fn referent_type<'a>(
    declarations: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> Option<StructuralTypeId> {
    let unique = |matches: &mut dyn Iterator<Item = &'a StructuralTypeDeclaration>| match (
        matches.next(),
        matches.next(),
    ) {
        (Some(declaration), None) => Some(declaration),
        _ => None,
    };
    let mut current = unique(
        &mut declarations
            .clone()
            .filter(|declaration| declaration.id == root),
    )?;
    for segment in path {
        let next = match (segment, &current.shape) {
            (StructuralPathSegment::Field(identity), StructuralTypeShape::Record { fields }) => {
                let mut matching = fields.iter().filter(|field| field.identity == *identity);
                let field = match (matching.next(), matching.next()) {
                    (Some(field), None) if !field.relevance.is_erased() => field,
                    _ => return None,
                };
                match &field.field_type {
                    StructuralFieldType::Structural(child) => unique(
                        &mut declarations
                            .clone()
                            .filter(|declaration| declaration.id == *child),
                    )?,
                    leaf => {
                        let shape = leaf.canonical_leaf_shape()?;
                        unique(
                            &mut declarations
                                .clone()
                                .filter(|declaration| declaration.shape == shape),
                        )?
                    }
                }
            }
            (
                StructuralPathSegment::FixedIndex(index),
                StructuralTypeShape::FixedArray { element, length },
            ) if index < length => unique(
                &mut declarations
                    .clone()
                    .filter(|declaration| declaration.id == *element),
            )?,
            // An element view carries no literal length: the producer's live
            // extent discharges the bound, and the path still names one
            // element's address.
            (
                StructuralPathSegment::FixedIndex(_),
                StructuralTypeShape::ElementView { element },
            ) => unique(
                &mut declarations
                    .clone()
                    .filter(|declaration| declaration.id == *element),
            )?,
            _ => return None,
        };
        current = next;
    }
    Some(current.id)
}

/// A primitive scalar, or an aggregate built only from owned scalar storage.
/// Fixed arrays retain their backing storage exactly like nested records;
/// they do not introduce a loan or descriptor into the borrowed referent.
/// Case tags and owned payloads likewise remain in that backing storage. All
/// alternatives must be plain: the address transport cannot depend on which
/// case happens to be active when an edge is taken.
/// `active` rejects malformed recursive catalogs, including array cycles.
fn is_plain_referent<'a>(
    identity: StructuralTypeId,
    lookup: impl Fn(StructuralTypeId) -> Option<&'a StructuralTypeDeclaration> + Copy,
    active: &mut Vec<StructuralTypeId>,
) -> bool {
    if active.contains(&identity) {
        return false;
    }
    let Some(declaration) = lookup(identity) else {
        return false;
    };
    active.push(identity);
    let plain = match &declaration.shape {
        StructuralTypeShape::PrimitiveScalar(_) => true,
        // Address transport preserves an established binding occurrence. This
        // says nothing about construction or zero validity of its authority.
        StructuralTypeShape::FusedServiceBinding => true,
        // Empty records still have a borrowed address carrier. No payload
        // bytes are copied, and the join retains its place and loan custody.
        StructuralTypeShape::Record { fields } => plain_fields(fields, lookup, active),
        StructuralTypeShape::FixedArray { element, .. } => {
            is_plain_referent(*element, lookup, active)
        }
        StructuralTypeShape::Sum { cases } => {
            !cases.is_empty()
                && cases
                    .iter()
                    .all(|case| plain_fields(&case.fields, lookup, active))
        }
        StructuralTypeShape::Mixed { fields, cases } => {
            plain_fields(fields, lookup, active)
                && !cases.is_empty()
                && cases
                    .iter()
                    .all(|case| plain_fields(&case.fields, lookup, active))
        }
        StructuralTypeShape::Reference { .. }
        | StructuralTypeShape::ByteSequence(_)
        | StructuralTypeShape::ElementView { .. } => false,
    };
    active.pop();
    plain
}

fn plain_fields<'a>(
    fields: &[StructuralFieldDeclaration],
    lookup: impl Fn(StructuralTypeId) -> Option<&'a StructuralTypeDeclaration> + Copy,
    active: &mut Vec<StructuralTypeId>,
) -> bool {
    fields.iter().all(|field| {
        // Erased members retain semantic identity, but add neither storage nor
        // a runtime loan to the referent. Transporting its address does not
        // materialize them; referent_type still rejects projecting into them.
        if field.relevance.is_erased() {
            return true;
        }
        match &field.field_type {
            StructuralFieldType::Scalar(_)
            | StructuralFieldType::BoundedInteger(_)
            | StructuralFieldType::IeeeFloat(_) => true,
            StructuralFieldType::Structural(child) => is_plain_referent(*child, lookup, active),
            StructuralFieldType::ByteSequence(_)
            | StructuralFieldType::PredicateByteSequence { .. }
            | StructuralFieldType::Erased { .. } => false,
        }
    })
}

#[cfg(test)]
mod tests;
