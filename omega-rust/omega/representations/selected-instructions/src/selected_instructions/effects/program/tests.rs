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
fn codec_round_trips_complete_effect_content() {
    let source = plan();
    let encoded = source.encode();

    assert_eq!(
        PreAllocationMachineEffectPlan::decode(&encoded).unwrap(),
        source
    );
}

#[test]
fn jump_effects_require_the_v12_wire_vocabulary() {
    let mut source = plan();
    let instruction = &mut source.functions[0].blocks[0].instructions[0];
    instruction.kind = SelectedInstructionKind::Jump;
    instruction.alternatives[0].key.family = MachineAlternativeFamily::Jump;
    instruction.alternatives[0].encoded.control =
        MachineEncodedControlEffect::UnconditionalRelativeBranchV1;
    source.identity = pre_allocation_machine_effect_identity(&source);
    let mut bytes = source.encode();
    assert_eq!(&bytes[8..12], &12_u32.to_le_bytes());
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
    for version in 0_u32..12 {
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
