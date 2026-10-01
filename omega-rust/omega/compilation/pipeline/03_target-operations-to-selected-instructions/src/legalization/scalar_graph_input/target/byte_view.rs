//! Independent correspondence for native descriptor derivations.
use super::Checker;
use abstract_operations::AbstractOperation;
use semantic_vocabulary::{PlaceId, ValueId};
use target_operations::TargetByteView;

impl Checker<'_> {
    pub(super) fn byte_view(
        &self,
        view: &TargetByteView,
        source: PlaceId,
        aliases: &[(ValueId, ValueId)],
    ) -> bool {
        self.byte_view_access(
            view,
            source,
            aliases,
            terminal_psi::StructuralAccess::SharedBorrow,
        )
    }

    pub(super) fn mutable_byte_view(&self, view: &TargetByteView, source: PlaceId) -> bool {
        self.byte_view_access(
            view,
            source,
            &[],
            terminal_psi::StructuralAccess::MutableBorrow,
        )
    }

    fn byte_view_access(
        &self,
        view: &TargetByteView,
        source: PlaceId,
        aliases: &[(ValueId, ValueId)],
        access: terminal_psi::StructuralAccess,
    ) -> bool {
        match view {
            TargetByteView::Owned {
                psi_operation,
                place,
                structural_type,
            } => {
                matches!(access, terminal_psi::StructuralAccess::SharedBorrow | terminal_psi::StructuralAccess::MutableBorrow)
                    && *place == source
                    && super::super::byte_views::owned_byte_result(self.optimized, source, self.types)
                        == Some((*psi_operation, *structural_type))
                    && self.types.iter().any(|declaration| {
                        declaration.id == *structural_type
                            && matches!(
                                declaration.shape,
                                terminal_psi::StructuralTypeShape::ByteSequence(
                                    terminal_psi::ByteSequenceCarrier::BoundedOwned { .. }
                                )
                            )
                    })
            }
            TargetByteView::Literal {
                psi_operation,
                place,
                structural_type,
            } => {
                access == terminal_psi::StructuralAccess::SharedBorrow
                    && *place == source
                    && (super::super::literals::declaration_producer(self.optimized, source)
                        == Some((*psi_operation, *structural_type))
                        || self.established_byte_view(*psi_operation, source, *structural_type))
            }
            TargetByteView::BlockParameter {
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
                                    && parameter.access == access
                                    && parameter.multiplicity
                                        == terminal_psi::StructuralMultiplicity::Unrestricted
                                    && ((parameter.qualifications.is_empty()
                                        && parameter.projected_qualifications.is_empty())
                                        || abstract_operations::control_flow::owner_loans::is_shared_bounded_owner_loan_in(
                                            parameter, self.types,
                                        )
                                        || abstract_operations::control_flow::owner_loans::is_shared_byte_view_in(
                                            parameter, self.types,
                                        ))
                            })
                    })
            }
            TargetByteView::Parameter { place, placement } => {
                *place == source
                    && super::super::structural_parameters(self.function).is_some_and(
                        |parameters| {
                            parameters.iter().any(|parameter| {
                                parameter.place == source
                                    && (parameter.access == access
                                        || (access == terminal_psi::StructuralAccess::SharedBorrow
                                            && parameter.access == terminal_psi::StructuralAccess::Owned
                                            && super::super::byte_views::owned_parameter(
                                                self.optimized, source, self.types,
                                            ).is_some_and(|semantic|
                                                semantic.structural_type == parameter.structural_type
                                                    && semantic.multiplicity == parameter.multiplicity
                                                    && semantic.projected_qualifications == parameter.projected_qualifications)))
                                    && &parameter.placement == placement
                            })
                        },
                    )
            }
            TargetByteView::Subslice {
                psi_operation,
                place,
                source: original,
                start,
                end,
                length,
                obligation,
            } => {
                access == terminal_psi::StructuralAccess::SharedBorrow
                    && *place == source
                    && self
                        .optimized
                        .blocks
                        .iter()
                        .flat_map(|block| &block.nodes)
                        .any(|node| {
                            let AbstractOperation::ByteSequenceSubslice {
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
                                && self.byte_view(original, *expected_source, aliases)
                                && self.integer_source(start, *expected_start, aliases)
                                && self.integer_source(end, *expected_end, aliases)
                        })
            }
        }
    }

    /// The operation at `psi_operation` establishes a borrowed `&[u8]` view at
    /// `source`: its declared result is the borrowed byte-view type and its
    /// element is the `u8` scalar exactly.
    fn established_byte_view(
        &self,
        psi_operation: semantic_vocabulary::OperationId,
        source: PlaceId,
        structural_type: semantic_vocabulary::StructuralTypeId,
    ) -> bool {
        self.optimized
            .blocks
            .iter()
            .flat_map(|block| &block.nodes)
            .any(|node| {
                let AbstractOperation::EstablishElementView {
                    psi_operation: operation,
                    result,
                    destination,
                    element,
                    ..
                } = &node.operation
                else {
                    return false;
                };
                operation == &psi_operation
                    && *destination == source
                    && result.structural_type == structural_type
                    && result.multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted
                    && result.qualifications.is_empty()
                    && result.projected_qualifications.is_empty()
                    && result.claims.is_empty()
                    && self
                        .types
                        .iter()
                        .find(|declaration| declaration.id == structural_type)
                        .is_some_and(|declaration| declaration.shape.is_borrowed_byte_view())
                    && self
                        .types
                        .iter()
                        .find(|declaration| declaration.id == *element)
                        .is_some_and(|declaration| {
                            matches!(
                                declaration.shape,
                                terminal_psi::StructuralTypeShape::PrimitiveScalar(
                                    semantic_vocabulary::ScalarType::Integer(integer)
                                ) if Ok(integer)
                                    == semantic_vocabulary::IntegerType::new(
                                        semantic_vocabulary::IntegerSign::Unsigned,
                                        8,
                                    )
                            )
                        })
            })
    }
}
