//! Account for case observations already checked by mandatory selected-source replay.
use abstract_operations::{AbstractBlockEntry, AbstractStructuralCaseSuccessor};
use selected_instructions::{
    LocalStorageSlotId, SelectedFunction, SelectedSuccessorRole, SelectedTerminator,
};
use semantic_vocabulary::PlaceId;

pub(super) fn retained(
    function: &SelectedFunction,
    source_blocks: &[AbstractBlockEntry],
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
                && source_slot_matches(retained.slot, source, source_blocks)
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

// Mandatory selected-source replay validates the complete realization. This
// roster join still distinguishes an operation result from a destination-owned
// arrival; equal place numbers cannot substitute a different block's home.
fn source_slot_matches(
    slot: LocalStorageSlotId,
    source: PlaceId,
    blocks: &[AbstractBlockEntry],
) -> bool {
    let mut owners = blocks.iter().flat_map(|block| {
        block
            .structural_parameters
            .iter()
            .filter_map(move |parameter| (parameter.place == source).then_some(block.block))
    });
    match slot {
        LocalStorageSlotId::Structural { place, .. } => place == source && owners.next().is_none(),
        LocalStorageSlotId::StructuralBlockParameter { block, place } => {
            place == source && owners.next() == Some(block) && owners.next().is_none()
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use semantic_vocabulary::{BlockId, OperationId, StructuralTypeId};
    use terminal_psi::{StructuralAccess, StructuralMultiplicity, StructuralParameterDeclaration};

    #[test]
    fn case_home_accounting_requires_the_unique_actual_arrival_owner() {
        let source = PlaceId::new(7).unwrap();
        let owner = BlockId::new(3).unwrap();
        let slot = LocalStorageSlotId::StructuralBlockParameter {
            block: owner,
            place: source,
        };
        let entry = AbstractBlockEntry {
            block: owner,
            operation_offset: 0,
            parameters: Vec::new(),
            structural_parameters: vec![StructuralParameterDeclaration {
                place: source,
                position: 0,
                is_self: false,
                structural_type: StructuralTypeId::new(2).unwrap(),
                access: StructuralAccess::Owned,
                multiplicity: StructuralMultiplicity::Affine,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
            }],
        };
        assert!(source_slot_matches(
            slot,
            source,
            std::slice::from_ref(&entry)
        ));
        assert!(!source_slot_matches(slot, source, &[]));
        assert!(!source_slot_matches(
            slot,
            source,
            &[entry.clone(), entry.clone()]
        ));
        let mut substituted = entry.clone();
        substituted.block = BlockId::new(4).unwrap();
        assert!(!source_slot_matches(slot, source, &[substituted]));
        let operation_slot = LocalStorageSlotId::Structural {
            operation: OperationId::new(1).unwrap(),
            place: source,
        };
        assert!(!source_slot_matches(operation_slot, source, &[entry]));
        assert!(source_slot_matches(operation_slot, source, &[]));
    }
}
