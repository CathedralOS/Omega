//! Borrowed joins whose runtime carrier is the referent's address.
//!
//! Shared joins admit primitive scalars, service binding leaves and aggregate storage
//! through static projections. Exclusive joins admit the same referents,
//! whole or statically projected, while transferring the exact incoming
//! mutable or write-only permission. Neither carrier copies the referent.
//! Bounded byte owner loans likewise carry the owner's address. Qualifications
//! describe obligations on that storage, not a different address carrier;
//! semantic admission owns their preservation and implication at each edge.
//! Byte and element views keep their `{base, extent}` descriptor carriers.
//!
//! This classifies physical transport, not producer evidence. Source semantics
//! and exact edge custody remain separate from the address representation.
//! Exclusive name availability
//! and shared-loan origin pinning remain separate receiving checks. Carrying
//! an address grants no readable authority to a write-only loan. Stored reference
//! and view fields stay in the referent: borrowing its address neither copies
//! those descriptors nor relocates their backing. Psi checks their lifetimes;
//! native transport uses the same address regardless of the contents. Top-level
//! views and reference carriers keep their distinct descriptor transport.

#[cfg(test)]
mod tests;

use crate::abstract_operations::structural_type_catalog::unique_structural_type;
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
    matches!(
        parameter.access,
        StructuralAccess::SharedBorrow
            | StructuralAccess::MutableBorrow
            | StructuralAccess::WriteOnlyBorrow
    ) && parameter.multiplicity == StructuralMultiplicity::Unrestricted
        && lookup(parameter.structural_type).is_some_and(|declaration| {
            matches!(
                declaration.shape,
                StructuralTypeShape::PrimitiveScalar(_)
                    | StructuralTypeShape::BindingCarrier
                    | StructuralTypeShape::Record { .. }
                    | StructuralTypeShape::FixedArray { .. }
                    | StructuralTypeShape::Sum { .. }
                    | StructuralTypeShape::Mixed { .. }
                    | StructuralTypeShape::ByteSequence(
                        terminal_psi::ByteSequenceCarrier::BoundedOwned { .. }
                    )
            )
        })
}

/// [`is_address_join`] over a declaration slice whose identities are unique.
pub fn is_address_join_in(
    parameter: &StructuralParameterDeclaration,
    types: &[StructuralTypeDeclaration],
) -> bool {
    is_address_join(parameter, |identity| {
        unique_structural_type(types.iter(), identity)
    })
}

/// Whether an address-join edge argument names a static place beneath its
/// root. Byte windows, runtime indexes and reference hops have no single
/// address fixed by the path alone. Case selection fixes payload geometry,
/// not membership: Terminal verification separately proves the case active.
pub fn is_static_projection(path: &[StructuralPathSegment]) -> bool {
    path.iter().all(|segment| {
        matches!(
            segment,
            StructuralPathSegment::Field(_)
                | StructuralPathSegment::Case(_)
                | StructuralPathSegment::FixedIndex(_)
        )
    })
}

/// A retained-address projection permits typed reference crossings and already
/// evaluated selectors. Each runtime selector carries its occurrence's bounds
/// obligation; this vocabulary query grants neither that evidence nor custody.
pub fn is_address_projection(path: &[StructuralPathSegment]) -> bool {
    path.iter().all(|segment| {
        matches!(
            segment,
            StructuralPathSegment::Field(_)
                | StructuralPathSegment::Case(_)
                | StructuralPathSegment::FixedIndex(_)
                | StructuralPathSegment::Referent
                | StructuralPathSegment::RuntimeIndex { .. }
        )
    })
}

/// The access ceiling retained along an exact address projection. An owned
/// container can forward its stored descriptor's loan without lending mutable
/// container storage. Shared enclosing access attenuates descendants; reading
/// a stored pointer through write-only storage is forbidden. This is only a
/// catalog/type judgment: each receiver must independently establish the live
/// root, its owner, qualifications and exact endpoint. See structural_access.md
/// (Projection across reference fields) and loans.md (Reborrow lineage).
pub fn loan_access_permitted<'a>(
    declarations: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
    source: StructuralAccess,
    destination: StructuralAccess,
) -> bool {
    use StructuralAccess::{MutableBorrow, Owned, SharedBorrow, WriteOnlyBorrow};
    if !is_address_projection(path)
        || borrowed_referent_type(declarations.clone(), root, path, destination).is_none()
    {
        return false;
    }
    let mut ceiling = source;
    for (position, segment) in path.iter().enumerate() {
        if *segment != StructuralPathSegment::Referent {
            continue;
        }
        if ceiling == WriteOnlyBorrow {
            return false;
        }
        let Some(reference) = referent_type(declarations.clone(), root, &path[..position])
            .and_then(|identity| unique_structural_type(declarations.clone(), identity))
        else {
            return false;
        };
        let StructuralTypeShape::Reference { access, .. } = reference.shape else {
            return false;
        };
        ceiling = match ceiling {
            SharedBorrow => SharedBorrow,
            Owned | MutableBorrow => access,
            WriteOnlyBorrow => return false,
        };
    }
    matches!(
        (ceiling, destination),
        (
            MutableBorrow,
            SharedBorrow | MutableBorrow | WriteOnlyBorrow
        ) | (Owned | SharedBorrow, SharedBorrow)
            | (WriteOnlyBorrow, WriteOnlyBorrow)
    )
}

/// Resolve a loan endpoint while checking authority at every pointer crossing.
/// This type query does not establish that a carrier or its backing is live.
pub fn borrowed_referent_type<'a>(
    declarations: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
    access: StructuralAccess,
) -> Option<StructuralTypeId> {
    if !is_address_projection(path) || access == StructuralAccess::Owned {
        return None;
    }
    for (position, segment) in path.iter().enumerate() {
        if *segment != StructuralPathSegment::Referent {
            continue;
        }
        let reference = referent_type(declarations.clone(), root, &path[..position])?;
        let declaration = unique_structural_type(declarations.clone(), reference)?;
        let StructuralTypeShape::Reference {
            access: permission, ..
        } = declaration.shape
        else {
            return None;
        };
        if !(permission == access
            || permission == StructuralAccess::MutableBorrow
                && matches!(
                    access,
                    StructuralAccess::SharedBorrow | StructuralAccess::WriteOnlyBorrow
                ))
            || permission == StructuralAccess::WriteOnlyBorrow && position + 1 != path.len()
        {
            return None;
        }
    }
    referent_type(declarations, root, path)
}

/// The referent type an address-join argument names: `path` resolved from
/// `root` through record or selected-case fields and retained element indexes.
/// An element view contributes its own live extent as the index bound. A leaf
/// field resolves to the catalog's unique standalone declaration of the
/// storage a loan of it addresses (`owner_loans::loaned_field_type`): a
/// scalar's canonical shape, the same identity a borrowed `&u64` parameter
/// declares, or a bounded byte owner's carrier. Erased fields, duplicate
/// identities and every other segment fail closed.
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
    let mut payload_fields = None;
    for segment in path {
        if let StructuralPathSegment::Case(identity) = segment {
            if payload_fields.is_some() {
                return None;
            }
            let (StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. }) =
                &current.shape
            else {
                return None;
            };
            let mut matching = cases.iter().filter(|case| case.id == *identity);
            let selected = matching.next()?;
            if matching.next().is_some() {
                return None;
            }
            payload_fields = Some(selected.fields.as_slice());
            continue;
        }
        if payload_fields.is_some() && !matches!(segment, StructuralPathSegment::Field(_)) {
            return None;
        }
        let next = match (segment, &current.shape) {
            (StructuralPathSegment::Referent, StructuralTypeShape::Reference { referent, .. }) => {
                unique(
                    &mut declarations
                        .clone()
                        .filter(|declaration| declaration.id == *referent),
                )?
            }
            (StructuralPathSegment::Field(identity), shape) => {
                let fields = match payload_fields.take() {
                    Some(fields) => fields,
                    None => match shape {
                        terminal_psi::record_or_mixed_shape_pattern!(fields) => fields.as_slice(),
                        _ => return None,
                    },
                };
                let mut matching = fields.iter().filter(|field| field.identity == *identity);
                let field = match (matching.next(), matching.next()) {
                    (Some(field), None)
                        if !field.relevance.is_erased()
                            && fields
                                .iter()
                                .filter(|candidate| candidate.id == field.id)
                                .count()
                                == 1 =>
                    {
                        field
                    }
                    _ => return None,
                };
                match &field.field_type {
                    StructuralFieldType::Structural(child) => unique(
                        &mut declarations
                            .clone()
                            .filter(|declaration| declaration.id == *child),
                    )?,
                    leaf => super::owner_loans::loaned_field_type(leaf)?
                        .leaf_declaration(declarations.clone())?,
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
                StructuralPathSegment::FixedIndex(_) | StructuralPathSegment::RuntimeIndex { .. },
                StructuralTypeShape::ElementView { element, .. },
            ) => unique(
                &mut declarations
                    .clone()
                    .filter(|declaration| declaration.id == *element),
            )?,
            (
                StructuralPathSegment::RuntimeIndex { .. },
                StructuralTypeShape::FixedArray { element, .. },
            ) => unique(
                &mut declarations
                    .clone()
                    .filter(|declaration| declaration.id == *element),
            )?,
            _ => return None,
        };
        current = next;
    }
    payload_fields.is_none().then_some(current.id)
}
