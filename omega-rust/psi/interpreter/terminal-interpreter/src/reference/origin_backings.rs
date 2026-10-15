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
            placed_ram: backing.placed_ram.clone(),
            // Traverse suffixes in the captured logical interpretation.
            // Original supplying storage remains private in the association.
            value: backing
                .placed_ram
                .as_ref()
                .map_or_else(|| backing.value.clone(), |ram| ram.presentation.clone()),
            view_tip: backing
                .aggregate_view
                .map(|view| AggregateViewTip::Aggregate {
                    view,
                    backing: backing.value.structural_type,
                }),
            endpoint_field: None,
            primitive_storage: backing.primitive_storage.clone(),
            collection_home: None,
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
                    selected.placed_ram.clone(),
                )?
                else {
                    return Ok(None);
                };
                selected = projected;
                captured_view = None;
            }
            let Some(position) = family else {
                if backing.extent == semantic_vocabulary::ReferenceOriginExtent::WithinRegion {
                    return self.resolve_region_return(
                        selected,
                        captured_view,
                        actual,
                        backing.access,
                    );
                }
                if let Some(ram) = &selected.placed_ram {
                    self.validate_placed_ram_home(ram)?;
                    selected.value = ram.source.clone();
                }
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
                    &types,
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
                selected.placed_ram.clone(),
            )?
            else {
                return Ok(None);
            };
            selected = projected;
            captured_view = None;
            remaining = &remaining[position + 1..];
        }
    }

    /// Narrow a captured region using the returned descriptor's physical
    /// coordinates, never an expression or selector in the callee namespace.
    /// The typed projection authenticates the complete child footprint and
    /// stored codec. No stored pointer may be crossed to leave this region.
    fn resolve_region_return(
        &self,
        mut region: ResolvedStructuralProjection,
        captured_view: Option<&crate::element_views::ElementView>,
        actual: &ReferenceReferent,
        access: StructuralAccess,
    ) -> Result<Option<ResolvedStructuralProjection>, TerminalInterpretError> {
        if let Some(view) = captured_view {
            if !access_covers(view.access(), access) {
                return Err(malformed());
            }
            let Some(index) = view.captured_structural_index(&actual.view) else {
                return Ok(None);
            };
            let (collection, path) = view.structural_projection(
                &self.program.structural_types(),
                &[StructuralPathSegment::FixedIndex(index)],
                access != StructuralAccess::SharedBorrow,
            )?;
            let Some(element) = self.resolve_reference_view_path_with_leaf_type(
                &collection,
                &path,
                Some(access),
                terminal_psi::StructuralFieldType::canonical_leaf_type,
                None,
                region.placed_ram.clone(),
            )?
            else {
                return Ok(None);
            };
            region = element;
        }
        if region.value.opaque_identity != actual.view.opaque_identity {
            return Ok(None);
        }
        let Some(path) = actual.view.path.strip_prefix(region.value.path.as_slice()) else {
            return Ok(None);
        };
        // These are physical owned coordinates. An implicit view index or a
        // stored descriptor crossing is not an inline storage projection.
        let types = self.program.structural_types();
        let Some(selected) = terminal_semantics::reference_storage_region(
            types.values(),
            region.value.structural_type,
            path,
        ) else {
            return Ok(None);
        };
        let allowed = match region.view_tip {
            Some(AggregateViewTip::Aggregate { view, .. }) => view,
            _ => match AggregateViewRegion::whole(&types, region.value.structural_type) {
                Some(whole) => whole,
                None => return Ok(None),
            },
        };
        let (offset, length) = match actual.aggregate_view {
            Some(view) if view.fits(&types, actual.view.structural_type) => {
                let Some(offset) = selected.byte_offset.checked_add(view.byte_offset) else {
                    return Ok(None);
                };
                (offset, view.byte_length)
            }
            Some(_) => return Ok(None),
            None => (selected.byte_offset, selected.byte_length),
        };
        if offset < allowed.byte_offset
            || offset
                .checked_add(length)
                .zip(allowed.byte_offset.checked_add(allowed.byte_length))
                .is_none_or(|(end, allowed_end)| end > allowed_end)
        {
            return Ok(None);
        }
        // Byte-backed descriptors must retain their original byte endpoint as
        // well as this typed footprint. That association is not captured by a
        // plain structural region, so it cannot be inferred from opaque IDs.
        if actual.byte_region.is_some() {
            return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                "returned input region requires captured byte backing",
            ));
        }
        let Some(mut projected) = self.resolve_reference_view_path_with_leaf_type(
            &region.value,
            path,
            Some(access),
            terminal_psi::StructuralFieldType::canonical_leaf_type,
            None,
            region.placed_ram.clone(),
        )?
        else {
            return Ok(None);
        };
        if let Some(view) = actual.aggregate_view {
            projected.view_tip = Some(AggregateViewTip::Aggregate {
                view,
                backing: projected.value.structural_type,
            });
        }
        Ok(Some(projected))
    }
}
