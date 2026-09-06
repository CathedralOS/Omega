//! Round-trip, closed-tag, framing, and content-identity tests.

use optimization_core::PostAllocationOptimizationManifestIdentity;
use optimization_unit::{EffectLink, OwnershipEvent};
use register_homes::{AllocationLegalityIdentity, LiveRangeIdentity, RegisterHomeIdentity};
use register_model::{
    PhysicalRegisterModelIdentity, RegisterClassId, RegisterConstraintCatalogIdentity,
    RegisterConstraintFamily, RegisterConstraintKey, RegisterOperandAccess, RegisterUnitId,
    RegisterViewId, RegisterWriteSemantics, TargetRegisterEnvironmentIdentity,
};
use selected_instructions::{
    MachineAlternative, MachineAlternativeApplicability, MachineAlternativeFamily,
    MachineAlternativeKey, MachineEffectCatalogIdentity, MachineEncodedControlEffect,
    MachineEncodedEffects, MachineEncodedMemoryEffect, MachineEncodedStackEffect,
    MachineEncodedTrapBehavior, MachineLatencyKnowledge, MachineSizeKnowledge, SelectedBlockId,
    SelectedInstructionId, SelectedInstructionPlanIdentity, SelectedInstructionProvenance,
    SelectedMicrosoftX64OwnedIndirectPairLayout, SelectedStructuralUnitIndirectBinding,
    StructuralUnitCallBarrier, StructuralUnitCallEffect, StructuralUnitCallEffectDeclaration,
    StructuralUnitCallFrameEffect, StructuralUnitCallMemoryEffect, VirtualRegisterId,
};
use selected_instructions::{PreAllocationMachineEffectIdentity, StructuralUnitCallMachineEffects};
use semantic_vocabulary::{ClaimId, MachineId, OperationId};
use target::NativeTarget;
use target_operations::MachineRegister;

use crate::{
    MachineAlternativeChoiceRule, PhysicalOperandFootprint, PostAllocationMachineBlock,
    PostAllocationMachineFunction, PostAllocationMachineIdentity, PostAllocationMachineInstruction,
    PostAllocationMachinePlan, PostAllocationStructuralUnitFunction,
    post_allocation_machine_identity,
};

use super::PostAllocationMachineDecodeError;

const HEADER_IDENTITY_OFFSET: usize = 12;
const SELECTED_OFFSET: usize = 44;
const TARGET_OFFSET: usize = 236;
const CHOICE_RULE_OFFSET: usize = 382;
const MACHINE_OFFSET: usize = 391;
const ALTERNATIVE_FAMILY_OFFSET: usize = 423;
const OPERAND_ACCESS_OFFSET: usize = 524;
const WRITE_SEMANTICS_PRESENCE_OFFSET: usize = 555;
const WRITE_SEMANTICS_OFFSET: usize = 556;

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
            blocks: vec![PostAllocationMachineBlock {
                block: SelectedBlockId(23),
                instructions: vec![PostAllocationMachineInstruction {
                    instruction: SelectedInstructionId(29),
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
        structural_unit_functions: Vec::new(),
    };
    let return_instruction = plan.functions[0].blocks[0].instructions[0].clone();
    let call_constraint = RegisterConstraintKey {
        family: RegisterConstraintFamily::Call,
        variant: 109,
    };
    plan.structural_unit_functions
        .push(PostAllocationStructuralUnitFunction {
            machine: MachineId::new(113).unwrap(),
            block: SelectedBlockId(127),
            call: Some(StructuralUnitCallMachineEffects {
                instruction: SelectedInstructionId(0),
                operation: OperationId::new(131).unwrap(),
                callee: MachineId::new(137).unwrap(),
                constraint: call_constraint,
                unit_uses: vec![RegisterUnitId(139), RegisterUnitId(149)],
                unit_defs: vec![RegisterUnitId(151)],
                unit_clobbers: vec![RegisterUnitId(157)],
                layout: SelectedMicrosoftX64OwnedIndirectPairLayout {
                    shadow_byte_count: 32,
                    outgoing_frame_byte_count: 72,
                    pre_call_stack_alignment: 16,
                    bindings: [
                        SelectedStructuralUnitIndirectBinding {
                            parameter_index: 0,
                            pointer: MachineRegister::X86Rcx,
                            copy_stack_byte_offset: 32,
                            byte_count: 16,
                            alignment: 8,
                        },
                        SelectedStructuralUnitIndirectBinding {
                            parameter_index: 1,
                            pointer: MachineRegister::X86Rdx,
                            copy_stack_byte_offset: 48,
                            byte_count: 16,
                            alignment: 8,
                        },
                    ],
                },
                effect: EffectLink {
                    input: 163,
                    output: 167,
                },
                ownership: vec![OwnershipEvent::ClaimTransfer(vec![
                    ClaimId::new(173).unwrap(),
                ])],
                claim_transfers: vec![terminal_psi::ClaimTransfer {
                    claim: ClaimId::new(173).unwrap(),
                    argument_index: 0,
                }],
                provenance: SelectedInstructionProvenance {
                    operations: vec![OperationId::new(131).unwrap()],
                    ..Default::default()
                },
                declaration: StructuralUnitCallEffectDeclaration {
                    constraint: call_constraint,
                    memory:
                        StructuralUnitCallMemoryEffect::ReadOwnedIndirectPairWriteCallerCopiesV1 {
                            root_byte_count: 16,
                            copy_stack_byte_offsets: [32, 48],
                        },
                    frame: StructuralUnitCallFrameEffect::BalancedCallerFrameV1 {
                        frame_byte_count: 72,
                        shadow_byte_count: 32,
                        pre_call_stack_alignment: 16,
                    },
                    trap: selected_instructions::MachineTrapBehavior::MayArchitecturalFaultV1,
                    barrier: StructuralUnitCallBarrier::CallV1,
                    call: StructuralUnitCallEffect::DirectInternalUnitV1,
                    cleanup: selected_instructions::MachineCleanupEffect::NoneV1,
                },
            }),
            return_instruction,
            return_provenance: SelectedInstructionProvenance::default(),
            return_effect: EffectLink {
                input: 179,
                output: 181,
            },
            return_ownership: vec![OwnershipEvent::StructuralReturn(vec![
                ClaimId::new(191).unwrap(),
            ])],
        });
    plan.identity = post_allocation_machine_identity(&plan);
    plan
}

#[test]
fn post_allocation_codec_is_deterministic_and_round_trips_every_field() {
    let plan = plan();
    let first = plan.encode();
    let second = plan.encode();

    use sha2::{Digest, Sha256};
    // Captured from the original optimizer-owned version-5 encoder.
    let mut legacy = first.clone();
    legacy[8..12].copy_from_slice(&5_u32.to_le_bytes());
    assert_eq!(first.len(), 1158);
    assert_eq!(
        format!("{:x}", Sha256::digest(&legacy)),
        "27ad65f5b9d1a58bc3972c51d65e25bd01ef242b3262de4ee430d7c603cb254d"
    );
    assert_eq!(
        plan.identity.bytes(),
        [
            0x08, 0x78, 0xaa, 0x6a, 0xc9, 0xe3, 0x40, 0x08, 0x6d, 0x78, 0x32, 0xb1, 0x15, 0xdc,
            0x22, 0x27, 0xd9, 0x6e, 0x40, 0x56, 0x0c, 0xfc, 0xaa, 0xcc, 0xf1, 0xcf, 0x40, 0x11,
            0x69, 0x69, 0x05, 0x26,
        ]
    );

    assert_eq!(first, second);
    assert_eq!(PostAllocationMachinePlan::decode(&legacy), Ok(plan.clone()));
    assert_eq!(PostAllocationMachinePlan::decode(&first), Ok(plan));
}

#[test]
fn post_allocation_jump_requires_v6_vocabulary() {
    let mut plan = plan();
    let alternative = &mut plan.functions[0].blocks[0].instructions[0].alternative;
    alternative.key.family = MachineAlternativeFamily::Jump;
    alternative.encoded.control = MachineEncodedControlEffect::UnconditionalRelativeBranchV1;
    plan.identity = post_allocation_machine_identity(&plan);
    let mut encoded = plan.encode();
    assert_eq!(PostAllocationMachinePlan::decode(&encoded), Ok(plan));
    encoded[8..12].copy_from_slice(&5_u32.to_le_bytes());
    assert_eq!(
        PostAllocationMachinePlan::decode(&encoded),
        Err(PostAllocationMachineDecodeError::InvalidField)
    );
}

#[test]
fn post_allocation_v4_round_trips_signed_branch_family_and_v3_rejects_it() {
    let mut plan = plan();
    plan.functions[0].blocks[0].instructions[0]
        .alternative
        .key
        .family = MachineAlternativeFamily::ConditionalBranchI64LessThan;
    plan.identity = post_allocation_machine_identity(&plan);
    assert_eq!(
        PostAllocationMachinePlan::decode(&plan.encode()),
        Ok(plan.clone())
    );

    plan.identity = super::super::identity::post_allocation_machine_identity_v4_legacy(&plan);
    let content = super::super::identity::encode_terminal_post_allocation_machine_content(&plan);
    let mut encoded = Vec::new();
    encoded.extend_from_slice(super::MAGIC);
    encoded.extend_from_slice(&3_u32.to_le_bytes());
    encoded.extend_from_slice(&plan.identity.bytes());
    encoded.extend_from_slice(&content);
    assert_eq!(
        PostAllocationMachinePlan::decode(&encoded),
        Err(PostAllocationMachineDecodeError::InvalidField)
    );
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

    let mut unsupported_version = encoded.clone();
    unsupported_version[8..12].copy_from_slice(&7_u32.to_le_bytes());
    assert_eq!(
        PostAllocationMachinePlan::decode(&unsupported_version),
        Err(PostAllocationMachineDecodeError::UnsupportedVersion(7))
    );

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
fn post_allocation_codec_authenticates_structural_call_content() {
    let source = plan();
    let mut substituted = source.clone();
    substituted.structural_unit_functions[0]
        .call
        .as_mut()
        .unwrap()
        .layout
        .outgoing_frame_byte_count = 80;

    assert_ne!(
        post_allocation_machine_identity(&substituted),
        source.identity
    );
    assert_eq!(
        PostAllocationMachinePlan::decode(&substituted.encode()),
        Err(PostAllocationMachineDecodeError::InvalidIdentity)
    );

    let mut erased = source;
    erased.structural_unit_functions.clear();
    assert_ne!(post_allocation_machine_identity(&erased), plan().identity);
    assert_eq!(
        PostAllocationMachinePlan::decode(&erased.encode()),
        Err(PostAllocationMachineDecodeError::InvalidIdentity)
    );
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
