//! Round-trip, closed-tag, framing, and content-identity tests.

use optimization_core::PostAllocationOptimizationManifestIdentity;
use register_homes::{AllocationLegalityIdentity, RegisterHomeIdentity};
use register_model::{
    PhysicalRegisterModelIdentity, RegisterClassId, RegisterConstraintCatalogIdentity,
    RegisterOperandAccess, RegisterUnitId, RegisterViewId, RegisterWriteSemantics,
    TargetRegisterEnvironmentIdentity,
};
use selected_instructions::LiveRangeIdentity;
use selected_instructions::{
    MachineAlternative, MachineAlternativeApplicability, MachineAlternativeFamily,
    MachineAlternativeKey, MachineEffectCatalogIdentity, MachineEncodedControlEffect,
    MachineEncodedEffects, MachineEncodedMemoryEffect, MachineEncodedStackEffect,
    MachineEncodedTrapBehavior, MachineLatencyKnowledge, MachineSizeKnowledge,
    OutgoingArgumentSlotId, PreAllocationMachineEffectIdentity, SelectedBlockId,
    SelectedInstructionId, SelectedInstructionPlanIdentity, SelectedOutgoingArgumentSlot,
    VirtualRegisterId,
};
use semantic_vocabulary::{MachineId, OperationId};
use target::NativeTarget;

use crate::{
    MachineAlternativeChoiceRule, PhysicalAddressOperation, PhysicalOperandFootprint,
    PostAllocationMachineBlock, PostAllocationMachineFunction, PostAllocationMachineIdentity,
    PostAllocationMachineInstruction, PostAllocationMachinePlan, post_allocation_machine_identity,
};

use super::PostAllocationMachineDecodeError;

const HEADER_IDENTITY_OFFSET: usize = 12;
const SELECTED_OFFSET: usize = 44;
const TARGET_OFFSET: usize = 236;
const CHOICE_RULE_OFFSET: usize = 382;
const MACHINE_OFFSET: usize = 391;
const ALTERNATIVE_FAMILY_OFFSET: usize = 439;
const OPERAND_ACCESS_OFFSET: usize = 540;
const WRITE_SEMANTICS_PRESENCE_OFFSET: usize = 571;
const WRITE_SEMANTICS_OFFSET: usize = 572;

fn identity(byte: u8) -> [u8; 32] {
    [byte; 32]
}

fn plan() -> PostAllocationMachinePlan {
    let mut plan = PostAllocationMachinePlan {
        identity: PostAllocationMachineIdentity::from_bytes([0; 32]),
        selected: SelectedInstructionPlanIdentity::from_bytes(identity(1)),
        effects: PreAllocationMachineEffectIdentity::from_bytes(identity(2)),
        ranges: LiveRangeIdentity::from_bytes(identity(3)),
        legality: AllocationLegalityIdentity::from_bytes(identity(4)),
        homes: RegisterHomeIdentity::from_bytes(identity(5)),
        post_allocation_manifest: PostAllocationOptimizationManifestIdentity::from_bytes(identity(
            6,
        )),
        target: NativeTarget::linux_x64(),
        register_environment: TargetRegisterEnvironmentIdentity::from_bytes(identity(7)),
        physical_register_model: PhysicalRegisterModelIdentity::from_bytes(identity(8)),
        register_constraints: RegisterConstraintCatalogIdentity::from_bytes(identity(9)),
        machine_effect_catalog: MachineEffectCatalogIdentity::from_bytes(identity(10)),
        choice_rule: MachineAlternativeChoiceRule::UniqueApplicableInCatalogOrderV1,
        functions: vec![PostAllocationMachineFunction {
            machine: MachineId::new(17).unwrap(),
            outgoing_arguments: vec![],
            local_storage_slots: Vec::new(),
            blocks: vec![PostAllocationMachineBlock {
                block: SelectedBlockId(23),
                instructions: vec![PostAllocationMachineInstruction {
                    instruction: SelectedInstructionId(29),
                    address: None,
                    alternative: MachineAlternative {
                        key: MachineAlternativeKey {
                            family: MachineAlternativeFamily::ExactSubtractI64Immediate,
                            variant: 31,
                        },
                        applicability: MachineAlternativeApplicability::ResultAliasesOperand {
                            result: 2,
                            operand: 0,
                        },
                        size: MachineSizeKnowledge::EncoderResolved {
                            minimum_bytes: 3,
                            maximum_bytes: Some(7),
                        },
                        latency: MachineLatencyKnowledge::StableBaselineUnavailable,
                        encoded: MachineEncodedEffects {
                            external_operand_reads: vec![0, 1],
                            external_operand_writes: vec![2],
                            implicit_unit_uses: vec![RegisterUnitId(37)],
                            implicit_unit_defs: vec![RegisterUnitId(41)],
                            implicit_unit_clobbers: vec![RegisterUnitId(43)],
                            memory: MachineEncodedMemoryEffect::ReadActivationStackV1 {
                                stack_pointer: RegisterViewId(47),
                                byte_count: 8,
                            },
                            stack: MachineEncodedStackEffect::PopBytesV1 {
                                stack_pointer: RegisterViewId(47),
                                byte_count: 8,
                            },
                            trap: MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
                            control: MachineEncodedControlEffect::ReturnIndirectRegisterV1 {
                                target: RegisterViewId(53),
                            },
                        },
                    },
                    operands: vec![PhysicalOperandFootprint {
                        operand: 2,
                        virtual_register: VirtualRegisterId(59),
                        class: RegisterClassId(61),
                        view: RegisterViewId(67),
                        access: RegisterOperandAccess::UseDef,
                        storage_units: vec![RegisterUnitId(71)],
                        read_units: vec![RegisterUnitId(73)],
                        write_units: vec![RegisterUnitId(79)],
                        write_semantics: Some(RegisterWriteSemantics::ZeroExtendsParent),
                    }],
                    implicit_unit_uses: vec![RegisterUnitId(83)],
                    implicit_unit_defs: vec![RegisterUnitId(89)],
                    implicit_unit_clobbers: vec![RegisterUnitId(97)],
                    unit_uses: vec![RegisterUnitId(101)],
                    unit_defs: vec![RegisterUnitId(103)],
                    unit_clobbers: vec![RegisterUnitId(107)],
                }],
            }],
        }],
    };
    plan.identity = post_allocation_machine_identity(&plan);
    plan
}

#[test]
fn post_allocation_codec_is_deterministic_and_round_trips_every_field() {
    let plan = plan();
    let first = plan.encode();
    let second = plan.encode();

    assert_eq!(first, second);
    assert_eq!(PostAllocationMachinePlan::decode(&first), Ok(plan));
}

#[test]
fn physical_codec_retains_byte_view_address_family_not_exact_add() {
    let mut source = plan();
    source.functions[0].blocks[0].instructions[0]
        .alternative
        .key
        .family = MachineAlternativeFamily::ByteViewAddress;
    source.identity = post_allocation_machine_identity(&source);
    assert_eq!(
        PostAllocationMachinePlan::decode(&source.encode()),
        Ok(source.clone())
    );
    source.functions[0].blocks[0].instructions[0]
        .alternative
        .key
        .family = MachineAlternativeFamily::ExactAddI64;
    assert_eq!(
        PostAllocationMachinePlan::decode(&source.encode()),
        Err(PostAllocationMachineDecodeError::InvalidIdentity)
    );
}

#[test]
fn physical_current_format_rejects_all_retired_versions() {
    let encoded = plan().encode();
    for version in 0..17_u32 {
        let mut stale = encoded.clone();
        stale[8..12].copy_from_slice(&version.to_le_bytes());
        assert_eq!(
            PostAllocationMachinePlan::decode(&stale),
            Err(PostAllocationMachineDecodeError::UnsupportedVersion(
                version
            ))
        );
    }
}

#[test]
fn physical_codec_retains_owned_entry_homes_and_rejects_stale_or_absent_identity() {
    use selected_instructions::{FrameStorageSlotId, LocalStorageSlotId, SelectedLocalStorageSlot};
    use semantic_vocabulary::PlaceId;

    let mut source = plan();
    let place = PlaceId::new(139).unwrap();
    let slot = LocalStorageSlotId::StructuralParameter { place };
    source.functions[0]
        .local_storage_slots
        .push(SelectedLocalStorageSlot {
            id: slot,
            byte_size: 16,
            alignment: 8,
        });
    for address in [
        PhysicalAddressOperation::FrameAddress {
            slot: FrameStorageSlotId::Local(slot),
            byte_offset: 0,
        },
        PhysicalAddressOperation::Store64 {
            slot: FrameStorageSlotId::Local(slot),
            byte_offset: 8,
        },
    ] {
        source.functions[0].blocks[0].instructions[0].address = Some(address);
        source.identity = post_allocation_machine_identity(&source);
        let encoded = source.encode();
        assert_eq!(u32::from_le_bytes(encoded[8..12].try_into().unwrap()), 17);
        assert_eq!(
            PostAllocationMachinePlan::decode(&encoded),
            Ok(source.clone())
        );

        let mut stale = encoded.clone();
        stale[8..12].copy_from_slice(&16_u32.to_le_bytes());
        assert_eq!(
            PostAllocationMachinePlan::decode(&stale),
            Err(PostAllocationMachineDecodeError::UnsupportedVersion(16))
        );

        // The function machine and local-slot count precede the first tagged slot.
        let slot_offset = MACHINE_OFFSET + 8 + 8;
        assert_eq!(encoded[slot_offset], 4);
        let mut absent = encoded;
        absent[slot_offset + 1..slot_offset + 9].fill(0);
        assert_eq!(
            PostAllocationMachinePlan::decode(&absent),
            Err(PostAllocationMachineDecodeError::InvalidField)
        );

        let mut substituted = source.clone();
        substituted.functions[0].local_storage_slots[0].id = LocalStorageSlotId::Structural {
            operation: OperationId::new(137).unwrap(),
            place,
        };
        assert_eq!(
            PostAllocationMachinePlan::decode(&substituted.encode()),
            Err(PostAllocationMachineDecodeError::InvalidIdentity)
        );
    }
}

#[test]
fn post_allocation_codec_rejects_bad_framing_and_closed_field_tags() {
    let encoded = plan().encode();
    assert_eq!(encoded[TARGET_OFFSET], 1);
    assert_eq!(encoded[CHOICE_RULE_OFFSET], 0);
    assert_eq!(encoded[ALTERNATIVE_FAMILY_OFFSET], 8);
    assert_eq!(encoded[OPERAND_ACCESS_OFFSET], 2);
    assert_eq!(encoded[WRITE_SEMANTICS_PRESENCE_OFFSET], 1);
    assert_eq!(encoded[WRITE_SEMANTICS_OFFSET], 2);

    let mut wrong_magic = encoded.clone();
    wrong_magic[0] ^= 0xff;
    assert_eq!(
        PostAllocationMachinePlan::decode(&wrong_magic),
        Err(PostAllocationMachineDecodeError::WrongMagic)
    );

    let current_version = u32::from_le_bytes(encoded[8..12].try_into().unwrap());
    for version in [current_version - 1, current_version.checked_add(1).unwrap()] {
        let mut unsupported_version = encoded.clone();
        unsupported_version[8..12].copy_from_slice(&version.to_le_bytes());
        assert_eq!(
            PostAllocationMachinePlan::decode(&unsupported_version),
            Err(PostAllocationMachineDecodeError::UnsupportedVersion(
                version
            ))
        );
    }

    for offset in [
        TARGET_OFFSET,
        CHOICE_RULE_OFFSET,
        ALTERNATIVE_FAMILY_OFFSET,
        OPERAND_ACCESS_OFFSET,
        WRITE_SEMANTICS_PRESENCE_OFFSET,
        WRITE_SEMANTICS_OFFSET,
    ] {
        let mut invalid = encoded.clone();
        invalid[offset] = 0xff;
        assert_eq!(
            PostAllocationMachinePlan::decode(&invalid),
            Err(PostAllocationMachineDecodeError::InvalidField),
            "closed field at byte {offset} was accepted"
        );
    }

    let mut zero_machine = encoded.clone();
    zero_machine[MACHINE_OFFSET..MACHINE_OFFSET + 8].fill(0);
    assert_eq!(
        PostAllocationMachinePlan::decode(&zero_machine),
        Err(PostAllocationMachineDecodeError::InvalidField)
    );

    assert_eq!(
        PostAllocationMachinePlan::decode(&encoded[..encoded.len() - 1]),
        Err(PostAllocationMachineDecodeError::Truncated)
    );

    let mut trailing = encoded;
    trailing.push(0);
    assert_eq!(
        PostAllocationMachinePlan::decode(&trailing),
        Err(PostAllocationMachineDecodeError::TrailingBytes)
    );
}

#[test]
fn post_allocation_codec_authenticates_header_and_all_content_roots() {
    let encoded = plan().encode();
    let mut root_offsets = vec![HEADER_IDENTITY_OFFSET];
    root_offsets.extend((0..6).map(|index| SELECTED_OFFSET + index * 32));
    root_offsets.extend((0..4).map(|index| 254 + index * 32));

    for offset in root_offsets {
        let mut corrupted = encoded.clone();
        corrupted[offset] ^= 0x80;
        assert_eq!(
            PostAllocationMachinePlan::decode(&corrupted),
            Err(PostAllocationMachineDecodeError::InvalidIdentity),
            "identity-bearing bytes at offset {offset} were accepted"
        );
    }
}

#[test]
fn physical_codec_binds_symbolic_address_roles_and_outgoing_geometry() {
    let mut source = plan();
    let slot = OutgoingArgumentSlotId {
        operation: OperationId::new(131).unwrap(),
        argument_index: 1,
    };
    source.functions[0]
        .outgoing_arguments
        .push(SelectedOutgoingArgumentSlot {
            id: slot,
            byte_size: 16,
            alignment: 8,
            abi_stack_byte_offset: 48,
        });
    for address in [
        PhysicalAddressOperation::FrameAddress {
            slot: selected_instructions::FrameStorageSlotId::Incoming {
                parameter_index: 8,
                abi_stack_byte_offset: 16,
            },
            byte_offset: 0,
        },
        PhysicalAddressOperation::Store {
            base_operand: 0,
            byte_offset: 2,
            byte_size: 2,
        },
        PhysicalAddressOperation::AddressOffset {
            base_operand: 0,
            byte_offset: 2,
        },
        PhysicalAddressOperation::Load8Indexed {
            base_operand: 0,
            index_operand: 1,
        },
        PhysicalAddressOperation::Load64 {
            base_operand: 0,
            byte_offset: 8,
        },
        PhysicalAddressOperation::Load8 {
            base_operand: 0,
            byte_offset: 12,
        },
        PhysicalAddressOperation::Load16 {
            base_operand: 0,
            byte_offset: 12,
        },
        PhysicalAddressOperation::Load32 {
            base_operand: 0,
            byte_offset: 12,
        },
        PhysicalAddressOperation::Store64 {
            slot: selected_instructions::FrameStorageSlotId::Outgoing(slot),
            byte_offset: 8,
        },
        PhysicalAddressOperation::FrameAddress {
            slot: selected_instructions::FrameStorageSlotId::Outgoing(slot),
            byte_offset: 0,
        },
    ] {
        source.functions[0].blocks[0].instructions[0].address = Some(address);
        source.identity = post_allocation_machine_identity(&source);
        assert_eq!(
            PostAllocationMachinePlan::decode(&source.encode()),
            Ok(source.clone())
        );
        let mut changed = source.clone();
        changed.functions[0].blocks[0].instructions[0].address = None;
        assert_eq!(
            PostAllocationMachinePlan::decode(&changed.encode()),
            Err(PostAllocationMachineDecodeError::InvalidIdentity)
        );
        let mut changed = source.clone();
        changed.functions[0].outgoing_arguments[0].abi_stack_byte_offset += 8;
        assert_eq!(
            PostAllocationMachinePlan::decode(&changed.encode()),
            Err(PostAllocationMachineDecodeError::InvalidIdentity)
        );
        let mut changed = source.clone();
        changed.functions[0].outgoing_arguments[0].id.argument_index += 1;
        assert_eq!(
            PostAllocationMachinePlan::decode(&changed.encode()),
            Err(PostAllocationMachineDecodeError::InvalidIdentity)
        );
    }
}

#[test]
fn boundary_scratch_codec_binds_tag_operation_geometry_and_address() {
    let mut source = plan();
    let slot = selected_instructions::LocalStorageSlotId::Boundary {
        operation: OperationId::new(137).unwrap(),
    };
    source.functions[0]
        .local_storage_slots
        .push(selected_instructions::SelectedLocalStorageSlot {
            id: slot,
            byte_size: 1,
            alignment: 1,
        });
    source.functions[0].blocks[0].instructions[0].address =
        Some(PhysicalAddressOperation::HostedWriteByteI32 { slot });
    source.identity = post_allocation_machine_identity(&source);
    assert_eq!(
        PostAllocationMachinePlan::decode(&source.encode()),
        Ok(source.clone())
    );
    for mutation in 0..4 {
        let mut changed = source.clone();
        match mutation {
            0 => {
                changed.functions[0].local_storage_slots[0].id =
                    selected_instructions::LocalStorageSlotId::Structural {
                        operation: slot.operation().expect("source-backed local slot"),
                        place: semantic_vocabulary::PlaceId::new(139).unwrap(),
                    }
            }
            1 => changed.functions[0].local_storage_slots[0].byte_size = 2,
            2 => changed.functions[0].local_storage_slots[0].alignment = 2,
            _ => {
                changed.functions[0].blocks[0].instructions[0].address =
                    Some(PhysicalAddressOperation::HostedWriteByteI32 {
                        slot: selected_instructions::LocalStorageSlotId::Boundary {
                            operation: OperationId::new(149).unwrap(),
                        },
                    })
            }
        }
        assert_eq!(
            PostAllocationMachinePlan::decode(&changed.encode()),
            Err(PostAllocationMachineDecodeError::InvalidIdentity)
        );
    }
}

#[test]
fn read_byte_codec_binds_structural_home_operation_geometry_and_address() {
    let mut source = plan();
    let slot = selected_instructions::LocalStorageSlotId::Structural {
        place: semantic_vocabulary::PlaceId::new(139).unwrap(),
        operation: OperationId::new(137).unwrap(),
    };
    source.functions[0]
        .local_storage_slots
        .push(selected_instructions::SelectedLocalStorageSlot {
            id: slot,
            byte_size: 8,
            alignment: 4,
        });
    source.functions[0].blocks[0].instructions[0].address =
        Some(PhysicalAddressOperation::HostedReadByte { slot });
    source.identity = post_allocation_machine_identity(&source);
    assert_eq!(
        PostAllocationMachinePlan::decode(&source.encode()),
        Ok(source.clone())
    );
    for mutation in 0..4 {
        let mut changed = source.clone();
        match mutation {
            0 => {
                changed.functions[0].local_storage_slots[0].id =
                    selected_instructions::LocalStorageSlotId::Boundary {
                        operation: slot.operation().expect("source-backed local slot"),
                    }
            }
            1 => changed.functions[0].local_storage_slots[0].byte_size = 2,
            2 => changed.functions[0].local_storage_slots[0].alignment = 2,
            _ => {
                changed.functions[0].blocks[0].instructions[0].address =
                    Some(PhysicalAddressOperation::HostedReadByte {
                        slot: selected_instructions::LocalStorageSlotId::Boundary {
                            operation: OperationId::new(149).unwrap(),
                        },
                    })
            }
        }
        assert_eq!(
            PostAllocationMachinePlan::decode(&changed.encode()),
            Err(PostAllocationMachineDecodeError::InvalidIdentity)
        );
    }
}

#[test]
fn pointer_store_codec_binds_exact_width_and_rejects_unsupported_widths() {
    for byte_size in [1, 2, 4, 8] {
        let mut source = plan();
        source.functions[0].blocks[0].instructions[0].address =
            Some(PhysicalAddressOperation::Store {
                base_operand: 0,
                byte_offset: 16,
                byte_size,
            });
        source.identity = post_allocation_machine_identity(&source);
        assert_eq!(
            PostAllocationMachinePlan::decode(&source.encode()),
            Ok(source.clone())
        );
        for changed_address in [
            PhysicalAddressOperation::Store {
                base_operand: 1,
                byte_offset: 16,
                byte_size,
            },
            PhysicalAddressOperation::Store {
                base_operand: 0,
                byte_offset: 24,
                byte_size,
            },
            PhysicalAddressOperation::Store {
                base_operand: 0,
                byte_offset: 16,
                byte_size: if byte_size == 8 { 4 } else { 8 },
            },
        ] {
            let mut changed = source.clone();
            changed.functions[0].blocks[0].instructions[0].address = Some(changed_address);
            assert_eq!(
                PostAllocationMachinePlan::decode(&changed.encode()),
                Err(PostAllocationMachineDecodeError::InvalidIdentity)
            );
        }
    }
    for byte_size in [0, 3, 5, 16, 255] {
        let mut invalid = plan();
        invalid.functions[0].blocks[0].instructions[0].address =
            Some(PhysicalAddressOperation::Store {
                base_operand: 0,
                byte_offset: 0,
                byte_size,
            });
        invalid.identity = post_allocation_machine_identity(&invalid);
        assert_eq!(
            PostAllocationMachinePlan::decode(&invalid.encode()),
            Err(PostAllocationMachineDecodeError::InvalidField)
        );
    }
}

#[test]
fn physical_codec_binds_activation_local_geometry_and_source_identity() {
    let mut source = plan();
    let slot = selected_instructions::LocalStorageSlotId::Structural {
        operation: OperationId::new(137).unwrap(),
        place: semantic_vocabulary::PlaceId::new(139).unwrap(),
    };
    source.functions[0]
        .local_storage_slots
        .push(selected_instructions::SelectedLocalStorageSlot {
            id: slot,
            byte_size: 24,
            alignment: 8,
        });
    for address in [
        PhysicalAddressOperation::FrameAddress {
            slot: selected_instructions::FrameStorageSlotId::Incoming {
                parameter_index: 8,
                abi_stack_byte_offset: 16,
            },
            byte_offset: 0,
        },
        PhysicalAddressOperation::Store64 {
            slot: selected_instructions::FrameStorageSlotId::Local(slot),
            byte_offset: 16,
        },
        PhysicalAddressOperation::FrameAddress {
            slot: selected_instructions::FrameStorageSlotId::Local(slot),
            byte_offset: 0,
        },
    ] {
        source.functions[0].blocks[0].instructions[0].address = Some(address);
        source.identity = post_allocation_machine_identity(&source);
        assert_eq!(
            PostAllocationMachinePlan::decode(&source.encode()),
            Ok(source.clone())
        );
        for field in 0..4 {
            let mut changed = source.clone();
            let local = &mut changed.functions[0].local_storage_slots[0];
            match field {
                0 => {
                    local.id = selected_instructions::LocalStorageSlotId::Structural {
                        operation: OperationId::new(149).unwrap(),
                        place: local.id.structural_place().unwrap(),
                    }
                }
                1 => {
                    local.id = selected_instructions::LocalStorageSlotId::Structural {
                        operation: local.id.operation().expect("source-backed local slot"),
                        place: semantic_vocabulary::PlaceId::new(151).unwrap(),
                    }
                }
                2 => local.byte_size += 8,
                _ => local.alignment = 4,
            }
            assert_eq!(
                PostAllocationMachinePlan::decode(&changed.encode()),
                Err(PostAllocationMachineDecodeError::InvalidIdentity)
            );
        }
    }
}

#[test]
fn every_truncated_physical_frame_rejects() {
    let encoded = plan().encode();
    for end in 0..encoded.len() {
        assert!(
            PostAllocationMachinePlan::decode(&encoded[..end]).is_err(),
            "truncated frame at byte {end} was accepted"
        );
    }
}

#[test]
fn reauthenticated_physical_data_is_still_only_a_proposal() {
    let mut substituted = plan();
    substituted.functions[0].blocks[0].instructions[0]
        .alternative
        .key
        .variant = u32::MAX;
    substituted.identity = post_allocation_machine_identity(&substituted);
    // Decoding is deliberately independent of a selected program and target
    // catalog. The consuming validator must reconstruct the chosen alternative.
    assert_eq!(
        PostAllocationMachinePlan::decode(&substituted.encode()),
        Ok(substituted)
    );
}

#[test]
fn physical_u32_normalization_round_trips_and_binds_its_family() {
    let mut source = plan();
    source.functions[0].blocks[0].instructions[0]
        .alternative
        .key
        .family = MachineAlternativeFamily::ZeroExtendU32;
    source.identity = post_allocation_machine_identity(&source);
    assert_eq!(
        PostAllocationMachinePlan::decode(&source.encode()),
        Ok(source.clone())
    );
    source.functions[0].blocks[0].instructions[0]
        .alternative
        .key
        .family = MachineAlternativeFamily::CopyI64;
    assert_eq!(
        PostAllocationMachinePlan::decode(&source.encode()),
        Err(PostAllocationMachineDecodeError::InvalidIdentity)
    );
}

#[test]
fn compiler_spill_slot_codec_retains_function_local_identity() {
    let mut source = plan();
    let slot = selected_instructions::LocalStorageSlotId::Spill {
        register: selected_instructions::VirtualRegisterId(7),
    };
    source.functions[0]
        .local_storage_slots
        .push(selected_instructions::SelectedLocalStorageSlot {
            id: slot,
            byte_size: 8,
            alignment: 8,
        });
    for address in [
        PhysicalAddressOperation::FrameAddress {
            slot: selected_instructions::FrameStorageSlotId::Local(slot),
            byte_offset: 0,
        },
        PhysicalAddressOperation::Store64 {
            slot: selected_instructions::FrameStorageSlotId::Local(slot),
            byte_offset: 0,
        },
    ] {
        source.functions[0].blocks[0].instructions[0].address = Some(address);
        source.identity = post_allocation_machine_identity(&source);
        assert_eq!(
            PostAllocationMachinePlan::decode(&source.encode()),
            Ok(source.clone())
        );
        let mut changed = source.clone();
        changed.functions[0].local_storage_slots[0].id =
            selected_instructions::LocalStorageSlotId::Spill {
                register: selected_instructions::VirtualRegisterId(8),
            };
        assert_ne!(post_allocation_machine_identity(&changed), source.identity);
        assert!(PostAllocationMachinePlan::decode(&changed.encode()).is_err());
    }
}

#[test]
fn physical_codec_retains_narrow_load_families_and_exact_addresses() {
    for (family, address, byte_count) in [
        (
            MachineAlternativeFamily::Load8,
            PhysicalAddressOperation::Load8 {
                base_operand: 0,
                byte_offset: 3,
            },
            1,
        ),
        (
            MachineAlternativeFamily::Load16,
            PhysicalAddressOperation::Load16 {
                base_operand: 0,
                byte_offset: 6,
            },
            2,
        ),
    ] {
        let mut source = plan();
        let instruction = &mut source.functions[0].blocks[0].instructions[0];
        instruction.alternative.key.family = family;
        instruction.address = Some(address);
        instruction.alternative.encoded.memory = MachineEncodedMemoryEffect::ReadPointerV1 {
            pointer_operand: 0,
            byte_count,
        };
        source.identity = post_allocation_machine_identity(&source);
        assert_eq!(
            PostAllocationMachinePlan::decode(&source.encode()),
            Ok(source.clone())
        );
        for mutation in 0..3 {
            let mut changed = source.clone();
            let instruction = &mut changed.functions[0].blocks[0].instructions[0];
            match mutation {
                0 => instruction.alternative.key.family = MachineAlternativeFamily::Load64,
                1 => {
                    instruction.address = Some(PhysicalAddressOperation::Load64 {
                        base_operand: 0,
                        byte_offset: 3,
                    })
                }
                _ => {
                    instruction.alternative.encoded.memory =
                        MachineEncodedMemoryEffect::ReadPointerV1 {
                            pointer_operand: 0,
                            byte_count: 8,
                        }
                }
            }
            assert_ne!(source.identity, post_allocation_machine_identity(&changed));
            assert!(PostAllocationMachinePlan::decode(&changed.encode()).is_err());
        }
    }
}
