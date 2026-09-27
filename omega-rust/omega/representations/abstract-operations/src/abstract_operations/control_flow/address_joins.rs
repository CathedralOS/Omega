//! Borrowed joins whose runtime carrier is the referent's address.
//!
//! Shared joins admit primitive scalars and plain records through static
//! projections. Mutable joins admit whole plain records while transferring the
//! exact incoming mutable permission. Neither carrier copies the referent.
//! Byte and element views keep their `{base, extent}` descriptor carriers.
//!
//! This is a type rule, not producer evidence. Each stage independently replays
//! the sources, type, access and origin custody. Exclusive name availability
//! and shared-loan origin pinning remain separate receiving checks. Write-only
//! joins and referents containing loans or descriptors remain unsupported.

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
    (parameter.access == StructuralAccess::SharedBorrow
        || parameter.access == StructuralAccess::MutableBorrow
            && lookup(parameter.structural_type).is_some_and(|declaration| {
                matches!(declaration.shape, StructuralTypeShape::Record { .. })
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

/// A primitive scalar, or an aggregate built only from owned scalar storage.
/// Fixed arrays retain their backing storage exactly like nested records;
/// they do not introduce a loan or descriptor into the borrowed referent.
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
                            matches!(
                                child.shape,
                                StructuralTypeShape::Record { .. }
                                    | StructuralTypeShape::FixedArray { .. }
                            )
                        }) && is_plain_referent(*child, lookup, active)
                    }
                    StructuralFieldType::ByteSequence(_) | StructuralFieldType::Erased { .. } => {
                        false
                    }
                });
            active.pop();
            plain
        }
        StructuralTypeShape::FixedArray { element, .. } => {
            active.push(identity);
            let plain = is_plain_referent(*element, lookup, active);
            active.pop();
            plain
        }
        StructuralTypeShape::Reference { .. }
        | StructuralTypeShape::ByteSequence(_)
        | StructuralTypeShape::ElementView { .. }
        | StructuralTypeShape::Sum { .. }
        | StructuralTypeShape::Mixed { .. } => false,
    }
}

#[cfg(test)]
mod tests;
