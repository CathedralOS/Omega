//! Join semantic case edges to the selected payload coordinates. Root identity
//! alone does not identify a sum nested in a sibling field or array element.
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
    target: &target_operations::TargetFunction,
    source: PlaceId,
    path: &[terminal_psi::StructuralPathSegment],
    cases: &[AbstractStructuralCaseSuccessor],
) -> bool {
    let Some(first) = cases.first() else {
        return false;
    };
    let mut matching = target.graph.blocks.iter().filter_map(|block| {
        let target_operations::TargetControlTerminator::StructuralCase {
            source: root,
            path: projection,
            cases: successors,
            ..
        } = &block.terminator
        else {
            return None;
        };
        (root.place() == source
            && projection == path
            && successors
                .iter()
                .any(|case| case.psi_edge == first.psi_edge))
        .then_some((root, successors))
    });
    let Some((target_source, target_cases)) = matching.next() else {
        return false;
    };
    if matching.next().is_some() || target_cases.len() != cases.len() {
        return false;
    }
    !cases.is_empty()
        && cases.iter().all(|case| {
            let mut target_rows = target_cases
                .iter()
                .filter(|row| row.psi_edge == case.psi_edge);
            let Some(target_case) = target_rows.next() else {
                return false;
            };
            if target_rows.next().is_some()
                || target_case.case != case.case
                || target_case.target != case.target
            {
                return false;
            }
            // A tested case leaves a tag branch; the last case of a dispatch
            // with no test before it (a one-case sum) leaves an unconditional
            // jump. Either way the semantic edge appears exactly once.
            let mut matching = function.blocks.iter().flat_map(|block| {
                match &block.terminator {
                    SelectedTerminator::ConditionalBranch {
                        when_nonzero,
                        when_zero,
                        ..
                    } => [Some(when_zero), Some(when_nonzero)],
                    SelectedTerminator::Jump { successor, .. } => [Some(successor), None],
                    _ => [None, None],
                }
                .into_iter()
                .flatten()
                .filter(|successor| {
                    successor.role == SelectedSuccessorRole::Semantic
                        && successor.psi_edge == Some(case.psi_edge)
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
                && projected_source_matches(
                    &function.local_storage_slots,
                    &retained.source,
                    target_source,
                    path,
                    source_blocks,
                    source_parameters,
                )
                && retained.case == case.case
                && retained.case_tag == target_case.case_tag
                && retained.cleanup_actions == case.cleanup_actions
                && retained.payloads.len() == case.payloads.len()
                && retained.payloads.len() == target_case.payloads.len()
                && retained
                    .payloads
                    .iter()
                    .zip(&case.payloads)
                    .zip(&target_case.payloads)
                    .all(|((retained, expected), physical)| {
                        retained.semantic.field == expected.field
                            && physical.field == expected.field
                            && retained.semantic.field_byte_offset == physical.field_byte_offset
                            && retained.semantic.parameter.value == expected.parameter
                            && retained.semantic.parameter.scalar_type == expected.scalar_type
                    })
        })
}

/// An addressed projection still belongs to its original descriptor home or
/// arrival. The source path must actually cross a reference, and the exact
/// target owner and storage extent stay retained beside the resolved pointer.
/// Selected-source replay independently reconstructs every address-word load.
fn projected_source_matches(
    slots: &[selected_instructions::SelectedLocalStorageSlot],
    dispatch: &selected_instructions::SelectedCaseDispatchSource,
    target: &target_operations::TargetStructuralCaseSource,
    path: &[terminal_psi::StructuralPathSegment],
    blocks: &[AbstractBlockEntry],
    parameters: &[StructuralParameterDeclaration],
) -> bool {
    if !path.contains(&terminal_psi::StructuralPathSegment::Referent) {
        return source_slot_matches(dispatch, target.place(), blocks, parameters);
    }
    let selected_instructions::SelectedCaseDispatchSource::Borrowed { place, .. } = dispatch else {
        return false;
    };
    if *place != target.place() || !terminal_psi::is_static_structural_path(path) {
        return false;
    }
    let target_operations::TargetStructuralCaseSource::Home(home) = target else {
        return source_slot_matches(dispatch, target.place(), blocks, parameters);
    };
    let slot = match &home.origin {
        target_operations::TargetStructuralHomeOrigin::OperationResult { operation, result } => {
            LocalStorageSlotId::Structural {
                operation: *operation,
                place: result.place,
            }
        }
        target_operations::TargetStructuralHomeOrigin::BlockParameter { block, declaration } => {
            LocalStorageSlotId::StructuralBlockParameter {
                block: *block,
                place: declaration.place,
            }
        }
    };
    let shape = home.layout.shape();
    source_slot_matches(
        &selected_instructions::SelectedCaseDispatchSource::Local { slot },
        *place,
        blocks,
        parameters,
    ) && slots
        .iter()
        .filter(|row| {
            row.id == slot && row.byte_size == shape.byte_size && row.alignment == shape.alignment
        })
        .count()
        == 1
}

// Full instruction correspondence belongs to selected-source replay. This
// publication roster join distinguishes an operation result from a destination-owned
// arrival; equal place numbers cannot substitute a different block's home.
// A function's owned parameter can use an entry-retained local slot or the
// address of its ABI value copy; a readable loan uses its referent pointer.
// The selected-source replay checks that physical choice against the call ABI.
fn source_slot_matches(
    dispatch: &selected_instructions::SelectedCaseDispatchSource,
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
    let slot = match dispatch {
        selected_instructions::SelectedCaseDispatchSource::Local { slot } => *slot,
        selected_instructions::SelectedCaseDispatchSource::Borrowed { place, .. } => {
            let borrowed = |parameter: &StructuralParameterDeclaration| {
                matches!(
                    parameter.access,
                    terminal_psi::reading_borrow_pair_pattern!()
                )
            };
            if *place != source {
                return false;
            }
            if let Some(owner) = owners.next() {
                // An edge-carried join dispatches through its destination
                // block's retained referent pointer: exactly one block declares
                // the place, and that declaration is a readable borrow.
                return owners.next().is_none()
                    && blocks
                        .iter()
                        .find(|block| block.block == owner)
                        .and_then(|block| {
                            let mut declared = block
                                .structural_parameters
                                .iter()
                                .filter(|parameter| parameter.place == source);
                            declared
                                .next()
                                .filter(|parameter| borrowed(parameter))
                                .and_then(|_| declared.next().is_none().then_some(()))
                        })
                        .is_some();
            }
            let mut declared = parameters
                .iter()
                .filter(|parameter| parameter.place == source);
            return declared.next().is_some_and(|parameter| {
                parameter.access == StructuralAccess::Owned || borrowed(parameter)
            }) && declared.next().is_none();
        }
    };
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
            structural_type: StructuralTypeId::new(2).unwrap(),
            access: StructuralAccess::Owned,
            multiplicity: StructuralMultiplicity::Affine,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        }
    }

    #[test]
    fn addressed_case_parameter_keeps_its_declared_owner_and_read_access() {
        let source = PlaceId::new(7).unwrap();
        let dispatch = selected_instructions::SelectedCaseDispatchSource::Borrowed {
            place: source,
            pointer: selected_instructions::VirtualRegisterId(3),
            byte_size: 20,
        };
        for access in [
            StructuralAccess::Owned,
            StructuralAccess::SharedBorrow,
            StructuralAccess::MutableBorrow,
        ] {
            let mut parameter = owned(source);
            parameter.access = access;
            assert!(source_slot_matches(
                &dispatch,
                source,
                &[],
                std::slice::from_ref(&parameter)
            ));
            assert!(!source_slot_matches(
                &dispatch,
                source,
                &[],
                &[parameter.clone(), parameter]
            ));
        }
        assert!(!source_slot_matches(&dispatch, source, &[], &[]));
        let mut write_only = owned(source);
        write_only.access = StructuralAccess::WriteOnlyBorrow;
        assert!(!source_slot_matches(&dispatch, source, &[], &[write_only]));
        let other = owned(PlaceId::new(8).unwrap());
        assert!(!source_slot_matches(&dispatch, source, &[], &[other]));
        let arrival = AbstractBlockEntry {
            block: BlockId::new(3).unwrap(),
            operation_offset: 0,
            parameters: Vec::new(),
            structural_parameters: vec![owned(source)],
        };
        assert!(!source_slot_matches(
            &dispatch,
            source,
            &[arrival],
            &[owned(source)]
        ));
    }

    #[test]
    fn case_parameter_accounting_requires_the_owned_function_parameter() {
        let source = PlaceId::new(7).unwrap();
        let slot = LocalStorageSlotId::StructuralParameter { place: source };
        let parameter = owned(source);
        assert!(source_slot_matches(
            &selected_instructions::SelectedCaseDispatchSource::Local { slot },
            source,
            &[],
            std::slice::from_ref(&parameter)
        ));
        // No declared parameter, a borrowed one, a duplicate, or a block
        // arrival at the same place cannot stand in for the owned parameter.
        assert!(!source_slot_matches(
            &selected_instructions::SelectedCaseDispatchSource::Local { slot },
            source,
            &[],
            &[]
        ));
        let mut borrowed = parameter.clone();
        borrowed.access = StructuralAccess::SharedBorrow;
        assert!(!source_slot_matches(
            &selected_instructions::SelectedCaseDispatchSource::Local { slot },
            source,
            &[],
            &[borrowed]
        ));
        assert!(!source_slot_matches(
            &selected_instructions::SelectedCaseDispatchSource::Local { slot },
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
            &selected_instructions::SelectedCaseDispatchSource::Local { slot },
            source,
            &[arrival],
            std::slice::from_ref(&parameter)
        ));
        let other = LocalStorageSlotId::StructuralParameter {
            place: PlaceId::new(8).unwrap(),
        };
        assert!(!source_slot_matches(
            &selected_instructions::SelectedCaseDispatchSource::Local { slot: other },
            source,
            &[],
            &[parameter]
        ));
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
                structural_type: StructuralTypeId::new(2).unwrap(),
                access: StructuralAccess::Owned,
                multiplicity: StructuralMultiplicity::Affine,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
            }],
        };
        assert!(source_slot_matches(
            &selected_instructions::SelectedCaseDispatchSource::Local { slot },
            source,
            std::slice::from_ref(&entry),
            &[]
        ));
        assert!(!source_slot_matches(
            &selected_instructions::SelectedCaseDispatchSource::Local { slot },
            source,
            &[],
            &[]
        ));
        assert!(!source_slot_matches(
            &selected_instructions::SelectedCaseDispatchSource::Local { slot },
            source,
            &[entry.clone(), entry.clone()],
            &[]
        ));
        let mut substituted = entry.clone();
        substituted.block = BlockId::new(4).unwrap();
        assert!(!source_slot_matches(
            &selected_instructions::SelectedCaseDispatchSource::Local { slot },
            source,
            &[substituted],
            &[]
        ));
        let operation_slot = LocalStorageSlotId::Structural {
            operation: OperationId::new(1).unwrap(),
            place: source,
        };
        assert!(!source_slot_matches(
            &selected_instructions::SelectedCaseDispatchSource::Local {
                slot: operation_slot
            },
            source,
            &[entry],
            &[]
        ));
        assert!(source_slot_matches(
            &selected_instructions::SelectedCaseDispatchSource::Local {
                slot: operation_slot
            },
            source,
            &[],
            &[]
        ));
    }
}
