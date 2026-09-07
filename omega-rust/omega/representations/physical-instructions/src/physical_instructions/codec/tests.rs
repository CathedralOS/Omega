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
const ALTERNATIVE_FAMILY_OFFSET: usize = 431;
const OPERAND_ACCESS_OFFSET: usize = 532;
const WRITE_SEMANTICS_PRESENCE_OFFSET: usize = 563;
const WRITE_SEMANTICS_OFFSET: usize = 564;

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
fn physical_current_format_rejects_all_retired_versions() {
    let encoded = plan().encode();
    for version in [3_u32, 4, 5, 6, 7, 9] {
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
        PhysicalAddressOperation::Load8Indexed {
            base_operand: 0,
            index_operand: 1,
        },
        PhysicalAddressOperation::Load64 {
            base_operand: 0,
            byte_offset: 8,
        },
        PhysicalAddressOperation::Store64 {
            slot,
            byte_offset: 8,
        },
        PhysicalAddressOperation::FrameAddress {
            slot,
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
