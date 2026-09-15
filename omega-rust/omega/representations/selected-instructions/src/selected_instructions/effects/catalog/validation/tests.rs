//! Fail-closed catalog admission for the closed hosted byte-output leaf.
use super::{
    MachineAlternativeApplicability, MachineBarrier, MachineEffectDeclaration,
    MachineEncodedControlEffect, MachineEncodedEffects, MachineEncodedMemoryEffect,
    MachineEncodedStackEffect, MachineEncodedTrapBehavior, MachineSemanticKind,
    MachineSizeKnowledge, RegisterInstructionConstraint, RegisterOperandAccess,
    validate_declaration,
};
use crate::{MachineAlternative, MachineAlternativeKey, MachineLatencyKnowledge};
use register_model::{
    RegisterClassId, RegisterConstraintFamily, RegisterConstraintId, RegisterConstraintKey,
    RegisterOperandConstraint, RegisterViewId,
};

#[test]
fn byte_copy_catalog_requires_dynamic_operands_and_early_clobbers() {
    let constraint = RegisterInstructionConstraint {
        id: RegisterConstraintId(0),
        key: RegisterConstraintKey {
            family: RegisterConstraintFamily::Instruction,
            variant: 42,
        },
        operands: (0..5)
            .map(|operand| RegisterOperandConstraint {
                operand,
                access: if operand < 3 {
                    RegisterOperandAccess::Use
                } else {
                    RegisterOperandAccess::Def
                },
                class: RegisterClassId(0),
                fixed_view: None,
                tied_to: None,
                early_clobber: operand >= 3,
            })
            .collect(),
        implicit_uses: vec![],
        implicit_defs: vec![],
        clobbers: vec![register_model::RegisterUnitId(0)],
    };
    let mut encoded = MachineEncodedEffects::fallthrough_v1(vec![0, 1, 2], vec![3, 4]);
    encoded.memory = MachineEncodedMemoryEffect::CopyBytesV1 {
        source_pointer_operand: 0,
        destination_pointer_operand: 1,
        count_operand: 2,
    };
    encoded.trap = MachineEncodedTrapBehavior::MayArchitecturalFaultV1;
    encoded.implicit_unit_clobbers = constraint.clobbers.clone();
    let source = MachineEffectDeclaration {
        semantic: MachineSemanticKind::CopyBytes,
        constraint: constraint.key,
        memory: crate::MachineMemoryEffect::CopyBytesV1,
        trap: crate::MachineTrapBehavior::MayArchitecturalFaultV1,
        barrier: MachineBarrier::None,
        call: crate::MachineCallEffect::NoneV1,
        cleanup: crate::MachineCleanupEffect::NoneV1,
        alternatives: vec![MachineAlternative {
            key: MachineAlternativeKey {
                family: MachineSemanticKind::CopyBytes.into(),
                variant: 0,
            },
            applicability: MachineAlternativeApplicability::Always,
            size: MachineSizeKnowledge::ExactBytes(28),
            latency: MachineLatencyKnowledge::StableBaselineUnavailable,
            encoded,
        }],
    };
    validate_declaration(&constraint, &source).unwrap();
    for mutation in 0..8 {
        let mut changed = source.clone();
        let encoded = &mut changed.alternatives[0].encoded;
        match mutation {
            0 => {
                changed.memory = crate::MachineMemoryEffect::NoneV1;
                encoded.memory = MachineEncodedMemoryEffect::NoneV1;
            }
            1 => {
                encoded.memory = MachineEncodedMemoryEffect::CopyBytesV1 {
                    source_pointer_operand: 1,
                    destination_pointer_operand: 0,
                    count_operand: 2,
                }
            }
            2 => {
                encoded.memory = MachineEncodedMemoryEffect::CopyBytesV1 {
                    source_pointer_operand: 0,
                    destination_pointer_operand: 1,
                    count_operand: 3,
                }
            }
            3 => encoded.implicit_unit_clobbers.clear(),
            4 => encoded.external_operand_reads.truncate(2),
            5 => encoded.external_operand_writes.truncate(1),
            6 => encoded.trap = MachineEncodedTrapBehavior::NeverV1,
            _ => encoded.external_operand_writes.push(0),
        }
        assert!(
            validate_declaration(&constraint, &changed).is_err(),
            "mutation {mutation}"
        );
    }
    for operand in 0..5 {
        let mut changed = constraint.clone();
        changed.operands[operand].early_clobber = !changed.operands[operand].early_clobber;
        assert!(validate_declaration(&changed, &source).is_err());
    }
}

#[test]
fn pointer_load_catalog_requires_the_semantic_exact_width() {
    let constraint = RegisterInstructionConstraint {
        id: RegisterConstraintId(0),
        key: RegisterConstraintKey {
            family: RegisterConstraintFamily::Instruction,
            variant: 32,
        },
        operands: [RegisterOperandAccess::Use, RegisterOperandAccess::Def]
            .into_iter()
            .enumerate()
            .map(|(operand, access)| RegisterOperandConstraint {
                operand: operand as u16,
                access,
                class: RegisterClassId(0),
                fixed_view: None,
                tied_to: None,
                early_clobber: false,
            })
            .collect(),
        implicit_uses: Vec::new(),
        implicit_defs: Vec::new(),
        clobbers: Vec::new(),
    };
    for (semantic, byte_count) in [
        (MachineSemanticKind::LoadPacked3, 3),
        (MachineSemanticKind::LoadPacked5, 5),
        (MachineSemanticKind::LoadPacked6, 6),
        (MachineSemanticKind::LoadPacked7, 7),
        (MachineSemanticKind::Load8, 1),
        (MachineSemanticKind::Load16, 2),
        (MachineSemanticKind::Load32, 4),
        (MachineSemanticKind::Load64, 8),
    ] {
        let mut constraint = constraint.clone();
        let packed = matches!(byte_count, 3 | 5 | 6 | 7);
        if packed {
            constraint.operands[1].early_clobber = true;
            let mut scratch = constraint.operands[1];
            scratch.operand = 2;
            constraint.operands.push(scratch);
        }
        let mut encoded = MachineEncodedEffects::fallthrough_v1(
            vec![0],
            if packed { vec![1, 2] } else { vec![1] },
        );
        encoded.memory = MachineEncodedMemoryEffect::ReadPointerV1 {
            pointer_operand: 0,
            byte_count,
        };
        encoded.trap = MachineEncodedTrapBehavior::MayArchitecturalFaultV1;
        let mut declaration = MachineEffectDeclaration {
            semantic,
            constraint: constraint.key,
            memory: crate::MachineMemoryEffect::ReadPointerV1,
            trap: crate::MachineTrapBehavior::MayArchitecturalFaultV1,
            barrier: MachineBarrier::None,
            call: crate::MachineCallEffect::NoneV1,
            cleanup: crate::MachineCleanupEffect::NoneV1,
            alternatives: vec![MachineAlternative {
                key: MachineAlternativeKey {
                    family: semantic.into(),
                    variant: 0,
                },
                applicability: MachineAlternativeApplicability::Always,
                size: MachineSizeKnowledge::ExactBytes(4),
                latency: MachineLatencyKnowledge::StableBaselineUnavailable,
                encoded,
            }],
        };
        validate_declaration(&constraint, &declaration).unwrap();
        if packed {
            for operand in [1, 2] {
                let mut weakened = constraint.clone();
                weakened.operands[operand].early_clobber = false;
                assert!(validate_declaration(&weakened, &declaration).is_err());
            }
            let mut stripped = declaration.clone();
            stripped.alternatives[0]
                .encoded
                .external_operand_writes
                .pop();
            assert!(validate_declaration(&constraint, &stripped).is_err());
        }
        declaration.alternatives[0].encoded.memory = MachineEncodedMemoryEffect::ReadPointerV1 {
            pointer_operand: 0,
            byte_count: if byte_count == 4 { 8 } else { 4 },
        };
        assert!(validate_declaration(&constraint, &declaration).is_err());
    }
}

#[test]
fn linux_write_catalog_rejects_stripped_memory_trap_and_operand_effects() {
    let constraint = RegisterInstructionConstraint {
        id: RegisterConstraintId(0),
        key: RegisterConstraintKey {
            family: RegisterConstraintFamily::Instruction,
            variant: 31,
        },
        operands: vec![RegisterOperandConstraint {
            operand: 0,
            access: RegisterOperandAccess::Use,
            class: RegisterClassId(0),
            fixed_view: None,
            tied_to: None,
            early_clobber: false,
        }],
        implicit_uses: Vec::new(),
        implicit_defs: Vec::new(),
        clobbers: Vec::new(),
    };
    let mut encoded = MachineEncodedEffects::fallthrough_v1(vec![0], Vec::new());
    encoded.memory = MachineEncodedMemoryEffect::HostedWriteByteV1 {
        stack_pointer: RegisterViewId(7),
    };
    encoded.control = MachineEncodedControlEffect::HostedWriteReturnOrTrapV1;
    encoded.trap = MachineEncodedTrapBehavior::HostedWriteFailureV1;
    let source = MachineEffectDeclaration {
        semantic: MachineSemanticKind::HostedWriteByteI32,
        constraint: constraint.key,
        memory: crate::MachineMemoryEffect::HostedWriteByteV1,
        trap: crate::MachineTrapBehavior::HostedWriteFailureV1,
        barrier: MachineBarrier::ExternalEffect,
        call: crate::MachineCallEffect::NoneV1,
        cleanup: crate::MachineCleanupEffect::NoneV1,
        alternatives: vec![MachineAlternative {
            key: MachineAlternativeKey {
                family: MachineSemanticKind::HostedWriteByteI32.into(),
                variant: 0,
            },
            applicability: MachineAlternativeApplicability::Always,
            size: MachineSizeKnowledge::ExactBytes(40),
            latency: MachineLatencyKnowledge::StableBaselineUnavailable,
            encoded,
        }],
    };
    validate_declaration(&constraint, &source).unwrap();
    for mutation in 0..5 {
        let mut changed = source.clone();
        match mutation {
            0 => {
                changed.memory = crate::MachineMemoryEffect::NoneV1;
                changed.alternatives[0].encoded.memory = MachineEncodedMemoryEffect::NoneV1;
            }
            1 => {
                changed.trap = crate::MachineTrapBehavior::NeverV1;
                changed.alternatives[0].encoded.trap = MachineEncodedTrapBehavior::NeverV1;
            }
            2 => changed.alternatives[0]
                .encoded
                .external_operand_reads
                .clear(),
            3 => changed.alternatives[0]
                .encoded
                .external_operand_writes
                .push(0),
            _ => {
                changed.alternatives[0].encoded.control = MachineEncodedControlEffect::FallThroughV1
            }
        }
        assert!(validate_declaration(&constraint, &changed).is_err());
    }
}

#[test]
fn control_flow_rows_pin_their_encoded_control_shape() {
    let constraint = RegisterInstructionConstraint {
        id: RegisterConstraintId(0),
        key: RegisterConstraintKey {
            family: RegisterConstraintFamily::Instruction,
            variant: 7,
        },
        operands: Vec::new(),
        implicit_uses: Vec::new(),
        implicit_defs: Vec::new(),
        clobbers: Vec::new(),
    };
    let declaration = |semantic, control| {
        let mut encoded = MachineEncodedEffects::fallthrough_v1(Vec::new(), Vec::new());
        encoded.control = control;
        encoded.trap = MachineEncodedTrapBehavior::MayArchitecturalFaultV1;
        MachineEffectDeclaration {
            semantic,
            constraint: constraint.key,
            memory: crate::MachineMemoryEffect::NoneV1,
            trap: crate::MachineTrapBehavior::NeverV1,
            barrier: MachineBarrier::ControlFlow,
            call: crate::MachineCallEffect::NoneV1,
            cleanup: crate::MachineCleanupEffect::NoneV1,
            alternatives: vec![MachineAlternative {
                key: MachineAlternativeKey {
                    family: semantic.into(),
                    variant: 0,
                },
                applicability: MachineAlternativeApplicability::Always,
                size: MachineSizeKnowledge::ExactBytes(5),
                latency: MachineLatencyKnowledge::StableBaselineUnavailable,
                encoded,
            }],
        }
    };
    validate_declaration(
        &constraint,
        &declaration(
            MachineSemanticKind::Jump,
            MachineEncodedControlEffect::UnconditionalRelativeBranchV1,
        ),
    )
    .unwrap();
    validate_declaration(
        &constraint,
        &declaration(
            MachineSemanticKind::ConditionalBranchNonZero,
            MachineEncodedControlEffect::ConditionalRelativeBranchV1,
        ),
    )
    .unwrap();
    for (semantic, control) in [
        (
            MachineSemanticKind::Jump,
            MachineEncodedControlEffect::ConditionalRelativeBranchV1,
        ),
        (
            MachineSemanticKind::Jump,
            MachineEncodedControlEffect::ReturnFromActivationStackV1,
        ),
        (
            MachineSemanticKind::ConditionalBranchNonZero,
            MachineEncodedControlEffect::UnconditionalRelativeBranchV1,
        ),
        (
            MachineSemanticKind::ConditionalBranchU64LessThan,
            MachineEncodedControlEffect::ReturnIndirectRegisterV1 {
                target: RegisterViewId(9),
            },
        ),
    ] {
        assert!(
            validate_declaration(&constraint, &declaration(semantic, control)).is_err(),
            "{semantic:?} borrowed control shape {control:?}"
        );
    }
    let mut hosted_trap = declaration(
        MachineSemanticKind::Jump,
        MachineEncodedControlEffect::UnconditionalRelativeBranchV1,
    );
    hosted_trap.alternatives[0].encoded.trap = MachineEncodedTrapBehavior::HostedExitReturnedV1;
    assert!(validate_declaration(&constraint, &hosted_trap).is_err());
}

#[test]
fn returns_and_calls_reject_each_others_stack_effects() {
    let stack_pointer = RegisterViewId(5);
    let constraint = |variant, access: RegisterOperandAccess| RegisterInstructionConstraint {
        id: RegisterConstraintId(0),
        key: RegisterConstraintKey {
            family: RegisterConstraintFamily::Instruction,
            variant,
        },
        operands: vec![RegisterOperandConstraint {
            operand: 0,
            access,
            class: RegisterClassId(0),
            fixed_view: None,
            tied_to: None,
            early_clobber: false,
        }],
        implicit_uses: Vec::new(),
        implicit_defs: Vec::new(),
        clobbers: Vec::new(),
    };
    let declaration = |semantic, key, barrier, reads, writes, memory, stack, control| {
        let mut encoded = MachineEncodedEffects::fallthrough_v1(reads, writes);
        encoded.memory = memory;
        encoded.stack = stack;
        encoded.control = control;
        encoded.trap = MachineEncodedTrapBehavior::MayArchitecturalFaultV1;
        MachineEffectDeclaration {
            semantic,
            constraint: key,
            memory: crate::MachineMemoryEffect::NoneV1,
            trap: crate::MachineTrapBehavior::NeverV1,
            barrier,
            call: if barrier == MachineBarrier::Call {
                crate::MachineCallEffect::DirectInternalNormalReturnV1 {
                    pre_call_stack_alignment: 16,
                }
            } else {
                crate::MachineCallEffect::NoneV1
            },
            cleanup: crate::MachineCleanupEffect::NoneV1,
            alternatives: vec![MachineAlternative {
                key: MachineAlternativeKey {
                    family: semantic.into(),
                    variant: 0,
                },
                applicability: MachineAlternativeApplicability::Always,
                size: MachineSizeKnowledge::ExactBytes(5),
                latency: MachineLatencyKnowledge::StableBaselineUnavailable,
                encoded,
            }],
        }
    };
    let activation_stack = (
        MachineEncodedMemoryEffect::ReadActivationStackV1 {
            stack_pointer,
            byte_count: 8,
        },
        MachineEncodedStackEffect::PopBytesV1 {
            stack_pointer,
            byte_count: 8,
        },
    );
    let call_return_address = (
        MachineEncodedMemoryEffect::WriteReturnAddressBelowStackPointerV1 {
            stack_pointer,
            byte_count: 8,
        },
        MachineEncodedStackEffect::CallReturnAddressLifecycleV1 {
            stack_pointer,
            return_address_byte_count: 8,
        },
    );
    let return_constraint = constraint(11, RegisterOperandAccess::Use);
    validate_declaration(
        &return_constraint,
        &declaration(
            MachineSemanticKind::ReturnScalar,
            return_constraint.key,
            MachineBarrier::ControlFlow,
            vec![0],
            Vec::new(),
            activation_stack.0,
            activation_stack.1,
            MachineEncodedControlEffect::ReturnFromActivationStackV1,
        ),
    )
    .unwrap();
    // A return cannot borrow the call return-address lifecycle.
    assert!(
        validate_declaration(
            &return_constraint,
            &declaration(
                MachineSemanticKind::ReturnScalar,
                return_constraint.key,
                MachineBarrier::ControlFlow,
                vec![0],
                Vec::new(),
                call_return_address.0,
                call_return_address.1,
                MachineEncodedControlEffect::ReturnFromActivationStackV1,
            ),
        )
        .is_err()
    );
    let call_constraint = constraint(13, RegisterOperandAccess::Def);
    validate_declaration(
        &call_constraint,
        &declaration(
            MachineSemanticKind::CallScalar,
            call_constraint.key,
            MachineBarrier::Call,
            Vec::new(),
            vec![0],
            call_return_address.0,
            call_return_address.1,
            MachineEncodedControlEffect::DirectRelativeCallV1,
        ),
    )
    .unwrap();
    // A call cannot borrow the return activation-stack pop either.
    assert!(
        validate_declaration(
            &call_constraint,
            &declaration(
                MachineSemanticKind::CallScalar,
                call_constraint.key,
                MachineBarrier::Call,
                Vec::new(),
                vec![0],
                activation_stack.0,
                activation_stack.1,
                MachineEncodedControlEffect::DirectRelativeCallV1,
            ),
        )
        .is_err()
    );
}

#[test]
fn indexed_and_frame_memory_rows_reject_foreign_semantics() {
    let constraint = RegisterInstructionConstraint {
        id: RegisterConstraintId(0),
        key: RegisterConstraintKey {
            family: RegisterConstraintFamily::Instruction,
            variant: 13,
        },
        operands: [
            RegisterOperandAccess::Use,
            RegisterOperandAccess::Use,
            RegisterOperandAccess::Def,
        ]
        .into_iter()
        .enumerate()
        .map(|(operand, access)| RegisterOperandConstraint {
            operand: operand as u16,
            access,
            class: RegisterClassId(0),
            fixed_view: None,
            tied_to: None,
            early_clobber: false,
        })
        .collect(),
        implicit_uses: Vec::new(),
        implicit_defs: Vec::new(),
        clobbers: Vec::new(),
    };
    let mut indexed = MachineEncodedEffects::fallthrough_v1(vec![0, 1], vec![2]);
    indexed.memory = MachineEncodedMemoryEffect::ReadIndexedPointerV1 {
        pointer_operand: 0,
        index_operand: 1,
        byte_count: 1,
    };
    indexed.trap = MachineEncodedTrapBehavior::MayArchitecturalFaultV1;
    let declaration =
        |semantic, memory, trap, encoded: &MachineEncodedEffects| MachineEffectDeclaration {
            semantic,
            constraint: constraint.key,
            memory,
            trap,
            barrier: MachineBarrier::None,
            call: crate::MachineCallEffect::NoneV1,
            cleanup: crate::MachineCleanupEffect::NoneV1,
            alternatives: vec![MachineAlternative {
                key: MachineAlternativeKey {
                    family: semantic.into(),
                    variant: 0,
                },
                applicability: MachineAlternativeApplicability::Always,
                size: MachineSizeKnowledge::ExactBytes(9),
                latency: MachineLatencyKnowledge::StableBaselineUnavailable,
                encoded: encoded.clone(),
            }],
        };
    validate_declaration(
        &constraint,
        &declaration(
            MachineSemanticKind::Load8Indexed,
            crate::MachineMemoryEffect::ReadPointerV1,
            crate::MachineTrapBehavior::MayArchitecturalFaultV1,
            &indexed,
        ),
    )
    .unwrap();
    // The indexed footprint belongs to Load8Indexed alone; other rules that
    // also declare a pointer read may not borrow it.
    for semantic in [
        MachineSemanticKind::Load64,
        MachineSemanticKind::ExactAddI64,
    ] {
        assert!(
            validate_declaration(
                &constraint,
                &declaration(
                    semantic,
                    crate::MachineMemoryEffect::ReadPointerV1,
                    crate::MachineTrapBehavior::MayArchitecturalFaultV1,
                    &indexed,
                ),
            )
            .is_err(),
            "{semantic:?} borrowed the indexed-load footprint"
        );
    }
    // An ordinary rule cannot understate a declared memory access as
    // never-faulting.
    let mut pointer_read = MachineEncodedEffects::fallthrough_v1(vec![0], vec![2]);
    pointer_read.memory = MachineEncodedMemoryEffect::ReadPointerV1 {
        pointer_operand: 0,
        byte_count: 8,
    };
    pointer_read.trap = MachineEncodedTrapBehavior::MayArchitecturalFaultV1;
    assert!(
        validate_declaration(
            &constraint,
            &declaration(
                MachineSemanticKind::Load64,
                crate::MachineMemoryEffect::ReadPointerV1,
                crate::MachineTrapBehavior::NeverV1,
                &pointer_read,
            ),
        )
        .is_err()
    );
    // A hosted trap result cannot appear on an ordinary rule either.
    assert!(
        validate_declaration(
            &constraint,
            &declaration(
                MachineSemanticKind::ExactAddI64,
                crate::MachineMemoryEffect::NoneV1,
                crate::MachineTrapBehavior::HostedReadFailureV1,
                &MachineEncodedEffects::fallthrough_v1(vec![0, 1], vec![2]),
            ),
        )
        .is_err()
    );
    let mut frame_store = MachineEncodedEffects::fallthrough_v1(vec![0], Vec::new());
    frame_store.memory = MachineEncodedMemoryEffect::WriteFrameStorageV1 {
        stack_pointer: RegisterViewId(5),
        byte_count: 8,
    };
    frame_store.trap = MachineEncodedTrapBehavior::MayArchitecturalFaultV1;
    for (semantic, valid) in [
        (MachineSemanticKind::Store64, true),
        (MachineSemanticKind::AddressOffset, false),
    ] {
        assert_eq!(
            validate_declaration(
                &constraint,
                &declaration(
                    semantic,
                    crate::MachineMemoryEffect::WriteFrameStorageV1,
                    crate::MachineTrapBehavior::MayArchitecturalFaultV1,
                    &frame_store,
                ),
            )
            .is_ok(),
            valid,
            "{semantic:?} frame-storage footprint"
        );
    }
}

#[test]
fn hosted_exit_cannot_fall_through_to_the_plain_surface_row() {
    let constraint = RegisterInstructionConstraint {
        id: RegisterConstraintId(0),
        key: RegisterConstraintKey {
            family: RegisterConstraintFamily::Instruction,
            variant: 17,
        },
        operands: [RegisterOperandAccess::Use, RegisterOperandAccess::Use]
            .into_iter()
            .enumerate()
            .map(|(operand, access)| RegisterOperandConstraint {
                operand: operand as u16,
                access,
                class: RegisterClassId(0),
                fixed_view: None,
                tied_to: None,
                early_clobber: false,
            })
            .collect(),
        implicit_uses: Vec::new(),
        implicit_defs: Vec::new(),
        clobbers: Vec::new(),
    };
    let mut encoded = MachineEncodedEffects::fallthrough_v1(vec![0, 1], Vec::new());
    encoded.control = MachineEncodedControlEffect::HostedExitOrTrapV1;
    encoded.trap = MachineEncodedTrapBehavior::HostedExitReturnedV1;
    let declaration = MachineEffectDeclaration {
        semantic: MachineSemanticKind::HostedExitProcessI32,
        constraint: constraint.key,
        memory: crate::MachineMemoryEffect::NoneV1,
        trap: crate::MachineTrapBehavior::HostedExitReturnedV1,
        barrier: MachineBarrier::ExternalEffect,
        call: crate::MachineCallEffect::NoneV1,
        cleanup: crate::MachineCleanupEffect::NoneV1,
        alternatives: vec![MachineAlternative {
            key: MachineAlternativeKey {
                family: MachineSemanticKind::HostedExitProcessI32.into(),
                variant: 0,
            },
            applicability: MachineAlternativeApplicability::Always,
            size: MachineSizeKnowledge::ExactBytes(5),
            latency: MachineLatencyKnowledge::StableBaselineUnavailable,
            encoded,
        }],
    };
    // The two-operand roster must be rejected by the hosted-exit row itself,
    // not waved through by the plain fallthrough row.
    assert!(validate_declaration(&constraint, &declaration).is_err());
}
