//! Returned provenance is matched against captured ingress, never by enumerating
//! a slice or evaluating a callee selector again. A returned coordinate proposes
//! an element; the captured window, typed projection and final backing equality
//! authenticate it. This does not form a loan or change its parent.

use super::*;
use terminal_psi::ReferenceOriginProjection;

impl TerminalExecution<'_> {
    pub(super) fn resolve_return_origin(
        &self,
        backing: &ReferenceBacking,
        actual: &ReferenceReferent,
    ) -> Result<Option<ResolvedStructuralProjection>, TerminalInterpretError> {
        let types = self.program.structural_types();
        let mut selected = ResolvedStructuralProjection {
            value: backing.value.clone(),
            view_tip: backing
                .aggregate_view
                .map(|view| AggregateViewTip::Aggregate {
                    view,
                    backing: backing.value.structural_type,
                }),
            endpoint_field: None,
            primitive_storage: backing.primitive_storage.clone(),
        };
        let mut remaining = backing.suffix.as_slice();
        let mut captured_view = backing.element_view.as_ref();
        loop {
            let family = remaining
                .iter()
                .position(|projection| *projection == ReferenceOriginProjection::Element);
            let prefix_length = family.unwrap_or(remaining.len());
            let prefix = remaining[..prefix_length]
                .iter()
                .map(|projection| match projection {
                    ReferenceOriginProjection::Exact(segment) => Ok(segment.clone()),
                    ReferenceOriginProjection::Element => Err(malformed()),
                })
                .collect::<Result<Vec<_>, _>>()?;
            if !prefix.is_empty() {
                let view = match selected.view_tip {
                    Some(AggregateViewTip::Aggregate { view, .. }) => Some(view),
                    _ => None,
                };
                let Some(projected) = self.resolve_reference_view_path_with_leaf_type(
                    &selected.value,
                    &prefix,
                    Some(backing.access),
                    terminal_psi::StructuralFieldType::canonical_leaf_type,
                    view,
                )?
                else {
                    return Ok(None);
                };
                selected = projected;
                captured_view = None;
            }
            let Some(position) = family else {
                return Ok(Some(selected));
            };
            let logical_type = match selected.view_tip {
                Some(AggregateViewTip::Aggregate { view, .. }) => view.structural_type,
                _ => selected.value.structural_type,
            };
            let (collection, projection) = if matches!(
                types
                    .get(&logical_type)
                    .map(|declaration| &declaration.shape),
                Some(StructuralTypeShape::ElementView { .. })
            ) {
                let view = match captured_view {
                    Some(view) => view,
                    None => self.element_view_at_resolved_home(&selected.value)?,
                };
                if !access_covers(view.access(), backing.access) {
                    return Err(malformed());
                }
                let Some(index) = view.captured_structural_index(&actual.view) else {
                    return Ok(None);
                };
                view.structural_projection(
                    &[StructuralPathSegment::FixedIndex(index)],
                    backing.access != StructuralAccess::SharedBorrow,
                )?
            } else {
                if selected.value.opaque_identity != actual.view.opaque_identity {
                    return Ok(None);
                }
                let Some(suffix) = actual
                    .view
                    .path
                    .strip_prefix(selected.value.path.as_slice())
                else {
                    return Ok(None);
                };
                let Some(StructuralPathSegment::FixedIndex(index)) = suffix.first() else {
                    return Ok(None);
                };
                (
                    selected.value.clone(),
                    vec![StructuralPathSegment::FixedIndex(*index)],
                )
            };
            let view = match selected.view_tip {
                Some(AggregateViewTip::Aggregate { view, .. }) => Some(view),
                _ => None,
            };
            let Some(projected) = self.resolve_reference_view_path_with_leaf_type(
                &collection,
                &projection,
                Some(backing.access),
                terminal_psi::StructuralFieldType::canonical_leaf_type,
                view,
            )?
            else {
                return Ok(None);
            };
            selected = projected;
            captured_view = None;
            remaining = &remaining[position + 1..];
        }
    }
}
