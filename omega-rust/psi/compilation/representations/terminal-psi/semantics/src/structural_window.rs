//! The exact subtree a restoration window vacates and reseats.
//!
//! `path` always selects storage. An optional field extends that path by its
//! declaration-local identity; without a field the path is already complete.
//! In particular, an index can select either a whole element or the parent of
//! a further field. Guessing from the last segment conflates those operations
//! and is ambiguous when different records reuse a field ID.
//!
//! An element view's extent is its runtime length, not a type literal, so a
//! view element is selected by a `RuntimeIndex` bounded by that exact
//! descriptor's length at its full prefix. A literal `FixedIndex` never
//! crosses into a view. Nested descriptors retain their captured projection;
//! the enclosing record is neither their backing nor their extent.
//!
//! This is geometry, not permission: consumers still authenticate the root,
//! runtime bounds, live loan, value custody and exact restoration debt.

use semantic_vocabulary::{PlaceId, StructuralFieldId, StructuralTypeId};
use terminal_psi::{StructuralPathSegment, StructuralTypeDeclaration, StructuralTypeShape};

/// Declaration-derived complete projection and the subtree it selects.
pub struct StructuralWindowProjection {
    pub path: Vec<StructuralPathSegment>,
    pub structural_type: StructuralTypeId,
}

/// Authenticate every reconstructed backing of a whole-referent window.
/// A captured exclusive loan can originate at an exclusive parameter or at
/// established owned or exclusive storage. The caller authenticates storage against
/// its current custody, never pointer bits or a producer origin assertion.
/// No candidate may be discarded merely because another one is admissible.
pub fn structural_window_origins<'parameters>(
    parameters: impl Iterator<Item = &'parameters terminal_psi::StructuralParameterDeclaration> + Clone,
    origins: impl IntoIterator<Item = PlaceId>,
    required_access: terminal_psi::StructuralAccess,
    mut live_storage: impl FnMut(PlaceId) -> bool,
) -> bool {
    let mut found_origin = false;
    for root in origins {
        found_origin = true;
        if !live_storage(root)
            && !parameters.clone().any(|parameter| {
                parameter.place == root
                    && required_access.admits_referent(parameter.access)
                    && parameter.multiplicity != terminal_psi::StructuralMultiplicity::Linear
                    && parameter.qualifications.is_empty()
                    && parameter.projected_qualifications.is_empty()
            })
        {
            return false;
        }
    }
    found_origin
}

/// Whether the window selects a child of the storage root it reaches. Crossing
/// a reference starts a new root: the carrier's enclosing fields do not make
/// the whole referent a child window. This classifies geometry only; callers
/// still validate the path and authenticate authority over that storage.
pub fn structural_window_selects_child(
    path: &[StructuralPathSegment],
    field: Option<StructuralFieldId>,
) -> bool {
    field.is_some()
        || path
            .iter()
            .rev()
            .take_while(|segment| !matches!(segment, StructuralPathSegment::Referent))
            .any(|segment| {
                matches!(
                    segment,
                    StructuralPathSegment::Field(_)
                        | StructuralPathSegment::FixedIndex(_)
                        | StructuralPathSegment::RuntimeIndex { .. }
                )
            })
}

/// Resolve a window below an already authenticated storage root. Reference
/// crossings describe geometry only; consumers must independently resolve the
/// live loan. Owning a carrier does not grant ownership of its referent.
pub fn structural_window_projection<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
    field: Option<StructuralFieldId>,
) -> Option<StructuralWindowProjection> {
    let declaration = |identity| {
        let mut matches = types.clone().filter(|candidate| candidate.id == identity);
        let selected = matches.next()?;
        matches.next().is_none().then_some(selected)
    };
    let mut selected = declaration(root)?;
    for segment in path {
        let child = match (segment, &selected.shape) {
            (StructuralPathSegment::Referent, StructuralTypeShape::Reference { referent, .. }) => {
                *referent
            }
            (
                StructuralPathSegment::Field(identity),
                terminal_psi::record_or_mixed_shape_pattern!(fields),
            ) => {
                let mut matches = fields.iter().filter(|field| field.identity == *identity);
                let field = matches.next()?;
                if matches.next().is_some() || field.relevance.is_erased() {
                    return None;
                }
                let child = field.field_type.as_structural()?;
                child
            }
            (
                StructuralPathSegment::FixedIndex(index),
                StructuralTypeShape::FixedArray { element, length },
            ) if index < length => *element,
            (
                StructuralPathSegment::RuntimeIndex { .. },
                StructuralTypeShape::FixedArray { element, .. },
            ) => *element,
            // Bounds and authority belong to this projected descriptor, not
            // to its containing carrier; this query resolves only its type.
            (
                StructuralPathSegment::RuntimeIndex { .. },
                StructuralTypeShape::ElementView { element },
            ) => *element,
            _ => return None,
        };
        selected = declaration(child)?;
    }
    let mut complete = path.to_vec();
    if let Some(identity) = field {
        let (terminal_psi::record_or_mixed_shape_pattern!(fields)) = &selected.shape else {
            return None;
        };
        let mut matches = fields.iter().filter(|field| field.id == identity);
        let field = matches.next()?;
        if matches.next().is_some() || field.relevance.is_erased() {
            return None;
        }
        let child = field.field_type.as_structural()?;
        selected = declaration(child)?;
        complete.push(StructuralPathSegment::Field(field.identity.clone()));
    }
    Some(StructuralWindowProjection {
        path: complete,
        structural_type: selected.id,
    })
}
