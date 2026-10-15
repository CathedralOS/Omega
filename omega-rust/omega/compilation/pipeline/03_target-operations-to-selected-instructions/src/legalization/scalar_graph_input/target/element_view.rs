//! Independent custody correspondence for element descriptor derivations.
use super::Checker;
use abstract_operations::AbstractOperation;
use semantic_vocabulary::{PlaceId, ValueId};
use target_operations::{TargetElementView, TargetStructuralArgumentSource};
use terminal_psi::StructuralTypeShape;

impl Checker<'_> {
    pub(super) fn element_view(
        &self,
        view: &TargetElementView,
        source: PlaceId,
        aliases: &[(ValueId, ValueId)],
    ) -> bool {
        self.element_view_access(
            view,
            source,
            aliases,
            terminal_psi::StructuralAccess::SharedBorrow,
        )
    }

    /// A mutable element view admits only the exclusive loan — an indexed
    /// element store's destination may never name a shared descriptor.
    pub(super) fn mutable_element_view(&self, view: &TargetElementView, source: PlaceId) -> bool {
        self.element_view_access(
            view,
            source,
            &[],
            terminal_psi::StructuralAccess::MutableBorrow,
        )
    }

    fn element_view_access(
        &self,
        view: &TargetElementView,
        source: PlaceId,
        aliases: &[(ValueId, ValueId)],
        requested: terminal_psi::StructuralAccess,
    ) -> bool {
        use terminal_psi::StructuralAccess as Access;
        let Some(actual) =
            super::super::byte_views::descriptor_result(self.optimized, source, self.types)
                .map(|_| Access::SharedBorrow)
                .or_else(|| {
                    optimization_unit_semantics::structural_place_access(self.optimized, source)
                })
        else {
            return false;
        };
        if !matches!(
            (actual, requested),
            (
                Access::SharedBorrow | Access::MutableBorrow,
                Access::SharedBorrow
            ) | (Access::MutableBorrow, Access::MutableBorrow)
        ) {
            return false;
        }
        let access = |access| access == actual;
        match view {
            TargetElementView::Result {
                psi_operation,
                place,
                structural_type,
            } => {
                *place == source
                    && super::super::byte_views::descriptor_result(
                        self.optimized,
                        source,
                        self.types,
                    ) == Some((*psi_operation, *structural_type))
            }
            TargetElementView::BlockParameter {
                block,
                place,
                structural_type,
            } => {
                *place == source
                    && self.optimized.blocks.iter().any(|owner| {
                        owner.id == *block
                            && owner.id != self.optimized.entry
                            && owner.structural_parameters.iter().any(|parameter| {
                                parameter.place == *place
                                    && parameter.structural_type == *structural_type
                                    && access(parameter.access)
                                    && terminal_semantics::view_descriptor_parameter(
                                        parameter,
                                        self.types.iter(),
                                    )
                                    && parameter.qualifications.is_empty()
                                    && parameter.projected_qualifications.is_empty()
                            })
                    })
            }
            TargetElementView::Parameter { place, placement } => {
                *place == source
                    && super::super::structural_parameters(self.function).is_some_and(
                        |parameters| {
                            parameters.iter().any(|parameter| {
                                parameter.place == source
                                    && access(parameter.access)
                                    && &parameter.placement == placement
                            })
                        },
                    )
            }
            TargetElementView::Established {
                psi_operation,
                place,
                structural_type,
                element,
                root_structural_type,
                source_byte_offset,
                indices,
                extent,
                region,
                element_stride,
                source: established_source,
            } => {
                let Some((argument, expected_region)) =
                    self.optimized.block_nodes().find_map(|node| {
                        let AbstractOperation::EstablishElementView {
                            psi_operation: operation,
                            result,
                            destination,
                            source: argument,
                            element: expected_element,
                            region: expected_region,
                        } = &node.operation
                        else {
                            return None;
                        };
                        (operation == psi_operation
                            && result.structural_type == *structural_type
                            // Descriptor custody belongs to its establishment,
                            // even when no later operation observes the view.
                            // The surrounding function's ABI grants no result
                            // evidence or ownership authority.
                            && matches!(result.multiplicity,
                                terminal_psi::StructuralMultiplicity::Unrestricted
                                    | terminal_psi::StructuralMultiplicity::Affine)
                            && result.qualifications.is_empty()
                            && result.projected_qualifications.is_empty()
                            && result.claims.is_empty()
                            && *destination == *place
                            && *expected_element == *element)
                            .then_some((argument, *expected_region))
                    })
                else {
                    return false;
                };
                let backing = match established_source {
                    // An incoming parameter lends its exact entrance placement.
                    TargetStructuralArgumentSource::Placement(placement) => {
                        self.optimized
                            .structural_parameters
                            .iter()
                            .any(|parameter| {
                                parameter.place == argument.place
                                    && parameter.structural_type == *root_structural_type
                            })
                            && super::super::structural_parameters(self.function).is_some_and(
                                |parameters| {
                                    parameters.iter().any(|parameter| {
                                        parameter.place == argument.place
                                            && &parameter.placement == placement
                                    })
                                },
                            )
                    }
                    // A local aggregate lends the home its one producer
                    // established before this view.
                    TargetStructuralArgumentSource::StructuralHome {
                        psi_operation: producer,
                    } => self.local_home_precedes(
                        *producer,
                        *psi_operation,
                        argument.place,
                        *root_structural_type,
                    ),
                    TargetStructuralArgumentSource::BlockParameter { block, place } => {
                        *place == argument.place
                            && self.block_backing_precedes(
                                *block,
                                *place,
                                *psi_operation,
                                *root_structural_type,
                                argument.access,
                            )
                    }
                    _ => false,
                };
                *place == source
                    && access(argument.access)
                    && backing
                    && match (region, expected_region) {
                        (
                            target_operations::TargetElementViewRegion::WholeCollection,
                            terminal_psi::ElementViewRegion::WholeCollection,
                        ) => true,
                        (
                            target_operations::TargetElementViewRegion::RepresentationBytes {
                                start,
                                end,
                                obligation,
                            },
                            terminal_psi::ElementViewRegion::RepresentationBytes {
                                start: expected_start,
                                end: expected_end,
                                obligation: expected_obligation,
                            },
                        ) => {
                            *obligation == expected_obligation
                                && self.integer_source(start, expected_start, aliases)
                                && self.integer_source(end, expected_end, aliases)
                        }
                        _ => false,
                    }
                    && self.established_element_window(
                        *root_structural_type,
                        &argument.path,
                        *element,
                        *source_byte_offset,
                        indices,
                        *extent,
                        *element_stride,
                        expected_region,
                        argument.access,
                    )
            }
            TargetElementView::Subslice {
                psi_operation,
                place,
                source: original,
                indices,
                start,
                end,
                length,
                obligation,
            } => {
                *place == source
                    && self.optimized.block_nodes().any(|node| {
                        let AbstractOperation::ElementViewSubslice {
                            psi_operation: operation,
                            result,
                            source: expected_source,
                            start: expected_start,
                            end: expected_end,
                            length: expected_length,
                            obligation: expected_obligation,
                        } = &node.operation
                        else {
                            return false;
                        };
                        operation == psi_operation
                        && result.place == source
                        && matches!(
                            (actual, result.multiplicity),
                            (
                                Access::SharedBorrow,
                                terminal_psi::StructuralMultiplicity::Unrestricted
                            ) | (
                                Access::MutableBorrow,
                                terminal_psi::StructuralMultiplicity::Affine
                            )
                        )
                        && result.qualifications.is_empty()
                        && result.projected_qualifications.is_empty()
                        && result.claims.is_empty()
                        && self.types.iter().any(|declaration| {
                            declaration.id == result.structural_type
                                && matches!(
                                    declaration.shape,
                                    StructuralTypeShape::ElementView { .. }
                                        | StructuralTypeShape::ByteSequence(
                                            terminal_psi::ByteSequenceCarrier::BorrowedView { .. }
                                        )
                                )
                        })
                        && length == expected_length
                        && obligation == expected_obligation
                        && original == expected_source
                        && super::super::element_views::descriptor_geometry(
                            self.optimized,
                            original,
                            self.types,
                        )
                        .is_some_and(|geometry| {
                            geometry.readable
                                && super::unit::runtime_indices_rejoin(
                                    indices,
                                    &geometry.elements,
                                    self.available.unwrap_or(&[]),
                                )
                        })
                        && self.integer_source(start, *expected_start, aliases)
                        && self.integer_source(end, *expected_end, aliases)
                    })
            }
        }
    }

    /// An owned arrival lends its storage; an address join lends the referent
    /// its incoming edges authenticated. Preserve the exact declaration and
    /// dominating block, so another same-typed carrier cannot substitute for it.
    fn block_backing_precedes(
        &self,
        block: semantic_vocabulary::BlockId,
        place: PlaceId,
        consumer: semantic_vocabulary::OperationId,
        root_structural_type: semantic_vocabulary::StructuralTypeId,
        requested: terminal_psi::StructuralAccess,
    ) -> bool {
        let Some(parameter) = self
            .optimized
            .block(block)
            .and_then(|entry| entry.structural_parameter(place))
        else {
            return false;
        };
        let Some((consumer_block, _)) =
            crate::legalization::scalar_graph_input::aggregate_results::node_site(
                self.optimized,
                |node| node.psi_operation() == Some(consumer),
            )
        else {
            return false;
        };
        // A block-carried borrowed descriptor backs the collection it names:
        // the edge already transported the pointer whose referent carries the
        // view's storage, the same admission an address join receives for a
        // plain referent.
        let descriptor_carrier = self.types.iter().any(|declaration| {
            declaration.id == parameter.structural_type
                && declaration.shape.is_borrowed_byte_carrier()
        });
        let backing = parameter.access == terminal_psi::StructuralAccess::Owned
            || (block != self.optimized.entry
                && (abstract_operations::control_flow::address_joins::is_address_join_in(
                    parameter, self.types,
                ) || descriptor_carrier)
                && matches!(
                    (parameter.access, requested),
                    (
                        terminal_psi::StructuralAccess::SharedBorrow,
                        terminal_psi::StructuralAccess::SharedBorrow,
                    ) | (
                        terminal_psi::StructuralAccess::MutableBorrow,
                        terminal_psi::StructuralAccess::SharedBorrow
                            | terminal_psi::StructuralAccess::MutableBorrow,
                    )
                ));
        backing
            && parameter.structural_type == root_structural_type
            && parameter.multiplicity != terminal_psi::StructuralMultiplicity::Linear
            && parameter.projected_qualifications.is_empty()
            && self.optimized.structural_places.iter().any(|declaration| {
                declaration.id == place
                    && declaration.kind
                        == semantic_vocabulary::StructuralPlaceKind::BlockParameter {
                            block,
                            position: parameter.position,
                        }
            })
            && super::control_flow::sources::dominates(self.optimized, block, consumer_block)
    }

    /// The local aggregate at `place` is the owned, non-linear, claim-free
    /// result of exactly `producer`, which runs before
    /// `consumer`: earlier in the consumer's block or in a dominating block.
    fn local_home_precedes(
        &self,
        producer: semantic_vocabulary::OperationId,
        consumer: semantic_vocabulary::OperationId,
        place: PlaceId,
        root_structural_type: semantic_vocabulary::StructuralTypeId,
    ) -> bool {
        let Ok((actual, result)) =
            crate::legalization::scalar_graph_input::structural_case::source_result(
                self.optimized,
                place,
            )
        else {
            return false;
        };
        let site = |operation: semantic_vocabulary::OperationId| {
            crate::legalization::scalar_graph_input::aggregate_results::node_site(
                self.optimized,
                |node| node.psi_operation() == Some(operation),
            )
        };
        let (Some(before), Some(after)) = (site(producer), site(consumer)) else {
            return false;
        };
        actual == producer
            && result.structural_type == root_structural_type
            && matches!(
                result.multiplicity,
                terminal_psi::StructuralMultiplicity::Unrestricted
                    | terminal_psi::StructuralMultiplicity::Affine
            )
            && result.projected_qualifications.is_empty()
            && result.claims.is_empty()
            && self.optimized.structural_places.iter().any(|declaration| {
                declaration.id == place
                    && declaration.kind
                        == semantic_vocabulary::StructuralPlaceKind::OperationResult {
                            producer,
                            structural_type: result.structural_type,
                        }
            })
            && if before.0 == after.0 {
                before.1 < after.1
            } else {
                super::control_flow::sources::dominates(self.optimized, before.0, after.0)
            }
    }

    /// Re-derive an established view's declared window: the runtime path tip
    /// of the canonical projection is a fixed array whose element and length
    /// witness the descriptor extent, and the element's own layout stride
    /// (its size rounded up to its alignment, the same geometry a fixed array
    /// of that element occupies) is the transported stride. Record elements
    /// take the same rule as primitive scalars: a view never reads padding as
    /// an element, because consecutive elements sit exactly one stride apart.
    fn established_element_window(
        &self,
        root_structural_type: semantic_vocabulary::StructuralTypeId,
        path: &[terminal_psi::StructuralPathSegment],
        element: semantic_vocabulary::StructuralTypeId,
        source_byte_offset: u32,
        indices: &[target_operations::TargetStructuralRuntimeIndex],
        extent: target_operations::TargetViewExtent,
        element_stride: u32,
        region: terminal_psi::ElementViewRegion,
        access: terminal_psi::StructuralAccess,
    ) -> bool {
        // A borrowed byte view carries a `FixedByteRange` window over the
        // fixed array its collection path names: the window bounds the array and
        // its width is the descriptor's extent.
        let (collection_path, byte_window) = match path.last() {
            Some(terminal_psi::StructuralPathSegment::FixedByteRange { start, end }) => {
                (&path[..path.len() - 1], Some((*start, *end)))
            }
            _ => (path, None),
        };
        // Replay the selected carrier and offset, not an invented standalone
        // type for an inline field. Copy presentation does not supply identity.
        let Some((collection, offset, elements)) =
            crate::structural_inputs::structural_reference_input::collection_projection(
                root_structural_type,
                collection_path,
                self.types,
            )
        else {
            return false;
        };
        if !super::unit::runtime_indices_rejoin(indices, &elements, self.available.unwrap_or(&[])) {
            return false;
        }
        let Some(geometry) = terminal_semantics::element_view_shape_region_geometry(
            self.types.iter(),
            &collection,
            element,
            region,
            access,
        ) else {
            return false;
        };
        if region == terminal_psi::ElementViewRegion::WholeCollection
            && !crate::structural_inputs::structural_reference_input::typed_collection_layout_matches(
                &collection, element, access, self.types,
            )
        { return false; }
        let backing =
            match geometry {
                terminal_semantics::ElementViewRegionGeometry::WholeCollection(backing) => backing,
                terminal_semantics::ElementViewRegionGeometry::RepresentationBytes {
                    byte_extent,
                    element: layout,
                } => {
                    if byte_window.is_some()
                        || layout.size != u64::from(element_stride)
                        || crate::structural_inputs::structural_reference_input::shape(
                            element, self.types,
                        )
                        .is_none_or(|shape| {
                            u64::from(shape.byte_size) != layout.size
                                || u64::from(shape.alignment) != layout.alignment
                        }) || layout.leaves.iter().any(|leaf| {
                        crate::structural_inputs::structural_reference_input::element_leaf_geometry(
                            element,
                            &leaf.path,
                            leaf.scalar_type,
                            self.types,
                        )
                        .is_none_or(|(offset, _)| u64::from(offset) != leaf.offset)
                    }) {
                        return false;
                    }
                    return extent == target_operations::TargetViewExtent::Fixed(byte_extent)
                        && offset == source_byte_offset;
                }
            };
        if backing.storage == terminal_semantics::CollectionStorage::Borrowed
            && byte_window.is_some()
        {
            return false;
        }
        match (byte_window, backing.extent) {
            (Some((start, end)), terminal_semantics::CollectionExtent::Fixed(length)) => {
                if !(start <= end && end <= length)
                    || extent != target_operations::TargetViewExtent::Fixed(end - start)
                {
                    return false;
                }
            }
            // A live carrier's count is dynamic; a static window cannot
            // bound it at establishment.
            (Some(_), _) => return false,
            (None, terminal_semantics::CollectionExtent::Fixed(length)) => {
                if extent != target_operations::TargetViewExtent::Fixed(length) {
                    return false;
                }
            }
            (None, terminal_semantics::CollectionExtent::LiveLength { .. }) => {
                if extent != target_operations::TargetViewExtent::LiveLength {
                    return false;
                }
            }
            (None, terminal_semantics::CollectionExtent::StoredLength) => {
                if extent != target_operations::TargetViewExtent::StoredLength {
                    return false;
                }
            }
        }
        let Some(stride) =
            crate::structural_inputs::structural_reference_input::shape(element, self.types)
                .and_then(|shape| {
                    crate::structural_inputs::structural_reference_input::align(
                        shape.byte_size,
                        shape.alignment,
                    )
                })
        else {
            return false;
        };
        let window_start = byte_window.map_or(0, |(start, _)| start);
        // An owned byte carrier's live-length word precedes its viewed
        // elements; a fixed collection names its first element directly.
        let leading: u64 = match backing.extent {
            terminal_semantics::CollectionExtent::LiveLength { .. } => 8,
            terminal_semantics::CollectionExtent::Fixed(_)
            | terminal_semantics::CollectionExtent::StoredLength => 0,
        };
        stride != 0
            && stride == element_stride
            && window_start
                .checked_mul(u64::from(stride))
                .and_then(|window_offset| {
                    u64::from(offset)
                        .checked_add(leading)
                        .and_then(|bytes| bytes.checked_add(window_offset))
                })
                == Some(u64::from(source_byte_offset))
    }
}
