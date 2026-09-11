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
                                    && parameter.qualifications.is_empty()
                                    && parameter.projected_qualifications.is_empty()
                            })
                    })
            }
            TargetByteView::Parameter { place, placement } => {
                *place == source
                    && super::super::structural_parameters(self.function).is_some_and(
                        |parameters| {
                            parameters.iter().any(|parameter| {
                                parameter.place == source
                                    && parameter.access == access
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
}
