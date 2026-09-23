//! Shared-borrow joins whose runtime carrier is the referent's address.
//!
//! A `SharedBorrow` block parameter over a primitive scalar or a plain record
//! is a thin reference: each incoming edge lends the address of one readable
//! root, or of a static field/index projection beneath it, and the joined
//! place owns no storage, copies no payload and moves no custody. Byte and
//! element views keep their `{base, extent}` descriptor carriers instead.
//!
//! This predicate is the shared type rule every Omega stage applies to the
//! declaration it is handed; it is not producer evidence. Each stage still
//! replays the edge sources, exact referent types, access and origin custody
//! against its own representation. Exclusive joins and referents that carry
//! loans or descriptors stay closed: their edge custody needs more than an
//! address.

use semantic_vocabulary::StructuralTypeId;
use terminal_psi::{
    StructuralAccess, StructuralFieldType, StructuralMultiplicity, StructuralParameterDeclaration,
    StructuralPathSegment, StructuralTypeDeclaration, StructuralTypeShape,
};

/// Whether this block parameter is carried as the address of its referent.
/// `lookup` resolves one structural identity in the caller's own catalog.
pub fn is_address_join<'a>(
    parameter: &StructuralParameterDeclaration,
    lookup: impl Fn(StructuralTypeId) -> Option<&'a StructuralTypeDeclaration> + Copy,
) -> bool {
    parameter.access == StructuralAccess::SharedBorrow
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
/// `root` through record fields and in-bounds fixed indexes. A scalar leaf
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
            _ => return None,
        };
        current = next;
    }
    Some(current.id)
}

/// A primitive scalar, or a record built only from scalar leaves and plain
/// records. `active` holds the records being walked, so a malformed recursive
/// catalog fails closed instead of looping.
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
    match &declaration.shape {
        StructuralTypeShape::PrimitiveScalar(_) => true,
        StructuralTypeShape::Record { fields } => {
            active.push(identity);
            let plain = !fields.is_empty()
                && fields.iter().all(|field| match &field.field_type {
                    StructuralFieldType::Scalar(_)
                    | StructuralFieldType::BoundedInteger(_)
                    | StructuralFieldType::IeeeFloat(_) => true,
                    StructuralFieldType::Structural(child) => {
                        lookup(*child).is_some_and(|child| {
                            matches!(child.shape, StructuralTypeShape::Record { .. })
                        }) && is_plain_referent(*child, lookup, active)
                    }
                    StructuralFieldType::ByteSequence(_) | StructuralFieldType::Erased { .. } => {
                        false
                    }
                });
            active.pop();
            plain
        }
        StructuralTypeShape::Reference { .. }
        | StructuralTypeShape::ByteSequence(_)
        | StructuralTypeShape::ElementView { .. }
        | StructuralTypeShape::FixedArray { .. }
        | StructuralTypeShape::Sum { .. }
        | StructuralTypeShape::Mixed { .. } => false,
    }
}

#[cfg(test)]
mod tests;
