//! Exit admission retains the checked envelope's ordinary fall-through frame effects.
use super::validate_non_return;
use post_allocation_machine_to_selected_form_encoding::machine_code::{
    ResolvedPhysicalAddress, ResolvedSelectedFormRow, SelectedFormDecodedFootprint,
    SelectedFormEncodingRow, SelectedFormEncodingState, SelectedFormMachineDisposition,
};
use register_homes_to_post_allocation_machine::PhysicalAddressOperation;
use semantic_vocabulary::OperationId;
use target_operations_to_selected_instructions::{
    LocalStorageSlotId, MachineAlternativeKey, MachineEncodedControlEffect,
    MachineEncodedMemoryEffect, MachineEncodedStackEffect, MachineEncodedTrapBehavior,
    MachineSemanticKind, SelectedInstructionId, SelectedInstructionKind,
};

#[test]
fn floating_control_exit_admission_requires_frame_memory_and_unchanged_stack() {
    let physical = target_operations_to_selected_instructions::register_model::validate_physical_register_model(
        target_operations_to_selected_instructions::isa_aarch64::aarch64_physical_register_model(),
    )
    .unwrap();
    let slot = LocalStorageSlotId::Boundary {
        operation: OperationId::new(7).unwrap(),
    };
    for restore in [false, true] {
        let kind = if restore {
            SelectedInstructionKind::RestoreFloatingControl { slot }
        } else {
            SelectedInstructionKind::SaveFloatingControl { slot }
        };
        let semantic = if restore {
            MachineSemanticKind::RestoreFloatingControl
        } else {
            MachineSemanticKind::SaveFloatingControl
        };
        let alternative = MachineAlternativeKey {
            family: semantic.into(),
            variant: 0,
        };
        let encoded = target_operations_to_selected_instructions::isa_aarch64::encode_aarch64_selected_floating_control_form(
            &physical,
            kind,
            alternative,
            &[],
            16,
        )
        .unwrap();
        let instruction = SelectedInstructionId(1);
        let footprint = &encoded.footprint().encoded;
        let encoding = SelectedFormEncodingRow {
            instruction,
            alternative,
            machine_disposition: SelectedFormMachineDisposition::RetainedV1,
            address: Some(ResolvedPhysicalAddress {
                symbolic: if restore {
                    PhysicalAddressOperation::RestoreFloatingControl { slot }
                } else {
                    PhysicalAddressOperation::SaveFloatingControl { slot }
                },
                displacement: 16,
            }),
            state: SelectedFormEncodingState::Encoded {
                bytes: encoded.bytes().to_vec(),
                footprint: Box::new(SelectedFormDecodedFootprint {
                    register_reads: Vec::new(),
                    register_writes: Vec::new(),
                    implicit_defs: footprint.implicit_unit_defs.clone(),
                    implicit_clobbers: footprint.implicit_unit_clobbers.clone(),
                    encoded: footprint.clone(),
                }),
            },
        };
        let layout = ResolvedSelectedFormRow {
            instruction,
            alternative,
            offset: 0,
            bytes: encoded.bytes().to_vec(),
            branch: None,
            internal_machine_fixup: None,
            normalized_foreign_call_fixup: None,
        };
        validate_non_return(instruction, kind, &encoding, &layout).unwrap();
        for mutation in 0..6 {
            let mut changed = encoding.clone();
            let SelectedFormEncodingState::Encoded { footprint, .. } = &mut changed.state else {
                unreachable!()
            };
            match mutation {
                0 => footprint.encoded.memory = MachineEncodedMemoryEffect::NoneV1,
                1 => footprint.encoded.control = MachineEncodedControlEffect::DirectRelativeCallV1,
                2 => {
                    footprint.encoded.stack = MachineEncodedStackEffect::PopBytesV1 {
                        stack_pointer: physical.model().view_named("sp").unwrap().id,
                        byte_count: 8,
                    }
                }
                3 => footprint.encoded.trap = MachineEncodedTrapBehavior::NeverV1,
                4 => changed.address = None,
                _ => {
                    changed.address.as_mut().unwrap().symbolic = if restore {
                        PhysicalAddressOperation::SaveFloatingControl { slot }
                    } else {
                        PhysicalAddressOperation::RestoreFloatingControl { slot }
                    }
                }
            }
            assert!(
                validate_non_return(instruction, kind, &changed, &layout).is_err(),
                "mutation {mutation}"
            );
        }
    }
}
