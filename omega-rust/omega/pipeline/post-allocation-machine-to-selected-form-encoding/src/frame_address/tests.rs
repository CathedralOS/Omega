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
    let id = LocalStorageSlotId::Structural {
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
fn read_byte_result_requires_structural_identity_and_exact_eight_byte_home() {
    let (mut function, mut frame, mut instruction) = fixture();
    let slot = function.local_storage_slots[0].id;
    function.local_storage_slots[0].byte_size = 8;
    function.local_storage_slots[0].alignment = 4;
    frame.functions[0].local_storage_slots[0].size_bytes = 8;
    frame.functions[0].local_storage_slots[0].alignment_bytes = 4;
    instruction.address = Some(Address::HostedReadByte { slot });
    let resolved = resolve(&function, Some(&frame), &instruction)
        .unwrap()
        .unwrap();
    validate_address(&function, Some(&frame), &instruction, Some(resolved)).unwrap();
    for mutation in 0..8 {
        let mut changed_function = function.clone();
        let mut changed_frame = frame.clone();
        let mut changed_instruction = instruction.clone();
        let mut candidate = resolved;
        match mutation {
            0 => candidate.displacement = 4,
            1 => {
                let wrong_slot = LocalStorageSlotId::Boundary {
                    operation: OperationId::new(3).unwrap(),
                };
                changed_function.local_storage_slots[0].id = wrong_slot;
                changed_frame.functions[0].local_storage_slots[0].id = wrong_slot;
                changed_instruction.address = Some(Address::HostedReadByte { slot: wrong_slot });
                candidate.symbolic = changed_instruction.address.unwrap();
            }
            2 => {
                changed_function.local_storage_slots[0].byte_size = 4;
                changed_frame.functions[0].local_storage_slots[0].size_bytes = 4;
            }
            3 => {
                changed_function.local_storage_slots[0].alignment = 8;
                changed_frame.functions[0].local_storage_slots[0].alignment_bytes = 8;
            }
            4 => changed_frame.functions[0].frame_size_bytes = 7,
            5 => changed_frame.functions[0].local_storage_slots[0].frame_offset_bytes = 1,
            6 => changed_function
                .local_storage_slots
                .push(function.local_storage_slots[0].clone()),
            _ => {
                changed_frame.functions[0].local_storage_slots[0].id =
                    LocalStorageSlotId::Structural {
                        operation: OperationId::new(3).unwrap(),
                        place: PlaceId::new(7).unwrap(),
                    }
            }
        }
        if mutation != 0 {
            assert!(
                resolve(
                    &changed_function,
                    Some(&changed_frame),
                    &changed_instruction
                )
                .is_err()
            );
        }
        assert!(
            validate_address(
                &changed_function,
                Some(&changed_frame),
                &changed_instruction,
                Some(candidate)
            )
            .is_err()
        );
    }
}

#[test]
fn incoming_pointer_slots_bind_entry_bias_frame_size_and_parameter_identity() {
    let (function, mut frame, mut instruction) = fixture();
    let slot = FrameStorageSlotId::Incoming {
        parameter_index: 8,
        abi_stack_byte_offset: 32,
    };
    instruction.address = Some(Address::FrameAddress {
        slot,
        byte_offset: 0,
    });
    for return_address in [
        machine_code::ReturnAddressFrameCustody::CallerActivationStack {
            post_prologue_offset_bytes: 16,
            size_bytes: 8,
        },
        machine_code::ReturnAddressFrameCustody::LiveLinkRegister {
            view: register_model::RegisterViewId(30),
        },
        machine_code::ReturnAddressFrameCustody::SavedLinkRegister {
            view: register_model::RegisterViewId(30),
            frame_offset_bytes: 8,
            size_bytes: 8,
        },
    ] {
        frame.functions[0].return_address = return_address;
        let bias = if matches!(
            return_address,
            machine_code::ReturnAddressFrameCustody::CallerActivationStack { .. }
        ) {
            8
        } else {
            0
        };
        let resolved = resolve(&function, Some(&frame), &instruction)
            .unwrap()
            .unwrap();
        assert_eq!(resolved.displacement, 16 + bias + 32);
        validate_address(&function, Some(&frame), &instruction, Some(resolved)).unwrap();
        for displacement in [resolved.displacement - 8, resolved.displacement + 8] {
            assert!(
                validate_address(
                    &function,
                    Some(&frame),
                    &instruction,
                    Some(ResolvedPhysicalAddress {
                        displacement,
                        ..resolved
                    })
                )
                .is_err()
            );
        }
        let changed = ResolvedPhysicalAddress {
            symbolic: Address::FrameAddress {
                slot: FrameStorageSlotId::Incoming {
                    parameter_index: 9,
                    abi_stack_byte_offset: 32,
                },
                byte_offset: 0,
            },
            ..resolved
        };
        assert!(validate_address(&function, Some(&frame), &instruction, Some(changed)).is_err());
    }
    assert!(resolve(&function, None, &instruction).is_err());
    for address in [
        Address::Store64 {
            slot,
            byte_offset: 0,
        },
        Address::FrameAddress {
            slot,
            byte_offset: 8,
        },
    ] {
        instruction.address = Some(address);
        assert!(resolve(&function, Some(&frame), &instruction).is_err());
        assert!(
            validate_address(
                &function,
                Some(&frame),
                &instruction,
                Some(ResolvedPhysicalAddress {
                    symbolic: address,
                    displacement: 48
                })
            )
            .is_err()
        );
    }
}

#[test]
fn boundary_byte_scratch_requires_exact_origin_geometry_and_offset() {
    let (mut function, mut frame, mut instruction) = fixture();
    let slot = LocalStorageSlotId::Boundary {
        operation: OperationId::new(3).unwrap(),
    };
    function.local_storage_slots[0] = SelectedLocalStorageSlot {
        id: slot,
        byte_size: 1,
        alignment: 1,
    };
    frame.functions[0].local_storage_slots[0] = machine_code::LocalStorageFrameSlot {
        id: slot,
        frame_offset_bytes: 0,
        size_bytes: 1,
        alignment_bytes: 1,
    };
    instruction.address = Some(Address::HostedWriteByteI32 { slot });
    let resolved = resolve(&function, Some(&frame), &instruction).unwrap();
    validate_address(&function, Some(&frame), &instruction, resolved).unwrap();
    for mutation in 0..6 {
        let mut changed_function = function.clone();
        let mut changed_frame = frame.clone();
        let mut changed_instruction = instruction.clone();
        let mut candidate = resolved.unwrap();
        match mutation {
            0 => candidate.displacement = 1,
            1 => {
                changed_instruction.address = Some(Address::HostedWriteByteI32 {
                    slot: LocalStorageSlotId::Structural {
                        operation: slot.operation().expect("source-backed local slot"),
                        place: PlaceId::new(5).unwrap(),
                    },
                })
            }
            2 => {
                changed_frame.functions[0].local_storage_slots[0].id =
                    LocalStorageSlotId::Boundary {
                        operation: OperationId::new(7).unwrap(),
                    }
            }
            3 => {
                changed_function.local_storage_slots[0].byte_size = 2;
                changed_frame.functions[0].local_storage_slots[0].size_bytes = 2;
            }
            4 => {
                changed_function.local_storage_slots[0].alignment = 2;
                changed_frame.functions[0].local_storage_slots[0].alignment_bytes = 2;
            }
            _ => changed_frame.functions[0].frame_size_bytes = 0,
        }
        if mutation != 0 {
            assert!(
                resolve(
                    &changed_function,
                    Some(&changed_frame),
                    &changed_instruction
                )
                .is_err()
            );
        }
        assert!(
            validate_address(
                &changed_function,
                Some(&changed_frame),
                &changed_instruction,
                Some(candidate)
            )
            .is_err()
        );
    }
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
            0 => {
                local.id = selected_instructions::LocalStorageSlotId::Structural {
                    operation: local.id.operation().expect("source-backed local slot"),
                    place: PlaceId::new(7).unwrap(),
                }
            }
            1 => {
                local.id = selected_instructions::LocalStorageSlotId::Structural {
                    operation: OperationId::new(11).unwrap(),
                    place: local.id.structural_place().unwrap(),
                }
            }
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

#[test]
fn pointer_address_replay_binds_base_offset_and_store_width_without_a_frame() {
    let (function, _, mut instruction) = fixture();
    for symbolic in [
        Address::Store {
            base_operand: 0,
            byte_offset: 2,
            byte_size: 2,
        },
        Address::AddressOffset {
            base_operand: 0,
            byte_offset: 2,
        },
    ] {
        instruction.address = Some(symbolic);
        let resolved = resolve(&function, None, &instruction).unwrap();
        validate_address(&function, None, &instruction, resolved).unwrap();
        let mut changed = resolved.unwrap();
        changed.displacement += 1;
        assert!(validate_address(&function, None, &instruction, Some(changed)).is_err());
        changed.displacement = 2;
        changed.symbolic = Address::Store {
            base_operand: 1,
            byte_offset: 2,
            byte_size: 2,
        };
        assert!(validate_address(&function, None, &instruction, Some(changed)).is_err());
    }
    instruction.address = Some(Address::Store {
        base_operand: 0,
        byte_offset: 2,
        byte_size: 3,
    });
    assert!(resolve(&function, None, &instruction).is_err());
    assert!(
        validate_address(
            &function,
            None,
            &instruction,
            Some(ResolvedPhysicalAddress {
                symbolic: instruction.address.unwrap(),
                displacement: 2,
            })
        )
        .is_err()
    );
}

#[test]
fn compiler_spill_slot_addresses_bind_register_identity_and_exact_store_bounds() {
    let (mut function, mut frame, mut instruction) = fixture();
    let slot = LocalStorageSlotId::Spill {
        register: selected_instructions::VirtualRegisterId(7),
    };
    function.local_storage_slots[0].id = slot;
    function.local_storage_slots[0].byte_size = 8;
    frame.functions[0].local_storage_slots[0].id = slot;
    frame.functions[0].local_storage_slots[0].size_bytes = 8;
    instruction.address = Some(Address::Store64 {
        slot: FrameStorageSlotId::Local(slot),
        byte_offset: 0,
    });
    assert!(resolve(&function, Some(&frame), &instruction).is_ok());
    instruction.address = Some(Address::Store64 {
        slot: FrameStorageSlotId::Local(slot),
        byte_offset: 1,
    });
    assert!(resolve(&function, Some(&frame), &instruction).is_err());
    instruction.address = Some(Address::Store64 {
        slot: FrameStorageSlotId::Local(slot),
        byte_offset: 0,
    });
    let original_identity = machine_code::target_frame_layout_identity(&frame);
    frame.functions[0].local_storage_slots[0].id = LocalStorageSlotId::Spill {
        register: selected_instructions::VirtualRegisterId(8),
    };
    assert_ne!(
        machine_code::target_frame_layout_identity(&frame),
        original_identity
    );
    assert!(resolve(&function, Some(&frame), &instruction).is_err());
    frame.functions[0].local_storage_slots[0].id = slot;
    frame.functions[0].frame_size_bytes = 7;
    assert!(resolve(&function, Some(&frame), &instruction).is_err());
}
