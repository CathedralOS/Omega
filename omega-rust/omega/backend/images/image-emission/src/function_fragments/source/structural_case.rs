//! Account for case observations already checked by mandatory selected-source replay.
use abstract_operations::{AbstractBlockEntry, AbstractStructuralCaseSuccessor};
use selected_instructions::{
    LocalStorageSlotId, SelectedFunction, SelectedSuccessorRole, SelectedTerminator,
};
use semantic_vocabulary::PlaceId;
use terminal_psi::{StructuralAccess, StructuralParameterDeclaration};

pub(super) fn retained(
    function: &SelectedFunction,
    source_blocks: &[AbstractBlockEntry],
    source_parameters: &[StructuralParameterDeclaration],
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
                && source_slot_matches(retained.slot, source, source_blocks, source_parameters)
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
// A function's own owned parameter dispatches from its entry-retained slot.
fn source_slot_matches(
    slot: LocalStorageSlotId,
    source: PlaceId,
    blocks: &[AbstractBlockEntry],
    parameters: &[StructuralParameterDeclaration],
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
        LocalStorageSlotId::StructuralParameter { place } => {
            let mut declared = parameters
                .iter()
                .filter(|parameter| parameter.place == source);
            place == source
                && owners.next().is_none()
                && declared
                    .next()
                    .is_some_and(|parameter| parameter.access == StructuralAccess::Owned)
                && declared.next().is_none()
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::{AbstractBlockEntry, LocalStorageSlotId, PlaceId, source_slot_matches};
    use semantic_vocabulary::{BlockId, OperationId, StructuralTypeId};
    use terminal_psi::{StructuralAccess, StructuralMultiplicity, StructuralParameterDeclaration};

    fn owned(place: PlaceId) -> StructuralParameterDeclaration {
        StructuralParameterDeclaration {
            place,
            position: 0,
            is_self: false,
            structural_type: StructuralTypeId::new(2).unwrap(),
            access: StructuralAccess::Owned,
            multiplicity: StructuralMultiplicity::Affine,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        }
    }

    #[test]
    fn case_parameter_accounting_requires_the_owned_function_parameter() {
        let source = PlaceId::new(7).unwrap();
        let slot = LocalStorageSlotId::StructuralParameter { place: source };
        let parameter = owned(source);
        assert!(source_slot_matches(
            slot,
            source,
            &[],
            std::slice::from_ref(&parameter)
        ));
        // No declared parameter, a borrowed one, a duplicate, or a block
        // arrival at the same place cannot stand in for the owned parameter.
        assert!(!source_slot_matches(slot, source, &[], &[]));
        let mut borrowed = parameter.clone();
        borrowed.access = StructuralAccess::SharedBorrow;
        assert!(!source_slot_matches(slot, source, &[], &[borrowed]));
        assert!(!source_slot_matches(
            slot,
            source,
            &[],
            &[parameter.clone(), parameter.clone()]
        ));
        let arrival = AbstractBlockEntry {
            block: BlockId::new(3).unwrap(),
            operation_offset: 0,
            parameters: Vec::new(),
            structural_parameters: vec![parameter.clone()],
        };
        assert!(!source_slot_matches(
            slot,
            source,
            &[arrival],
            std::slice::from_ref(&parameter)
        ));
        let other = LocalStorageSlotId::StructuralParameter {
            place: PlaceId::new(8).unwrap(),
        };
        assert!(!source_slot_matches(other, source, &[], &[parameter]));
    }

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
            std::slice::from_ref(&entry),
            &[]
        ));
        assert!(!source_slot_matches(slot, source, &[], &[]));
        assert!(!source_slot_matches(
            slot,
            source,
            &[entry.clone(), entry.clone()],
            &[]
        ));
        let mut substituted = entry.clone();
        substituted.block = BlockId::new(4).unwrap();
        assert!(!source_slot_matches(slot, source, &[substituted], &[]));
        let operation_slot = LocalStorageSlotId::Structural {
            operation: OperationId::new(1).unwrap(),
            place: source,
        };
        assert!(!source_slot_matches(operation_slot, source, &[entry], &[]));
        assert!(source_slot_matches(operation_slot, source, &[], &[]));
    }
}
