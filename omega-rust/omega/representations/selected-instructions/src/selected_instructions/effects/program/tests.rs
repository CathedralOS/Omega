//! Pre-allocation machine-effect codec fixtures.

use crate::{
    MachineAlternative, MachineAlternativeApplicability, MachineAlternativeFamily,
    MachineAlternativeKey, MachineBarrier, MachineCallEffect, MachineCleanupEffect,
    MachineEffectCatalogIdentity, MachineEncodedControlEffect, MachineEncodedEffects,
    MachineEncodedMemoryEffect, MachineEncodedStackEffect, MachineEncodedTrapBehavior,
    MachineLatencyKnowledge, MachineMemoryEffect, MachineSizeKnowledge, MachineTrapBehavior,
    SelectedBlockId, SelectedInstructionId, SelectedInstructionKind,
    SelectedInstructionPlanIdentity, SelectedInstructionProvenance,
};
use optimization_core::OptimizationUnitIdentity;
use optimization_unit::{FuelSettlement, PsiProvenance};
use register_model::{
    RegisterConstraintCatalogIdentity, RegisterConstraintFamily, RegisterConstraintKey,
    RegisterUnitId, TargetRegisterEnvironmentIdentity,
};
use semantic_vocabulary::{
    EdgeId, FuelScheduleIdentity, MachineId, ObligationId, OperationId, ValueId,
};
use target::NativeTarget;

use super::encoding::*;
use super::*;

fn plan() -> PreAllocationMachineEffectPlan {
    let mut plan = PreAllocationMachineEffectPlan {
        identity: PreAllocationMachineEffectIdentity::from_bytes([0; 32]),
        selected: SelectedInstructionPlanIdentity::from_bytes([1; 32]),
        optimization_unit: OptimizationUnitIdentity::from_bytes([2; 32]),
        fuel_schedule: FuelScheduleIdentity::new(1).unwrap(),
        target: NativeTarget::linux_x64(),
        register_environment: TargetRegisterEnvironmentIdentity::from_bytes([3; 32]),
        register_constraints: RegisterConstraintCatalogIdentity::from_bytes([4; 32]),
        machine_effect_catalog: MachineEffectCatalogIdentity::from_bytes([5; 32]),
        functions: vec![FunctionMachineEffects {
            machine: MachineId::new(1).unwrap(),
            blocks: vec![BlockMachineEffects {
                block: SelectedBlockId(0),
                instructions: vec![InstructionMachineEffects {
                    instruction: SelectedInstructionId(0),
                    kind: SelectedInstructionKind::CompareI64,
                    constraint: RegisterConstraintKey {
                        family: RegisterConstraintFamily::Instruction,
                        variant: 4,
                    },
                    unit_uses: vec![RegisterUnitId(0)],
                    unit_defs: vec![RegisterUnitId(1)],
                    unit_clobbers: vec![RegisterUnitId(2)],
                    memory: MachineMemoryEffect::NoneV1,
                    trap: MachineTrapBehavior::NeverV1,
                    barrier: MachineBarrier::None,
                    call: MachineCallEffect::NoneV1,
                    cleanup: MachineCleanupEffect::NoneV1,
                    provenance: SelectedInstructionProvenance {
                        operations: vec![OperationId::new(2).unwrap()],
                        values: vec![ValueId::new(3).unwrap()],
                        edges: vec![EdgeId::new(4).unwrap()],
                        obligations: vec![ObligationId::new(5).unwrap()],
                        fuel: vec![
                            FuelSettlement {
                                site: PsiProvenance::Operation(OperationId::new(2).unwrap()),
                                units: 7,
                            },
                            FuelSettlement {
                                site: PsiProvenance::Edge(EdgeId::new(4).unwrap()),
                                units: 11,
                            },
                        ],
                    },
                    alternatives: vec![
                        MachineAlternative {
                            key: MachineAlternativeKey {
                                family: MachineAlternativeFamily::CompareI64,
                                variant: 0,
                            },
                            applicability: MachineAlternativeApplicability::Always,
                            size: MachineSizeKnowledge::ExactBytes(3),
                            latency:
                                MachineLatencyKnowledge::StableBaselineUnavailable,
                            encoded: MachineEncodedEffects::fallthrough_v1(
                                vec![0],
                                vec![],
                            ),
                        },
                        MachineAlternative {
                            key: MachineAlternativeKey {
                                family: MachineAlternativeFamily::CompareI64,
                                variant: 1,
                            },
                            applicability: MachineAlternativeApplicability::
                                ResultAliasesOperandAndDistinctFromOperand {
                                    result: 0,
                                    aliased_operand: 1,
                                    distinct_operand: 2,
                                },
                            size: MachineSizeKnowledge::EncoderResolved {
                                minimum_bytes: 2,
                                maximum_bytes: Some(6),
                            },
                            latency:
                                MachineLatencyKnowledge::StableBaselineUnavailable,
                            encoded: MachineEncodedEffects::fallthrough_v1(
                                vec![0, 1],
                                vec![2],
                            ),
                        },
                        MachineAlternative {
                            key: MachineAlternativeKey {
                                family: MachineAlternativeFamily::CompareI64,
                                variant: 2,
                            },
                            applicability: MachineAlternativeApplicability::
                                AtLeastOneOperandDoesNotAliasView {
                                    left: 0,
                                    right: 1,
                                    excluded_view: register_model::RegisterViewId(12),
                                },
                            size: MachineSizeKnowledge::ExactBytes(4),
                            latency:
                                MachineLatencyKnowledge::StableBaselineUnavailable,
                            encoded: MachineEncodedEffects {
                                external_operand_reads: vec![],
                                external_operand_writes: vec![],
                                implicit_unit_uses: vec![RegisterUnitId(0)],
                                implicit_unit_defs: vec![RegisterUnitId(1)],
                                implicit_unit_clobbers: vec![],
                                memory:
                                    MachineEncodedMemoryEffect::ReadActivationStackV1 {
                                        stack_pointer:
                                            register_model::RegisterViewId(12),
                                        byte_count: 8,
                                    },
                                stack: MachineEncodedStackEffect::PopBytesV1 {
                                    stack_pointer: register_model::RegisterViewId(12),
                                    byte_count: 8,
                                },
                                trap: MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
                                control: MachineEncodedControlEffect::ReturnFromActivationStackV1,
                            },
                        },
                    ],
                }],
            }],
        }],
    };
    let return_instruction = InstructionMachineEffects {
        instruction: SelectedInstructionId(1),
        kind: SelectedInstructionKind::ReturnUnit,
        constraint: RegisterConstraintKey {
            family: RegisterConstraintFamily::Return,
            variant: 3,
        },
        unit_uses: vec![RegisterUnitId(4)],
        unit_defs: vec![RegisterUnitId(4), RegisterUnitId(5)],
        unit_clobbers: Vec::new(),
        memory: MachineMemoryEffect::NoneV1,
        trap: MachineTrapBehavior::NeverV1,
        barrier: MachineBarrier::ControlFlow,
        call: MachineCallEffect::NoneV1,
        cleanup: MachineCleanupEffect::NoneV1,
        provenance: SelectedInstructionProvenance::default(),
        alternatives: Vec::new(),
    };
    let call_constraint = RegisterConstraintKey {
        family: RegisterConstraintFamily::Call,
        variant: 2,
    };
    plan.functions.push(FunctionMachineEffects {
        machine: MachineId::new(6).unwrap(),
        blocks: vec![BlockMachineEffects {
            block: SelectedBlockId(0),
            instructions: vec![
                InstructionMachineEffects {
                    instruction: SelectedInstructionId(0),
                    kind: SelectedInstructionKind::CallUnit {
                        callee: MachineId::new(8).unwrap(),
                    },
                    constraint: call_constraint,
                    unit_uses: vec![RegisterUnitId(1), RegisterUnitId(2)],
                    unit_defs: vec![RegisterUnitId(3)],
                    unit_clobbers: vec![RegisterUnitId(4)],
                    memory: MachineMemoryEffect::NoneV1,
                    trap: MachineTrapBehavior::MayArchitecturalFaultV1,
                    barrier: MachineBarrier::Call,
                    call: MachineCallEffect::DirectInternalNormalReturnV1 {
                        pre_call_stack_alignment: 16,
                    },
                    cleanup: MachineCleanupEffect::NoneV1,
                    provenance: SelectedInstructionProvenance {
                        operations: vec![OperationId::new(7).unwrap()],
                        ..Default::default()
                    },
                    alternatives: Vec::new(),
                },
                return_instruction,
            ],
        }],
    });
    plan.identity = pre_allocation_machine_effect_identity(&plan);
    plan
}

#[test]
fn codec_keeps_linux_output_pointer_store_and_address_offset_distinct() {
    let mut source = plan();
    let original = source.functions[0].blocks[0].instructions[0].clone();
    let rows = &mut source.functions[0].blocks[0].instructions;
    rows.clear();
    for position in 0..3 {
        let mut row = original.clone();
        row.instruction = crate::SelectedInstructionId(position);
        row.alternatives.truncate(1);
        let alternative = &mut row.alternatives[0];
        match position {
            0 => {
                row.kind = SelectedInstructionKind::HostedWriteByteI32 {
                    slot: crate::LocalStorageSlotId::Boundary {
                        operation: OperationId::new(313).unwrap(),
                    },
                };
                row.memory = MachineMemoryEffect::HostedWriteByteV1;
                row.trap = MachineTrapBehavior::HostedWriteFailureV1;
                row.barrier = MachineBarrier::ExternalEffect;
                alternative.key.family = MachineAlternativeFamily::HostedWriteByteI32;
                alternative.encoded.memory = MachineEncodedMemoryEffect::HostedWriteByteV1 {
                    stack_pointer: register_model::RegisterViewId(7),
                };
                alternative.encoded.trap = MachineEncodedTrapBehavior::HostedWriteFailureV1;
                alternative.encoded.control =
                    MachineEncodedControlEffect::HostedWriteReturnOrTrapV1;
            }
            1 => {
                row.kind = SelectedInstructionKind::Store {
                    byte_offset: 2,
                    byte_size: 2,
                };
                row.memory = MachineMemoryEffect::WritePointerV1;
                alternative.key.family = MachineAlternativeFamily::Store;
                alternative.encoded.memory =
                    MachineEncodedMemoryEffect::WritePointerV1 { pointer_operand: 0 };
            }
            _ => {
                row.kind = SelectedInstructionKind::AddressOffset { byte_offset: 2 };
                alternative.key.family = MachineAlternativeFamily::AddressOffset;
            }
        }
        rows.push(row);
    }
    source.identity = pre_allocation_machine_effect_identity(&source);
    assert_eq!(
        PreAllocationMachineEffectPlan::decode(&source.encode()),
        Ok(source.clone())
    );
    let mut substituted = source;
    substituted.functions[0].blocks[0].instructions[1].memory =
        MachineMemoryEffect::HostedWriteByteV1;
    assert!(PreAllocationMachineEffectPlan::decode(&substituted.encode()).is_err());
}

#[test]
fn linux_byte_output_codec_retains_external_effect_trap_and_boundary_scratch() {
    let mut source = plan();
    let row = &mut source.functions[0].blocks[0].instructions[0];
    let slot = crate::LocalStorageSlotId::Boundary {
        operation: OperationId::new(313).unwrap(),
    };
    row.kind = SelectedInstructionKind::HostedWriteByteI32 { slot };
    row.memory = MachineMemoryEffect::HostedWriteByteV1;
    row.trap = MachineTrapBehavior::HostedWriteFailureV1;
    row.barrier = MachineBarrier::ExternalEffect;
    row.alternatives.truncate(1);
    let alternative = &mut row.alternatives[0];
    alternative.key.family = MachineAlternativeFamily::HostedWriteByteI32;
    alternative.encoded.memory = MachineEncodedMemoryEffect::HostedWriteByteV1 {
        stack_pointer: register_model::RegisterViewId(7),
    };
    alternative.encoded.trap = MachineEncodedTrapBehavior::HostedWriteFailureV1;
    alternative.encoded.control = MachineEncodedControlEffect::HostedWriteReturnOrTrapV1;
    source.identity = pre_allocation_machine_effect_identity(&source);
    assert_eq!(
        PreAllocationMachineEffectPlan::decode(&source.encode()).unwrap(),
        source
    );
    for mutation in 0..4 {
        let mut changed = source.clone();
        let row = &mut changed.functions[0].blocks[0].instructions[0];
        match mutation {
            0 => {
                row.kind = SelectedInstructionKind::HostedWriteByteI32 {
                    slot: crate::LocalStorageSlotId::Boundary {
                        operation: OperationId::new(317).unwrap(),
                    },
                }
            }
            1 => row.memory = MachineMemoryEffect::NoneV1,
            2 => row.trap = MachineTrapBehavior::NeverV1,
            _ => row.barrier = MachineBarrier::None,
        }
        assert_ne!(
            pre_allocation_machine_effect_identity(&changed),
            source.identity
        );
        assert!(PreAllocationMachineEffectPlan::decode(&changed.encode()).is_err());
    }
}

#[test]
fn linux_byte_input_codec_binds_structural_home_and_distinct_effects() {
    let mut source = plan();
    let row = &mut source.functions[0].blocks[0].instructions[0];
    let slot = crate::LocalStorageSlotId::Structural {
        place: semantic_vocabulary::PlaceId::new(311).unwrap(),
        operation: OperationId::new(313).unwrap(),
    };
    row.kind = SelectedInstructionKind::HostedReadByte { slot };
    row.memory = MachineMemoryEffect::HostedReadByteV1;
    row.trap = MachineTrapBehavior::HostedReadFailureV1;
    row.barrier = MachineBarrier::ExternalEffect;
    row.alternatives.truncate(1);
    let alternative = &mut row.alternatives[0];
    alternative.key.family = MachineAlternativeFamily::HostedReadByte;
    alternative.encoded.memory = MachineEncodedMemoryEffect::HostedReadByteV1 {
        stack_pointer: register_model::RegisterViewId(7),
    };
    alternative.encoded.trap = MachineEncodedTrapBehavior::HostedReadFailureV1;
    alternative.encoded.control = MachineEncodedControlEffect::HostedReadReturnOrTrapV1;
    source.identity = pre_allocation_machine_effect_identity(&source);
    assert_eq!(
        PreAllocationMachineEffectPlan::decode(&source.encode()).unwrap(),
        source
    );
    for mutation in 0..5 {
        let mut changed = source.clone();
        let row = &mut changed.functions[0].blocks[0].instructions[0];
        match mutation {
            0 => {
                row.kind = SelectedInstructionKind::HostedReadByte {
                    slot: crate::LocalStorageSlotId::Structural {
                        place: semantic_vocabulary::PlaceId::new(311).unwrap(),
                        operation: OperationId::new(317).unwrap(),
                    },
                }
            }
            1 => row.memory = MachineMemoryEffect::NoneV1,
            2 => row.trap = MachineTrapBehavior::NeverV1,
            3 => row.kind = SelectedInstructionKind::HostedWriteByteI32 { slot },
            _ => row.barrier = MachineBarrier::None,
        }
        assert_ne!(
            pre_allocation_machine_effect_identity(&changed),
            source.identity
        );
        assert!(PreAllocationMachineEffectPlan::decode(&changed.encode()).is_err());
    }
}

#[test]
fn byte_view_address_codec_retains_distinct_family_and_source_provenance() {
    let mut source = plan();
    let row = &mut source.functions[0].blocks[0].instructions[0];
    row.kind = SelectedInstructionKind::ByteViewAddress;
    row.alternatives[0].key.family = MachineAlternativeFamily::ByteViewAddress;
    row.provenance.operations = vec![OperationId::new(313).unwrap()];
    source.identity = pre_allocation_machine_effect_identity(&source);
    assert_eq!(
        PreAllocationMachineEffectPlan::decode(&source.encode()).unwrap(),
        source
    );
    let mut changed = source.clone();
    changed.functions[0].blocks[0].instructions[0]
        .provenance
        .operations[0] = OperationId::new(317).unwrap();
    assert_ne!(
        pre_allocation_machine_effect_identity(&changed),
        source.identity
    );
    changed = source.clone();
    changed.functions[0].blocks[0].instructions[0].alternatives[0]
        .key
        .family = MachineAlternativeFamily::ExactAddI64;
    assert_ne!(
        pre_allocation_machine_effect_identity(&changed),
        source.identity
    );
}

#[test]
fn codec_round_trips_complete_effect_content() {
    let source = plan();
    let encoded = source.encode();

    assert_eq!(
        PreAllocationMachineEffectPlan::decode(&encoded).unwrap(),
        source
    );
}

#[test]
fn jump_effects_require_the_current_wire_vocabulary() {
    let mut source = plan();
    let instruction = &mut source.functions[0].blocks[0].instructions[0];
    instruction.kind = SelectedInstructionKind::Jump;
    instruction.alternatives[0].key.family = MachineAlternativeFamily::Jump;
    instruction.alternatives[0].encoded.control =
        MachineEncodedControlEffect::UnconditionalRelativeBranchV1;
    source.identity = pre_allocation_machine_effect_identity(&source);
    let mut bytes = source.encode();
    assert_eq!(&bytes[8..12], &20_u32.to_le_bytes());
    assert_eq!(
        PreAllocationMachineEffectPlan::decode(&bytes).unwrap(),
        source
    );
    bytes[8..12].copy_from_slice(&9_u32.to_le_bytes());
    assert_eq!(
        PreAllocationMachineEffectPlan::decode(&bytes),
        Err(PreAllocationMachineEffectDecodeError::UnsupportedVersion(9))
    );
}

#[test]
fn codec_v8_round_trips_signed_less_than_branch_vocabulary() {
    let mut source = plan();
    let instruction = &mut source.functions[0].blocks[0].instructions[0];
    instruction.kind = SelectedInstructionKind::ConditionalBranchI64LessThan;
    instruction.alternatives[0].key.family = MachineAlternativeFamily::ConditionalBranchI64LessThan;
    source.identity = pre_allocation_machine_effect_identity(&source);

    assert_eq!(
        PreAllocationMachineEffectPlan::decode(&source.encode()).unwrap(),
        source
    );
}

#[test]
fn codec_zero_extension_round_trips_and_rejects_all_prior_versions() {
    let mut source = plan();
    let instruction = &mut source.functions[0].blocks[0].instructions[0];
    instruction.kind = SelectedInstructionKind::ZeroExtendU8;
    instruction.alternatives[0].key.family = MachineAlternativeFamily::ZeroExtendU8;
    source.identity = pre_allocation_machine_effect_identity(&source);
    let encoded = source.encode();
    assert_eq!(
        PreAllocationMachineEffectPlan::decode(&encoded).unwrap(),
        source
    );
    for version in 0_u32..20 {
        let mut stale = encoded.clone();
        stale[8..12].copy_from_slice(&version.to_le_bytes());
        assert_eq!(
            PreAllocationMachineEffectPlan::decode(&stale),
            Err(PreAllocationMachineEffectDecodeError::UnsupportedVersion(
                version
            ))
        );
    }
}

#[test]
fn pointer_store_effect_codec_binds_width_offset_and_write_effect() {
    for byte_size in [1, 2, 4, 8] {
        let mut source = plan();
        let instruction = &mut source.functions[0].blocks[0].instructions[0];
        instruction.kind = SelectedInstructionKind::Store {
            byte_offset: 16,
            byte_size,
        };
        instruction.memory = MachineMemoryEffect::WritePointerV1;
        instruction.alternatives[0].key.family = MachineAlternativeFamily::Store;
        instruction.alternatives[0].encoded.memory =
            MachineEncodedMemoryEffect::WritePointerV1 { pointer_operand: 0 };
        source.identity = pre_allocation_machine_effect_identity(&source);
        assert_eq!(
            PreAllocationMachineEffectPlan::decode(&source.encode()),
            Ok(source.clone())
        );
        let mut changed = source.clone();
        changed.functions[0].blocks[0].instructions[0].kind = SelectedInstructionKind::Store {
            byte_offset: 24,
            byte_size,
        };
        assert!(PreAllocationMachineEffectPlan::decode(&changed.encode()).is_err());
        changed.functions[0].blocks[0].instructions[0].kind = SelectedInstructionKind::Store {
            byte_offset: 16,
            byte_size: 3,
        };
        changed.identity = pre_allocation_machine_effect_identity(&changed);
        assert_eq!(
            PreAllocationMachineEffectPlan::decode(&changed.encode()),
            Err(PreAllocationMachineEffectDecodeError::InvalidField)
        );
    }
    let mut source = plan();
    let instruction = &mut source.functions[0].blocks[0].instructions[0];
    instruction.kind = SelectedInstructionKind::AddressOffset { byte_offset: 2 };
    instruction.alternatives[0].key.family = MachineAlternativeFamily::AddressOffset;
    source.identity = pre_allocation_machine_effect_identity(&source);
    assert_eq!(
        PreAllocationMachineEffectPlan::decode(&source.encode()),
        Ok(source)
    );
}

#[test]
fn codec_u32_zero_extension_round_trips_and_rejects_all_prior_versions() {
    let mut source = plan();
    let instruction = &mut source.functions[0].blocks[0].instructions[0];
    instruction.kind = SelectedInstructionKind::ZeroExtendU32;
    instruction.alternatives[0].key.family = MachineAlternativeFamily::ZeroExtendU32;
    source.identity = pre_allocation_machine_effect_identity(&source);
    let encoded = source.encode();
    assert_eq!(
        PreAllocationMachineEffectPlan::decode(&encoded).unwrap(),
        source
    );
    for version in 0_u32..20 {
        let mut stale = encoded.clone();
        stale[8..12].copy_from_slice(&version.to_le_bytes());
        assert_eq!(
            PreAllocationMachineEffectPlan::decode(&stale),
            Err(PreAllocationMachineEffectDecodeError::UnsupportedVersion(
                version
            ))
        );
    }
}

#[test]
fn codec_rejects_framing_corruption_and_stale_identity() {
    let source = plan();
    let encoded = source.encode();

    let mut wrong_magic = encoded.clone();
    wrong_magic[0] ^= 1;
    assert_eq!(
        PreAllocationMachineEffectPlan::decode(&wrong_magic),
        Err(PreAllocationMachineEffectDecodeError::WrongMagic)
    );

    let mut unsupported_version = encoded.clone();
    unsupported_version[8..12].copy_from_slice(&2_u32.to_le_bytes());
    assert_eq!(
        PreAllocationMachineEffectPlan::decode(&unsupported_version),
        Err(PreAllocationMachineEffectDecodeError::UnsupportedVersion(2))
    );

    let mut stale_identity = encoded.clone();
    stale_identity[12] ^= 1;
    assert_eq!(
        PreAllocationMachineEffectPlan::decode(&stale_identity),
        Err(PreAllocationMachineEffectDecodeError::InvalidIdentity)
    );

    let mut invalid_target = encoded.clone();
    invalid_target[112] = u8::MAX;
    assert_eq!(
        PreAllocationMachineEffectPlan::decode(&invalid_target),
        Err(PreAllocationMachineEffectDecodeError::InvalidField)
    );

    assert_eq!(
        PreAllocationMachineEffectPlan::decode(&encoded[..encoded.len() - 1]),
        Err(PreAllocationMachineEffectDecodeError::Truncated)
    );

    let mut trailing = encoded;
    trailing.push(0);
    assert_eq!(
        PreAllocationMachineEffectPlan::decode(&trailing),
        Err(PreAllocationMachineEffectDecodeError::TrailingBytes)
    );
}

#[test]
fn structural_call_content_is_authenticated_and_closed() {
    let source = plan();
    let mut substituted = source.clone();
    substituted.functions[1].blocks[0].instructions[0].kind = SelectedInstructionKind::CallUnit {
        callee: MachineId::new(99).unwrap(),
    };
    assert_ne!(
        pre_allocation_machine_effect_identity(&substituted),
        source.identity
    );
    assert_eq!(
        PreAllocationMachineEffectPlan::decode(&substituted.encode()),
        Err(PreAllocationMachineEffectDecodeError::InvalidIdentity)
    );

    let mut invalid_declaration_tag = source.encode();
    // The final instruction ends with four empty provenance rosters, empty
    // fuel, and empty alternatives (six u64 lengths). Cleanup precedes them.
    let declaration_tag = invalid_declaration_tag.len() - 49;
    invalid_declaration_tag[declaration_tag] = u8::MAX;
    assert!(matches!(
        PreAllocationMachineEffectPlan::decode(&invalid_declaration_tag),
        Err(PreAllocationMachineEffectDecodeError::InvalidField)
            | Err(PreAllocationMachineEffectDecodeError::InvalidIdentity)
    ));
}

#[test]
fn hosted_exit_codec_retains_nonreturning_effect_without_scratch() {
    let mut source = plan();
    let row = &mut source.functions[0].blocks[0].instructions[0];
    row.kind = SelectedInstructionKind::HostedExitProcessI32;
    row.memory = MachineMemoryEffect::NoneV1;
    row.trap = MachineTrapBehavior::HostedExitReturnedV1;
    row.barrier = MachineBarrier::ExternalEffect;
    row.alternatives.truncate(1);
    let alternative = &mut row.alternatives[0];
    alternative.key.family = MachineAlternativeFamily::HostedExitProcessI32;
    alternative.encoded.memory = MachineEncodedMemoryEffect::NoneV1;
    alternative.encoded.trap = MachineEncodedTrapBehavior::HostedExitReturnedV1;
    alternative.encoded.control = MachineEncodedControlEffect::HostedExitOrTrapV1;
    source.identity = pre_allocation_machine_effect_identity(&source);
    assert_eq!(
        PreAllocationMachineEffectPlan::decode(&source.encode()).unwrap(),
        source
    );
    for mutation in 0..4 {
        let mut changed = source.clone();
        let row = &mut changed.functions[0].blocks[0].instructions[0];
        match mutation {
            0 => row.kind = SelectedInstructionKind::ReturnUnit,
            1 => row.trap = MachineTrapBehavior::NeverV1,
            2 => row.barrier = MachineBarrier::None,
            _ => row.alternatives[0].encoded.control = MachineEncodedControlEffect::FallThroughV1,
        }
        assert_ne!(
            pre_allocation_machine_effect_identity(&changed),
            source.identity
        );
        assert!(PreAllocationMachineEffectPlan::decode(&changed.encode()).is_err());
    }
}
