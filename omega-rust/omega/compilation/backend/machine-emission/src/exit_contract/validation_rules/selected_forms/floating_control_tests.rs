//! Exit admission retains the checked envelope's ordinary fall-through frame effects.
use super::assert_non_return_refused;
use physical_instructions::PhysicalAddressOperation;
use selected_instructions::{
    LocalStorageSlotId, MachineAlternativeKey, MachineEncodedControlEffect,
    MachineEncodedMemoryEffect, MachineEncodedStackEffect, MachineEncodedTrapBehavior,
    MachineSemanticKind, SelectedInstructionId, SelectedInstructionKind,
};
use semantic_vocabulary::OperationId;

#[test]
fn floating_control_exit_admission_requires_frame_memory_and_unchanged_stack() {
    let physical = super::aarch64_physical_register_model();
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
        let encoded = isa_aarch64::encode_aarch64_selected_floating_control_form(
            &physical,
            kind,
            alternative,
            &[],
            16,
        )
        .unwrap();
        let instruction = SelectedInstructionId(1);
        let (store, encoding, layout) = super::validated_aarch64_addressed_form_fixture(
            &encoded,
            instruction,
            alternative,
            kind,
            if restore {
                PhysicalAddressOperation::RestoreFloatingControl { slot }
            } else {
                PhysicalAddressOperation::SaveFloatingControl { slot }
            },
        );
        for mutation in 0..6 {
            // A mutation reaches either the row's own address or the
            // footprint the store holds, so both are cloned per mutation.
            let mut changed_row = encoding.clone();
            let mut changed = store.clone();
            let footprint = changed.footprint_mut(encoding.footprint).unwrap();
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
                4 => changed_row.address = None,
                _ => {
                    changed_row.address.as_mut().unwrap().symbolic = if restore {
                        PhysicalAddressOperation::SaveFloatingControl { slot }
                    } else {
                        PhysicalAddressOperation::RestoreFloatingControl { slot }
                    }
                }
            }
            assert_non_return_refused(
                &format!("mutation {mutation}"),
                instruction,
                kind,
                &changed_row,
                &layout,
                &changed,
            );
        }
    }
}
