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
        match view {
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
                                    && parameter.access
                                        == terminal_psi::StructuralAccess::SharedBorrow
                                    && parameter.multiplicity
                                        == terminal_psi::StructuralMultiplicity::Unrestricted
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
                                    && parameter.access
                                        == terminal_psi::StructuralAccess::SharedBorrow
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
                source_byte_offset: _,
                extent,
                element_stride,
                source: established_source,
            } => {
                let TargetStructuralArgumentSource::Placement(placement) = established_source
                else {
                    return false;
                };
                let Some(argument) = self
                    .optimized
                    .blocks
                    .iter()
                    .flat_map(|block| &block.nodes)
                    .find_map(|node| {
                        let AbstractOperation::EstablishElementView {
                            psi_operation: operation,
                            result,
                            destination,
                            source: argument,
                            element: expected_element,
                        } = &node.operation
                        else {
                            return None;
                        };
                        (operation == psi_operation
                            && result.structural_type == *structural_type
                            && *destination == *place
                            && *expected_element == *element)
                            .then_some(argument)
                    })
                else {
                    return false;
                };
                *place == source
                    && argument.access == terminal_psi::StructuralAccess::SharedBorrow
                    && argument.path.iter().all(|segment| {
                        matches!(segment, terminal_psi::StructuralPathSegment::Field(_))
                    })
                    && self
                        .optimized
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
                    && self.established_element_window(
                        *root_structural_type,
                        &argument.path,
                        *element,
                        *extent,
                        *element_stride,
                    )
            }
            TargetElementView::Subslice {
                psi_operation,
                place,
                source: original,
                start,
                end,
                length,
                obligation,
            } => {
                *place == source
                    && self
                        .optimized
                        .blocks
                        .iter()
                        .flat_map(|block| &block.nodes)
                        .any(|node| {
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
                                && length == expected_length
                                && obligation == expected_obligation
                                && self.element_view(original, *expected_source, aliases)
                                && self.integer_source(start, *expected_start, aliases)
                                && self.integer_source(end, *expected_end, aliases)
                        })
            }
        }
    }

    /// Re-derive an established view's declared window: the runtime path tip
    /// of the authored field chain is a fixed array whose element and length
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
        extent: u64,
        element_stride: u32,
    ) -> bool {
        let Some(tip) = terminal_semantics::runtime_structural_path_tip(
            self.types.iter(),
            root_structural_type,
            path,
        ) else {
            return false;
        };
        let StructuralTypeShape::FixedArray {
            element: array_element,
            length,
        } = &tip.shape
        else {
            return false;
        };
        if *array_element != element || *length != extent {
            return false;
        }
        crate::structural_inputs::structural_reference_input::shape(element, self.types)
            .and_then(|shape| {
                crate::structural_inputs::structural_reference_input::align(
                    u32::from(shape.byte_size),
                    shape.alignment,
                )
            })
            .is_some_and(|stride| stride != 0 && stride == element_stride)
    }
}
