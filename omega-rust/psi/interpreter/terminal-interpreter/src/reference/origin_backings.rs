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
            array_loan: backing.array_loan.clone(),
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
                let Some(projected) =
                    self.resolve_origin_projection(&selected, &prefix, backing.access)?
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
            if matches!(
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
                let interpretation = match selected.view_tip {
                    Some(AggregateViewTip::Aggregate { view, .. }) => Some(view),
                    _ => None,
                };
                let Some(projected) = self.resolve_captured_element_origin(
                    view,
                    index,
                    backing.access,
                    interpretation,
                    selected.placed_ram.clone(),
                )?
                else {
                    return Ok(None);
                };
                selected = projected;
                captured_view = None;
                remaining = &remaining[position + 1..];
                continue;
            }
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
            let projection = vec![StructuralPathSegment::FixedIndex(*index)];
            let Some(projected) =
                self.resolve_origin_projection(&selected, &projection, backing.access)?
            else {
                return Ok(None);
            };
            selected = projected;
            captured_view = None;
            remaining = &remaining[position + 1..];
        }
    }

    /// Continue within an already authenticated backing. An empty suffix must
    /// preserve its endpoint and codec. Inline children keep the supplying
    /// activation home; a descriptor crossing uses that descriptor's own capture.
    fn resolve_origin_projection(
        &self,
        source: &ResolvedStructuralProjection,
        path: &[StructuralPathSegment],
        access: StructuralAccess,
    ) -> Result<Option<ResolvedStructuralProjection>, TerminalInterpretError> {
        if path.is_empty() {
            return Ok(Some(source.clone()));
        }
        let interpretation = match source.view_tip {
            Some(AggregateViewTip::Aggregate { view, .. }) => Some(view),
            _ => None,
        };
        let Some(mut projected) = self.resolve_reference_view_path_with_leaf_type(
            &source.value,
            path,
            Some(access),
            terminal_psi::StructuralFieldType::canonical_leaf_type,
            interpretation,
            source.placed_ram.clone(),
            source.array_loan.clone(),
        )?
        else {
            return Ok(None);
        };
        if let Some(home) = &source.collection_home {
            let types = self.program.structural_types();
            if terminal_semantics::reference_storage_region(
                types.values(),
                source.value.structural_type,
                path,
            )
            .is_some()
            {
                let mut selected_home = home.clone();
                selected_home.path.extend_from_slice(path);
                match types
                    .get(&projected.value.structural_type)
                    .map(|declaration| &declaration.shape)
                {
                    Some(StructuralTypeShape::PrimitiveScalar(_))
                        if projected.primitive_storage.is_none() =>
                    {
                        projected.primitive_storage = Some(
                            self.primitive_home_access(
                                selected_home.storage,
                                access != StructuralAccess::SharedBorrow,
                                &selected_home.path,
                            )?
                            .0,
                        );
                    }
                    Some(StructuralTypeShape::FixedArray { length, .. })
                        if projected.array_loan.is_none() =>
                    {
                        projected.array_loan = Some(
                            crate::element_views::ElementView::structural(
                                projected.value.clone(),
                                usize::try_from(*length).map_err(|_| malformed())?,
                                access,
                                Some(selected_home.clone()),
                                selected_home.storage.scalar_array_frame().1,
                            )
                            .with_descriptor_home(projected.value.clone()),
                        );
                    }
                    _ => {}
                }
                projected.collection_home = Some(selected_home);
            }
        }
        Ok(Some(projected))
    }

    /// Geometry selects the original collection with a physical index; storage
    /// lookup consumes the logical view index. Applying the narrowed companion
    /// to that physical index again would add its start twice. Both operations
    /// use this exact captured view, preserving its activation, access and codec.
    fn resolve_captured_element_origin(
        &self,
        view: &crate::element_views::ElementView,
        index: u64,
        access: StructuralAccess,
        interpretation: Option<AggregateViewRegion>,
        placed: Option<PlacedRamReference>,
    ) -> Result<Option<ResolvedStructuralProjection>, TerminalInterpretError> {
        let types = self.program.structural_types();
        let logical_path = [StructuralPathSegment::FixedIndex(index)];
        let (collection, physical_path) = view.structural_projection(
            &types,
            &logical_path,
            access != StructuralAccess::SharedBorrow,
        )?;
        let Some(mut projected) = self.resolve_reference_view_path_with_leaf_type(
            &collection,
            &physical_path,
            Some(access),
            terminal_psi::StructuralFieldType::canonical_leaf_type,
            interpretation,
            placed,
            None,
        )?
        else {
            return Ok(None);
        };
        match types
            .get(&projected.value.structural_type)
            .map(|declaration| &declaration.shape)
        {
            Some(StructuralTypeShape::PrimitiveScalar(_))
                if projected.primitive_storage.is_none() =>
            {
                projected.primitive_storage = Some(
                    view.scalar_storage(
                        self,
                        &logical_path,
                        access != StructuralAccess::SharedBorrow,
                    )?
                    .0,
                );
            }
            Some(StructuralTypeShape::FixedArray { .. }) => {
                projected.array_loan =
                    Some(view.project_array(&types, &logical_path, &projected.value)?);
            }
            _ => {}
        }
        projected.collection_home = view.captured_storage_home(&types, &logical_path)?;
        Ok(Some(projected))
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
            let Some(element) = self.resolve_captured_element_origin(
                view,
                index,
                access,
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
        let Some(mut projected) = self.resolve_origin_projection(&region, path, access)? else {
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
