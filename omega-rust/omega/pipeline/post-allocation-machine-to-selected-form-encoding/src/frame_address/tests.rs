//! Raw frame proposals exercise address equations without granting frame authority.
use super::*;
use selected_instructions::{
    LocalStorageSlotId, MachineAlternative, MachineAlternativeApplicability,
    MachineAlternativeFamily, MachineAlternativeKey, MachineEncodedEffects,
    MachineLatencyKnowledge, MachineSizeKnowledge, SelectedInstructionId, SelectedLocalStorageSlot,
};
use semantic_vocabulary::{MachineId, OperationId, PlaceId};

fn fixture() -> (
    PostAllocationMachineFunction,
    TargetFrameLayoutPlan,
    PostAllocationMachineInstruction,
) {
    let id = LocalStorageSlotId {
        operation: OperationId::new(3).unwrap(),
        place: PlaceId::new(5).unwrap(),
    };
    let machine = MachineId::new(1).unwrap();
    let function = PostAllocationMachineFunction {
        machine,
        outgoing_arguments: Vec::new(),
        local_storage_slots: vec![SelectedLocalStorageSlot {
            id,
            byte_size: 16,
            alignment: 8,
        }],
        blocks: Vec::new(),
    };
    let frame = TargetFrameLayoutPlan {
        post_allocation_machine: physical_instructions::PostAllocationMachineIdentity::from_bytes([0;32]),
        callee_saved_requirements: selected_instructions_to_register_homes::AllocatedCalleeSavedRequirementIdentity::from_bytes([0;32]),
        callee_save_storage: machine_code::NonAuthoritativeCalleeSaveStorageIdentity::from_bytes([0;32]),
        register_environment: register_model::TargetRegisterEnvironmentIdentity::from_bytes([0;32]),
        physical_register_model: register_model::PhysicalRegisterModelIdentity::from_bytes([0;32]),
        target: target::NativeTarget::linux_x64(),
        abi: register_model::FrameAbiPreservationConvention::SystemVAMD64,
        policy: machine_code::TargetFrameLayoutPolicy::CanonicalOrdinaryCallFrameV1,
        functions: vec![FunctionTargetFrameLayout {
            machine, contains_call: false, stack_pointer: register_model::RegisterViewId(0),
            pre_call_stack_alignment: 16, frame_size_bytes: 16, abi_stack_alignment_bytes: 16,
            outgoing_abi_area: machine_code::OutgoingAbiFrameArea { byte_size: 0, shadow_bytes: 0 },
            local_storage_slots: vec![machine_code::LocalStorageFrameSlot {
                id, frame_offset_bytes: 0, size_bytes: 16, alignment_bytes: 8,
            }],
            callee_save_slots: Vec::new(),
            return_address: machine_code::ReturnAddressFrameCustody::CallerActivationStack {
                post_prologue_offset_bytes: 16, size_bytes: 8,
            },
        }],
    };
    let instruction = PostAllocationMachineInstruction {
        instruction: SelectedInstructionId(1),
        alternative: MachineAlternative {
            key: MachineAlternativeKey {
                family: MachineAlternativeFamily::FrameAddress,
                variant: 0,
            },
            applicability: MachineAlternativeApplicability::Always,
            size: MachineSizeKnowledge::ExactBytes(8),
            latency: MachineLatencyKnowledge::StableBaselineUnavailable,
            encoded: MachineEncodedEffects::fallthrough_v1(Vec::new(), vec![0]),
        },
        address: Some(Address::FrameAddress {
            slot: FrameStorageSlotId::Local(id),
            byte_offset: 16,
        }),
        operands: Vec::new(),
        implicit_unit_uses: Vec::new(),
        implicit_unit_defs: Vec::new(),
        implicit_unit_clobbers: Vec::new(),
        unit_uses: Vec::new(),
        unit_defs: Vec::new(),
        unit_clobbers: Vec::new(),
    };
    (function, frame, instruction)
}

#[test]
fn empty_local_backing_allows_address_but_not_one_past_store() {
    let (function, frame, mut instruction) = fixture();
    let resolved = resolve(&function, Some(&frame), &instruction).unwrap();
    assert_eq!(resolved.unwrap().displacement, 16);
    validate_address(&function, Some(&frame), &instruction, resolved).unwrap();
    let Address::FrameAddress { slot, .. } = instruction.address.unwrap() else {
        unreachable!()
    };
    instruction.address = Some(Address::Store64 {
        slot,
        byte_offset: 16,
    });
    assert!(resolve(&function, Some(&frame), &instruction).is_err());
    assert!(
        validate_address(
            &function,
            Some(&frame),
            &instruction,
            Some(ResolvedPhysicalAddress {
                symbolic: instruction.address.unwrap(),
                displacement: 16
            })
        )
        .is_err()
    );
    instruction.address = Some(Address::Store64 {
        slot,
        byte_offset: 8,
    });
    let resolved = resolve(&function, Some(&frame), &instruction).unwrap();
    validate_address(&function, Some(&frame), &instruction, resolved).unwrap();
}

#[test]
fn local_address_replay_rejects_displacement_source_and_extent_substitution() {
    let (function, frame, instruction) = fixture();
    let resolved = resolve(&function, Some(&frame), &instruction).unwrap();
    let mut shifted = resolved.unwrap();
    shifted.displacement -= 8;
    assert!(validate_address(&function, Some(&frame), &instruction, Some(shifted)).is_err());
    for mutation in 0..4 {
        let mut changed = frame.clone();
        let local = &mut changed.functions[0].local_storage_slots[0];
        match mutation {
            0 => local.id.place = PlaceId::new(7).unwrap(),
            1 => local.id.operation = OperationId::new(11).unwrap(),
            2 => local.size_bytes += 8,
            _ => local.alignment_bytes = 4,
        }
        assert!(resolve(&function, Some(&changed), &instruction).is_err());
        assert!(validate_address(&function, Some(&changed), &instruction, resolved).is_err());
    }
    let mut duplicate = function.clone();
    duplicate
        .local_storage_slots
        .push(function.local_storage_slots[0].clone());
    assert!(resolve(&duplicate, Some(&frame), &instruction).is_err());
    assert!(validate_address(&duplicate, Some(&frame), &instruction, resolved).is_err());
}
