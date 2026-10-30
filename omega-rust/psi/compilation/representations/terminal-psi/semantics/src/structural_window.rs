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
/// Value qualifications remain the receiving store's restoration obligations;
/// they do not remove the origin's independent storage permission.
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
    // Share exact field/case/reference selection with the ordinary projection
    // grammar. A final field ID is relative to the current payload namespace,
    // never an invented record for that case.
    let mut complete = path.to_vec();
    if let Some(identity) = field {
        let field = crate::projected_field_declaration(types.clone(), root, path, identity)?;
        field.field_type.as_structural()?;
        complete.push(StructuralPathSegment::Field(field.identity));
    }
    let structural_type = crate::primitive_place::projection_tip_with_policy(
        types.clone(),
        root,
        &complete,
        |current, segment| match segment {
            StructuralPathSegment::Field(_)
            | StructuralPathSegment::Case(_)
            | StructuralPathSegment::Referent => true,
            // Preserve window selection's existing collection policy. Dynamic
            // extents require their runtime index and its separate bound proof.
            StructuralPathSegment::FixedIndex(_) => current.is_some_and(|current| {
                types.clone().any(|declaration| {
                    declaration.id == current
                        && matches!(
                            declaration.shape,
                            StructuralTypeShape::FixedArray {
                                length: terminal_psi::ArrayExtent::Literal(_),
                                ..
                            }
                        )
                })
            }),
            StructuralPathSegment::RuntimeIndex { .. } => current.is_some_and(|current| {
                types.clone().any(|declaration| {
                    declaration.id == current
                        && matches!(
                            declaration.shape,
                            StructuralTypeShape::FixedArray { .. }
                                | StructuralTypeShape::ElementView { .. }
                        )
                })
            }),
            _ => false,
        },
    )??;
    Some(StructuralWindowProjection {
        path: complete,
        structural_type,
    })
}
