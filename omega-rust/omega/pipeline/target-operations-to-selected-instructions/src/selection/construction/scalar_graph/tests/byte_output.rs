//! Raw graph replay checks; these fixtures do not mint boundary admission.
use super::*;
use selected_instructions::{LocalStorageSlotId, SelectedBoundarySettlementPayload};
use semantic_vocabulary::BoundaryMachineId;

#[test]
fn byte_output_replay_binds_scalar_scratch_effect_and_boundary() {
    for target in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::macos_arm64(),
    ] {
        let mut source = fixture(target, 0);
        source.attachment = None;
        source.blocks[0].instructions.truncate(2);
        source.provenance.operations.truncate(2);
        let constant = &mut source.blocks[0].instructions[0];
        constant.result.as_mut().unwrap().scalar_type =
            ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).unwrap());
        constant.kind = LegalizedScalarInstructionKind::Constant(IntegerValue::Signed(255));
        let row = &mut source.blocks[0].instructions[1];
        row.result = None;
        row.ownership = vec![optimization_unit::OwnershipEvent::ClaimCompletion(
            Vec::new(),
        )];
        row.kind = LegalizedScalarInstructionKind::HostedWriteByteI32 {
            boundary: BoundaryMachineId::new(1).unwrap(),
            source: ValueId::new(1).unwrap(),
        };
        let environment =
            register_environment::baseline_target_register_environment(target).unwrap();
        let constraints = SelectedSelectionConstraints {
            keys: environment.selected_keys(),
            fixed_inputs: Vec::new(),
        };
        let selected = build(
            0,
            &source,
            target,
            &constraints,
            environment.physical(),
            environment.constraints(),
        )
        .unwrap();
        let validate = |selected: &SelectedFunction| {
            crate::selection::validation::scalar_graph::validate(
                0,
                &source,
                selected,
                target,
                &constraints,
                environment.physical(),
                environment.constraints(),
            )
        };
        validate(&selected).unwrap();
        for ownership in [
            Vec::new(),
            vec![optimization_unit::OwnershipEvent::ClaimCompletion(vec![
                semantic_vocabulary::ClaimId::new(1).unwrap(),
            ])],
        ] {
            let mut changed = source.clone();
            changed.blocks[0].instructions[1].ownership = ownership;
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
                    environment.constraints()
                )
                .is_err()
            );
        }
        assert_eq!(selected.local_storage_slots.len(), 1);
        assert!(
            selected.memory_accesses.is_empty(),
            "no invented structural place"
        );
        for mutation in 0..8 {
            let mut changed = selected.clone();
            match mutation {
                0 => {
                    changed.blocks[0].instructions[1].kind =
                        SelectedInstructionKind::HostedWriteByteI32 {
                            slot: LocalStorageSlotId::Boundary {
                                operation: OperationId::new(99).unwrap(),
                            },
                        }
                }
                1 => changed.local_storage_slots[0].byte_size = 0,
                2 => changed.blocks[0].instructions[1].provenance.fuel.clear(),
                3 => changed.blocks[0].instructions[1].clobbers.clear(),
                4 => {
                    changed.blocks[0].instructions[1].operands[0].virtual_register =
                        VirtualRegisterId(99)
                }
                5 => changed.boundary_settlements[0].instruction_index = 0,
                6 => {
                    changed.boundary_settlements[0].settlement =
                        SelectedBoundarySettlementPayload::HostedWriteByteI32 {
                            operation: OperationId::new(2).unwrap(),
                            boundary: BoundaryMachineId::new(99).unwrap(),
                            source: ValueId::new(1).unwrap(),
                        }
                }
                _ => {
                    changed.boundary_settlements.clear();
                }
            }
            assert!(validate(&changed).is_err(), "mutation {mutation}");
        }
    }
}
