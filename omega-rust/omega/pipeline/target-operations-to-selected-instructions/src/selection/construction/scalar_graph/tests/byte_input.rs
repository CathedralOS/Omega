//! Selected byte input rejoins a lawfully legalized owned result and frame slot.
use super::*;
use selected_instructions::{LocalStorageSlotId, SelectedBoundarySettlementPayload};

#[test]
fn repeated_byte_input_preserves_distinct_owned_homes_and_reverse_cleanup() {
    for target in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::macos_arm64(),
    ] {
        let (abstracted, native, unit) =
            crate::tests::legalization::byte_input::two_results_fixture(target);
        let legal = crate::legalize_target_operations(&native, &abstracted, &unit).unwrap();
        let source = &legal.plan().scalar_functions[0];
        let LegalizedScalarTerminator::Return(returned) = &source.blocks[0].terminator else {
            unreachable!()
        };
        assert_eq!(
            returned.ownership,
            [optimization_unit::OwnershipEvent::Cleanup(vec![
                terminal_psi::TerminalAffineCleanupAction::DiscardRoot(
                    semantic_vocabulary::PlaceId::new(2).unwrap()
                ),
                terminal_psi::TerminalAffineCleanupAction::DiscardRoot(
                    semantic_vocabulary::PlaceId::new(1).unwrap()
                ),
            ])]
        );
        let environment =
            register_environment::baseline_target_register_environment(target).unwrap();
        let constraints = SelectedSelectionConstraints {
            keys: environment.selected_keys(),
            fixed_inputs: Vec::new(),
        };
        let selected = build(
            0,
            source,
            target,
            &constraints,
            environment.physical(),
            environment.constraints(),
        )
        .unwrap();
        let validate = |selected: &SelectedFunction| {
            crate::selection::validation::scalar_graph::validate(
                0,
                source,
                selected,
                target,
                &constraints,
                environment.physical(),
                environment.constraints(),
            )
        };
        validate(&selected).unwrap();
        assert_eq!(selected.local_storage_slots.len(), 2);
        assert_eq!(selected.boundary_settlements.len(), 2);
        for (position, slot) in selected.local_storage_slots.iter().enumerate() {
            let identity = u64::try_from(position + 1).unwrap();
            assert_eq!(
                slot.id,
                LocalStorageSlotId::Structural {
                    operation: OperationId::new(identity).unwrap(),
                    place: semantic_vocabulary::PlaceId::new(identity).unwrap(),
                }
            );
            assert_eq!(slot.byte_size, 8);
        }
        let mut aliased = selected.clone();
        aliased.local_storage_slots[1].id = aliased.local_storage_slots[0].id;
        assert!(validate(&aliased).is_err());
        let mut duplicated = selected.clone();
        duplicated.boundary_settlements[1] = duplicated.boundary_settlements[0].clone();
        assert!(validate(&duplicated).is_err());
        for omitted in [false, true] {
            let mut changed = legal.plan().clone();
            let LegalizedScalarTerminator::Return(returned) =
                &mut changed.scalar_functions[0].blocks[0].terminator
            else {
                unreachable!()
            };
            let [optimization_unit::OwnershipEvent::Cleanup(actions)] =
                returned.ownership.as_mut_slice()
            else {
                unreachable!()
            };
            if omitted {
                actions.pop();
            } else {
                actions.reverse();
            }
            assert!(
                crate::validate_legalized_operations(&native, &abstracted, &unit, changed).is_err()
            );
        }
    }
}

#[test]
fn byte_input_selection_rejects_result_home_and_occurrence_substitution() {
    for target in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::macos_arm64(),
    ] {
        let (abstracted, native, unit) = crate::tests::legalization::byte_input::fixture(target);
        let legal = crate::legalize_target_operations(&native, &abstracted, &unit).unwrap();
        let source = &legal.plan().scalar_functions[0];
        let environment =
            register_environment::baseline_target_register_environment(target).unwrap();
        let constraints = SelectedSelectionConstraints {
            keys: environment.selected_keys(),
            fixed_inputs: Vec::new(),
        };
        let selected = build(
            0,
            source,
            target,
            &constraints,
            environment.physical(),
            environment.constraints(),
        )
        .unwrap();
        let validate = |selected: &SelectedFunction| {
            crate::selection::validation::scalar_graph::validate(
                0,
                source,
                selected,
                target,
                &constraints,
                environment.physical(),
                environment.constraints(),
            )
        };
        validate(&selected).unwrap();
        assert_eq!(selected.local_storage_slots.len(), 1);
        assert_eq!(selected.local_storage_slots[0].byte_size, 8);
        assert!(
            selected.virtual_registers.is_empty(),
            "read result is not a fabricated scalar"
        );
        for mutation in 0..10 {
            let mut changed = selected.clone();
            match mutation {
                0 => {
                    changed.blocks[0].instructions[0].kind =
                        SelectedInstructionKind::HostedReadByte {
                            slot: LocalStorageSlotId::Boundary {
                                operation: OperationId::new(1).unwrap(),
                            },
                        }
                }
                1 => changed.local_storage_slots[0].byte_size = 4,
                2 => changed.local_storage_slots[0].alignment = 1,
                3 => changed.blocks[0].instructions[0].provenance.fuel.clear(),
                4 => changed.blocks[0].instructions[0].clobbers.clear(),
                5 => changed.boundary_settlements[0].instruction_index += 1,
                6 | 7 => {
                    let SelectedBoundarySettlementPayload::HostedReadByte {
                        boundary, result, ..
                    } = &mut changed.boundary_settlements[0].settlement
                    else {
                        unreachable!()
                    };
                    if mutation == 6 {
                        *boundary = semantic_vocabulary::BoundaryMachineId::new(2).unwrap();
                    } else {
                        result.place = semantic_vocabulary::PlaceId::new(2).unwrap();
                    }
                }
                8 => changed.boundary_settlements.clear(),
                9 => changed
                    .boundary_settlements
                    .push(changed.boundary_settlements[0].clone()),
                _ => unreachable!(),
            }
            assert!(
                validate(&changed).is_err(),
                "mutation {mutation}, target {target:?}"
            );
        }
        for ownership in [
            Vec::new(),
            vec![optimization_unit::OwnershipEvent::ClaimCompletion(vec![
                semantic_vocabulary::ClaimId::new(1).unwrap(),
            ])],
        ] {
            let mut changed = source.clone();
            changed.blocks[0].instructions[0].ownership = ownership;
            assert!(
                build(
                    0,
                    &changed,
                    target,
                    &constraints,
                    environment.physical(),
                    environment.constraints()
                )
                .is_err()
            );
            assert!(
                crate::selection::validation::scalar_graph::validate(
                    0,
                    &changed,
                    &selected,
                    target,
                    &constraints,
                    environment.physical(),
                    environment.constraints(),
                )
                .is_err()
            );
        }
    }
}
