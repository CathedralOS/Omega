//! Account for case observations already checked by mandatory selected-source replay.
use abstract_operations::AbstractStructuralCaseSuccessor;
use selected_instructions::{
    LocalStorageSlotId, SelectedFunction, SelectedSuccessorRole, SelectedTerminator,
};
use semantic_vocabulary::PlaceId;

pub(super) fn retained(
    function: &SelectedFunction,
    source: PlaceId,
    cases: &[AbstractStructuralCaseSuccessor],
) -> bool {
    !cases.is_empty()
        && cases.iter().all(|case| {
            let mut matching = function.blocks.iter().flat_map(|block| {
                match &block.terminator {
                    SelectedTerminator::ConditionalBranch {
                        when_nonzero,
                        when_zero,
                        ..
                    } => [Some(when_zero), Some(when_nonzero)],
                    _ => [None, None],
                }
                .into_iter()
                .flatten()
                .filter(|successor| {
                    successor.role == SelectedSuccessorRole::Semantic
                        && successor.psi_edge == case.psi_edge
                })
            });
            let Some(successor) = matching.next() else {
                return false;
            };
            let Some(retained) = &successor.structural_case else {
                return false;
            };
            matching.next().is_none()
                && successor.source_target == case.target
                && matches!(retained.slot, LocalStorageSlotId::Structural { place, .. }
                    if place == source)
                && retained.case == case.case
                && retained.trivial_affine_discards == case.trivial_affine_discards
                && retained.payloads.len() == case.payloads.len()
                && retained
                    .payloads
                    .iter()
                    .zip(&case.payloads)
                    .all(|(retained, expected)| {
                        retained.semantic.field == expected.field
                            && retained.semantic.parameter.value == expected.parameter
                            && retained.semantic.parameter.scalar_type == expected.scalar_type
                    })
        })
}
