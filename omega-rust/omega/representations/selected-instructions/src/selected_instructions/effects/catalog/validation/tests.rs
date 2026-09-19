//! Fail-closed catalog admission for the closed hosted byte-output leaf.
use super::{
    MachineAlternativeApplicability, MachineBarrier, MachineEffectCatalogValidationError,
    MachineEffectDeclaration, MachineEncodedControlEffect, MachineEncodedEffects,
    MachineEncodedMemoryEffect, MachineEncodedStackEffect, MachineEncodedTrapBehavior,
    MachineSemanticKind, MachineSizeKnowledge, RegisterInstructionConstraint,
    RegisterOperandAccess, validate_declaration,
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
            Vec::new(),
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
                Vec::new(),
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
    // Store64 owns the frame-storage write on its single value operand.
    let store_constraint = RegisterInstructionConstraint {
        id: RegisterConstraintId(0),
        key: RegisterConstraintKey {
            family: RegisterConstraintFamily::Instruction,
            variant: 29,
        },
        operands: vec![RegisterOperandConstraint {
            operand: 0,
            access: RegisterOperandAccess::Use,
            class: RegisterClassId(0),
            fixed_view: None,
            tied_to: None,
            early_clobber: false,
        }],
        implicit_uses: vec![register_model::RegisterUnitId(9)],
        implicit_defs: Vec::new(),
        clobbers: Vec::new(),
    };
    let mut frame_store = MachineEncodedEffects::fallthrough_v1(vec![0], Vec::new());
    frame_store.memory = MachineEncodedMemoryEffect::WriteFrameStorageV1 {
        stack_pointer: RegisterViewId(5),
        byte_count: 8,
    };
    frame_store.trap = MachineEncodedTrapBehavior::MayArchitecturalFaultV1;
    frame_store.implicit_unit_uses = store_constraint.implicit_uses.clone();
    validate_declaration(
        &store_constraint,
        &declaration(
            MachineSemanticKind::Store64,
            crate::MachineMemoryEffect::WriteFrameStorageV1,
            crate::MachineTrapBehavior::MayArchitecturalFaultV1,
            &frame_store,
        ),
    )
    .unwrap();
    // AddressOffset keeps an internally consistent operand contract of its
    // own; it still cannot borrow the frame-storage footprint.
    let offset_constraint = RegisterInstructionConstraint {
        id: RegisterConstraintId(0),
        key: RegisterConstraintKey {
            family: RegisterConstraintFamily::Instruction,
            variant: 31,
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
    let mut borrowed = MachineEncodedEffects::fallthrough_v1(vec![0], vec![1]);
    borrowed.memory = frame_store.memory;
    borrowed.trap = frame_store.trap;
    assert!(
        validate_declaration(
            &offset_constraint,
            &declaration(
                MachineSemanticKind::AddressOffset,
                crate::MachineMemoryEffect::WriteFrameStorageV1,
                crate::MachineTrapBehavior::MayArchitecturalFaultV1,
                &borrowed,
            ),
        )
        .is_err()
    );
}

#[test]
fn encoded_rows_restate_the_contracted_operand_custody() {
    let constraint = |variant, accesses: &[RegisterOperandAccess]| RegisterInstructionConstraint {
        id: RegisterConstraintId(0),
        key: RegisterConstraintKey {
            family: RegisterConstraintFamily::Instruction,
            variant,
        },
        operands: accesses
            .iter()
            .enumerate()
            .map(|(operand, access)| RegisterOperandConstraint {
                operand: operand as u16,
                access: *access,
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
    let declaration = |semantic, key, applicability, reads: Vec<u16>, writes: Vec<u16>| {
        MachineEffectDeclaration {
            semantic,
            constraint: key,
            memory: crate::MachineMemoryEffect::NoneV1,
            trap: crate::MachineTrapBehavior::NeverV1,
            barrier: MachineBarrier::None,
            call: crate::MachineCallEffect::NoneV1,
            cleanup: crate::MachineCleanupEffect::NoneV1,
            alternatives: vec![MachineAlternative {
                key: MachineAlternativeKey {
                    family: semantic.into(),
                    variant: 0,
                },
                applicability,
                size: MachineSizeKnowledge::ExactBytes(4),
                latency: MachineLatencyKnowledge::StableBaselineUnavailable,
                encoded: MachineEncodedEffects::fallthrough_v1(reads, writes),
            }],
        }
    };
    // An ordinary three-operand rule reads every Use and writes every Def.
    let arithmetic = constraint(
        41,
        &[
            RegisterOperandAccess::Use,
            RegisterOperandAccess::Use,
            RegisterOperandAccess::Def,
        ],
    );
    validate_declaration(
        &arithmetic,
        &declaration(
            MachineSemanticKind::ExactAddI64,
            arithmetic.key,
            MachineAlternativeApplicability::Always,
            vec![0, 1],
            vec![2],
        ),
    )
    .unwrap();
    for (reads, writes) in [
        (vec![0], vec![2]),       // an input the semantic consumes is missing
        (vec![1], vec![2]),       // the other input is missing
        (vec![0, 1], vec![]),     // the contracted definition is missing
        (vec![0, 1], vec![0]),    // a substituted write names an input
        (vec![0, 1, 2], vec![2]), // a definition cannot pose as an input
    ] {
        assert!(
            validate_declaration(
                &arithmetic,
                &declaration(
                    MachineSemanticKind::ExactAddI64,
                    arithmetic.key,
                    MachineAlternativeApplicability::Always,
                    reads.clone(),
                    writes.clone(),
                ),
            )
            .is_err(),
            "reads {reads:?} writes {writes:?}"
        );
    }
    // A read-write operand is contracted in both directions; neither may be
    // dropped from the encoded surface.
    let read_write = constraint(
        43,
        &[RegisterOperandAccess::UseDef, RegisterOperandAccess::Use],
    );
    validate_declaration(
        &read_write,
        &declaration(
            MachineSemanticKind::CopyI64,
            read_write.key,
            MachineAlternativeApplicability::Always,
            vec![0, 1],
            vec![0],
        ),
    )
    .unwrap();
    for (reads, writes) in [(vec![1], vec![0]), (vec![0, 1], vec![])] {
        assert!(
            validate_declaration(
                &read_write,
                &declaration(
                    MachineSemanticKind::CopyI64,
                    read_write.key,
                    MachineAlternativeApplicability::Always,
                    reads.clone(),
                    writes.clone(),
                ),
            )
            .is_err(),
            "reads {reads:?} writes {writes:?}"
        );
    }
    // A return rule places its operand homes for the caller: its encoding
    // reads no operand, and it may not claim that it does.
    let returned = constraint(45, &[RegisterOperandAccess::Use]);
    let mut return_row = declaration(
        MachineSemanticKind::ReturnScalar,
        returned.key,
        MachineAlternativeApplicability::Always,
        Vec::new(),
        Vec::new(),
    );
    return_row.barrier = MachineBarrier::ControlFlow;
    return_row.alternatives[0].encoded.control =
        MachineEncodedControlEffect::ReturnIndirectRegisterV1 {
            target: RegisterViewId(9),
        };
    return_row.alternatives[0].encoded.trap = MachineEncodedTrapBehavior::MayArchitecturalFaultV1;
    validate_declaration(&returned, &return_row).unwrap();
    return_row.alternatives[0].encoded.external_operand_reads = vec![0];
    assert!(validate_declaration(&returned, &return_row).is_err());
    // Only the all-aliased subtract realization drops its inputs: the result
    // of x - x does not depend on either operand's incoming value.
    let subtract = constraint(
        47,
        &[
            RegisterOperandAccess::Use,
            RegisterOperandAccess::Use,
            RegisterOperandAccess::Def,
        ],
    );
    let all_aliased = MachineAlternativeApplicability::ResultAliasesOperands {
        result: 2,
        left: 0,
        right: 1,
    };
    validate_declaration(
        &subtract,
        &declaration(
            MachineSemanticKind::ExactSubtractI64,
            subtract.key,
            all_aliased,
            Vec::new(),
            vec![2],
        ),
    )
    .unwrap();
    for (applicability, reads) in [
        // The zeroing form cannot pretend it still consumes an input.
        (all_aliased, vec![0]),
        // A partially aliased form reads both inputs it operates on.
        (
            MachineAlternativeApplicability::ResultAliasesOperandAndDistinctFromOperand {
                result: 2,
                aliased_operand: 0,
                distinct_operand: 1,
            },
            vec![0],
        ),
        (
            MachineAlternativeApplicability::ResultDistinctFromOperands {
                result: 2,
                left: 0,
                right: 1,
            },
            vec![0],
        ),
    ] {
        assert!(
            validate_declaration(
                &subtract,
                &declaration(
                    MachineSemanticKind::ExactSubtractI64,
                    subtract.key,
                    applicability,
                    reads.clone(),
                    vec![2],
                ),
            )
            .is_err(),
            "subtract applicability {applicability:?} reads {reads:?}"
        );
    }
    // The exemption belongs to subtraction: an all-aliased add row still
    // depends on its shared input's incoming value.
    assert!(
        validate_declaration(
            &subtract,
            &declaration(
                MachineSemanticKind::ExactAddI64,
                subtract.key,
                all_aliased,
                Vec::new(),
                vec![2],
            ),
        )
        .is_err()
    );
}

#[test]
fn encoded_rows_restate_the_contracted_implicit_custody() {
    let unit = |id| register_model::RegisterUnitId(id);
    let constraint = |variant,
                      accesses: &[RegisterOperandAccess],
                      uses: &[u16],
                      defs: &[u16],
                      clobbers: &[u16]| {
        RegisterInstructionConstraint {
            id: RegisterConstraintId(0),
            key: RegisterConstraintKey {
                family: RegisterConstraintFamily::Instruction,
                variant,
            },
            operands: accesses
                .iter()
                .enumerate()
                .map(|(operand, access)| RegisterOperandConstraint {
                    operand: operand as u16,
                    access: *access,
                    class: RegisterClassId(0),
                    fixed_view: None,
                    tied_to: None,
                    early_clobber: false,
                })
                .collect(),
            implicit_uses: uses.iter().map(|id| unit(*id)).collect(),
            implicit_defs: defs.iter().map(|id| unit(*id)).collect(),
            clobbers: clobbers.iter().map(|id| unit(*id)).collect(),
        }
    };
    let declaration = |semantic, key, encoded| MachineEffectDeclaration {
        semantic,
        constraint: key,
        memory: crate::MachineMemoryEffect::NoneV1,
        trap: crate::MachineTrapBehavior::NeverV1,
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
    // An ordinary rule restates every implicit unit the row contracts:
    // each read, each definition, each clobber.
    let row = constraint(
        53,
        &[RegisterOperandAccess::Use, RegisterOperandAccess::Def],
        &[7, 9],
        &[11],
        &[13],
    );
    let mut encoded = MachineEncodedEffects::fallthrough_v1(vec![0], vec![1]);
    encoded.trap = MachineEncodedTrapBehavior::MayArchitecturalFaultV1;
    encoded.implicit_unit_uses = row.implicit_uses.clone();
    encoded.implicit_unit_defs = row.implicit_defs.clone();
    encoded.implicit_unit_clobbers = row.clobbers.clone();
    validate_declaration(
        &row,
        &declaration(MachineSemanticKind::ExactAddI64, row.key, encoded.clone()),
    )
    .unwrap();
    // Dropping a contracted unit understates the surface homes are
    // allocated against; inventing one misstates the encoding. Neither
    // passes admission in any of the three lists.
    for mutation in 0..6 {
        let mut changed = encoded.clone();
        let (list, foreign) = match mutation {
            0 | 1 => (&mut changed.implicit_unit_uses, unit(8)),
            2 | 3 => (&mut changed.implicit_unit_defs, unit(12)),
            _ => (&mut changed.implicit_unit_clobbers, unit(14)),
        };
        if mutation % 2 == 0 {
            list.pop();
        } else {
            list.push(foreign);
            list.sort_unstable();
        }
        assert!(
            validate_declaration(
                &row,
                &declaration(MachineSemanticKind::ExactAddI64, row.key, changed),
            )
            .is_err(),
            "mutation {mutation}"
        );
    }
    // An indirect-register return narrows its implicit uses to the
    // non-empty subset it honestly reads — the link register — while the
    // row still carries the ABI's stack-pointer use. Its definitions and
    // clobbers remain exact.
    let returned = constraint(55, &[RegisterOperandAccess::Use], &[5, 9], &[11], &[]);
    let mut return_encoded = MachineEncodedEffects::fallthrough_v1(Vec::new(), Vec::new());
    return_encoded.trap = MachineEncodedTrapBehavior::MayArchitecturalFaultV1;
    return_encoded.control = MachineEncodedControlEffect::ReturnIndirectRegisterV1 {
        target: RegisterViewId(9),
    };
    return_encoded.implicit_unit_uses = vec![unit(9)];
    return_encoded.implicit_unit_defs = returned.implicit_defs.clone();
    let mut return_row = declaration(
        MachineSemanticKind::ReturnScalar,
        returned.key,
        return_encoded,
    );
    return_row.barrier = MachineBarrier::ControlFlow;
    validate_declaration(&returned, &return_row).unwrap();
    // Restating the full use list is still admitted; reading no implicit
    // unit, or a foreign one, is not — and defs and clobbers never narrow.
    return_row.alternatives[0].encoded.implicit_unit_uses = returned.implicit_uses.clone();
    validate_declaration(&returned, &return_row).unwrap();
    for mutation in 0..4 {
        let mut changed = return_row.clone();
        let encoded = &mut changed.alternatives[0].encoded;
        match mutation {
            0 => encoded.implicit_unit_uses.clear(),
            1 => encoded.implicit_unit_uses = vec![unit(4)],
            2 => encoded.implicit_unit_defs.clear(),
            _ => encoded.implicit_unit_clobbers.push(unit(15)),
        }
        assert!(
            validate_declaration(&returned, &changed).is_err(),
            "mutation {mutation}"
        );
    }
    // The narrowed set belongs to the indirect form alone: an
    // activation-stack return owes the row every implicit use it declares.
    let mut stack_return = return_row.clone();
    stack_return.alternatives[0].encoded.control =
        MachineEncodedControlEffect::ReturnFromActivationStackV1;
    validate_declaration(&returned, &stack_return).unwrap();
    stack_return.alternatives[0].encoded.implicit_unit_uses = vec![unit(9)];
    assert!(validate_declaration(&returned, &stack_return).is_err());
}

#[test]
fn declared_memory_and_trap_surfaces_bind_the_selected_semantic() {
    let constraint = |variant, accesses: &[RegisterOperandAccess]| RegisterInstructionConstraint {
        id: RegisterConstraintId(0),
        key: RegisterConstraintKey {
            family: RegisterConstraintFamily::Instruction,
            variant,
        },
        operands: accesses
            .iter()
            .enumerate()
            .map(|(operand, access)| RegisterOperandConstraint {
                operand: operand as u16,
                access: *access,
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
    let declaration =
        |semantic, key, memory, trap, barrier, call, encoded| MachineEffectDeclaration {
            semantic,
            constraint: key,
            memory,
            trap,
            barrier,
            call,
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
    // A never-faulting rule cannot claim the bare architectural fault, and
    // no arithmetic row carries a declared memory footprint.
    let arithmetic = constraint(
        61,
        &[
            RegisterOperandAccess::Use,
            RegisterOperandAccess::Use,
            RegisterOperandAccess::Def,
        ],
    );
    let add = declaration(
        MachineSemanticKind::ExactAddI64,
        arithmetic.key,
        crate::MachineMemoryEffect::NoneV1,
        crate::MachineTrapBehavior::NeverV1,
        MachineBarrier::None,
        crate::MachineCallEffect::NoneV1,
        MachineEncodedEffects::fallthrough_v1(vec![0, 1], vec![2]),
    );
    validate_declaration(&arithmetic, &add).unwrap();
    for (memory, trap) in [
        (
            crate::MachineMemoryEffect::NoneV1,
            crate::MachineTrapBehavior::MayArchitecturalFaultV1,
        ),
        (
            crate::MachineMemoryEffect::NoneV1,
            crate::MachineTrapBehavior::HostedReadFailureV1,
        ),
        (
            crate::MachineMemoryEffect::ReadPointerV1,
            crate::MachineTrapBehavior::NeverV1,
        ),
        (
            crate::MachineMemoryEffect::WriteFrameStorageV1,
            crate::MachineTrapBehavior::MayArchitecturalFaultV1,
        ),
    ] {
        let mut forged = add.clone();
        forged.memory = memory;
        forged.trap = trap;
        assert!(
            validate_declaration(&arithmetic, &forged).is_err(),
            "memory {memory:?} trap {trap:?}"
        );
    }
    // A faulting load cannot understate its declared trap, cannot name a
    // hosted result, and cannot borrow a sibling footprint class.
    let load = constraint(
        63,
        &[RegisterOperandAccess::Use, RegisterOperandAccess::Def],
    );
    let mut load_encoded = MachineEncodedEffects::fallthrough_v1(vec![0], vec![1]);
    load_encoded.memory = MachineEncodedMemoryEffect::ReadPointerV1 {
        pointer_operand: 0,
        byte_count: 8,
    };
    load_encoded.trap = MachineEncodedTrapBehavior::MayArchitecturalFaultV1;
    let load64 = declaration(
        MachineSemanticKind::Load64,
        load.key,
        crate::MachineMemoryEffect::ReadPointerV1,
        crate::MachineTrapBehavior::MayArchitecturalFaultV1,
        MachineBarrier::None,
        crate::MachineCallEffect::NoneV1,
        load_encoded,
    );
    validate_declaration(&load, &load64).unwrap();
    for (memory, trap) in [
        (
            crate::MachineMemoryEffect::ReadPointerV1,
            crate::MachineTrapBehavior::NeverV1,
        ),
        (
            crate::MachineMemoryEffect::ReadPointerV1,
            crate::MachineTrapBehavior::HostedWriteFailureV1,
        ),
        (
            crate::MachineMemoryEffect::WritePointerV1,
            crate::MachineTrapBehavior::MayArchitecturalFaultV1,
        ),
        (
            crate::MachineMemoryEffect::NoneV1,
            crate::MachineTrapBehavior::MayArchitecturalFaultV1,
        ),
    ] {
        let mut forged = load64.clone();
        forged.memory = memory;
        forged.trap = trap;
        assert!(
            validate_declaration(&load, &forged).is_err(),
            "memory {memory:?} trap {trap:?}"
        );
    }
    // A return's activation-stack lifecycle lives in its encoded stack
    // surface: the declaration still claims no memory footprint and no
    // architectural fault, and its join arm cannot be borrowed to launder
    // either one.
    let returned = constraint(65, &[RegisterOperandAccess::Use]);
    let stack_pointer = RegisterViewId(5);
    let mut return_encoded = MachineEncodedEffects::fallthrough_v1(Vec::new(), Vec::new());
    return_encoded.memory = MachineEncodedMemoryEffect::ReadActivationStackV1 {
        stack_pointer,
        byte_count: 8,
    };
    return_encoded.stack = MachineEncodedStackEffect::PopBytesV1 {
        stack_pointer,
        byte_count: 8,
    };
    return_encoded.trap = MachineEncodedTrapBehavior::MayArchitecturalFaultV1;
    return_encoded.control = MachineEncodedControlEffect::ReturnFromActivationStackV1;
    let returned_row = declaration(
        MachineSemanticKind::ReturnScalar,
        returned.key,
        crate::MachineMemoryEffect::NoneV1,
        crate::MachineTrapBehavior::NeverV1,
        MachineBarrier::ControlFlow,
        crate::MachineCallEffect::NoneV1,
        return_encoded,
    );
    validate_declaration(&returned, &returned_row).unwrap();
    for (memory, trap) in [
        (
            crate::MachineMemoryEffect::ReadPointerV1,
            crate::MachineTrapBehavior::NeverV1,
        ),
        (
            crate::MachineMemoryEffect::NoneV1,
            crate::MachineTrapBehavior::MayArchitecturalFaultV1,
        ),
    ] {
        let mut forged = returned_row.clone();
        forged.memory = memory;
        forged.trap = trap;
        assert!(
            validate_declaration(&returned, &forged).is_err(),
            "memory {memory:?} trap {trap:?}"
        );
    }
    // A call's return-address lifecycle lives in its encoded stack surface
    // the same way: the declaration claims neither a footprint nor a fault.
    let called = constraint(
        67,
        &[RegisterOperandAccess::Use, RegisterOperandAccess::Def],
    );
    let mut call_encoded = MachineEncodedEffects::fallthrough_v1(vec![0], vec![1]);
    call_encoded.memory = MachineEncodedMemoryEffect::WriteReturnAddressBelowStackPointerV1 {
        stack_pointer,
        byte_count: 8,
    };
    call_encoded.stack = MachineEncodedStackEffect::CallReturnAddressLifecycleV1 {
        stack_pointer,
        return_address_byte_count: 8,
    };
    call_encoded.trap = MachineEncodedTrapBehavior::MayArchitecturalFaultV1;
    call_encoded.control = MachineEncodedControlEffect::DirectRelativeCallV1;
    let call_row = declaration(
        MachineSemanticKind::CallScalar,
        called.key,
        crate::MachineMemoryEffect::NoneV1,
        crate::MachineTrapBehavior::NeverV1,
        MachineBarrier::Call,
        crate::MachineCallEffect::DirectInternalNormalReturnV1 {
            pre_call_stack_alignment: 16,
        },
        call_encoded,
    );
    validate_declaration(&called, &call_row).unwrap();
    for (memory, trap) in [
        (
            crate::MachineMemoryEffect::WritePointerV1,
            crate::MachineTrapBehavior::NeverV1,
        ),
        (
            crate::MachineMemoryEffect::NoneV1,
            crate::MachineTrapBehavior::MayArchitecturalFaultV1,
        ),
    ] {
        let mut forged = call_row.clone();
        forged.memory = memory;
        forged.trap = trap;
        assert!(
            validate_declaration(&called, &forged).is_err(),
            "memory {memory:?} trap {trap:?}"
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

#[test]
fn normalized_foreign_call_declarations_bind_external_call_effects() {
    let unit = |id| register_model::RegisterUnitId(id);
    // One per-plan foreign row: two Use arguments and one Def result, every
    // operand pinned to the exact ABI view the evaluated plan selected.
    let foreign_row = |variant: u32, arity: u16, result: bool| RegisterInstructionConstraint {
        id: RegisterConstraintId(0),
        key: RegisterConstraintKey {
            family: RegisterConstraintFamily::Call,
            variant,
        },
        operands: (0..arity)
            .map(|operand| RegisterOperandConstraint {
                operand,
                access: RegisterOperandAccess::Use,
                class: RegisterClassId(0),
                fixed_view: Some(RegisterViewId(operand)),
                tied_to: None,
                early_clobber: false,
            })
            .chain(result.then_some(RegisterOperandConstraint {
                operand: arity,
                access: RegisterOperandAccess::Def,
                class: RegisterClassId(0),
                fixed_view: Some(RegisterViewId(arity)),
                tied_to: None,
                early_clobber: false,
            }))
            .collect(),
        implicit_uses: vec![unit(4)],
        implicit_defs: Vec::new(),
        clobbers: vec![unit(0), unit(1)],
    };
    let declaration = |key, encoded: MachineEncodedEffects| MachineEffectDeclaration {
        semantic: MachineSemanticKind::NormalizedForeignCall,
        constraint: key,
        memory: crate::MachineMemoryEffect::NoneV1,
        trap: crate::MachineTrapBehavior::NeverV1,
        barrier: MachineBarrier::Call,
        call: crate::MachineCallEffect::DirectExternalNormalReturnV1 {
            pre_call_stack_alignment: 16,
        },
        cleanup: crate::MachineCleanupEffect::NoneV1,
        alternatives: vec![MachineAlternative {
            key: MachineAlternativeKey {
                family: MachineSemanticKind::NormalizedForeignCall.into(),
                variant: 0,
            },
            applicability: MachineAlternativeApplicability::Always,
            size: MachineSizeKnowledge::ExactBytes(5),
            latency: MachineLatencyKnowledge::StableBaselineUnavailable,
            encoded,
        }],
    };
    let encoded = |row: &RegisterInstructionConstraint| {
        let mut encoded = MachineEncodedEffects::fallthrough_v1(
            row.operands
                .iter()
                .filter(|operand| operand.access == RegisterOperandAccess::Use)
                .map(|operand| operand.operand)
                .collect(),
            row.operands
                .iter()
                .filter(|operand| operand.access == RegisterOperandAccess::Def)
                .map(|operand| operand.operand)
                .collect(),
        );
        encoded.implicit_unit_uses = row.implicit_uses.clone();
        encoded.implicit_unit_defs = row.implicit_defs.clone();
        encoded.implicit_unit_clobbers = row.clobbers.clone();
        encoded.memory = MachineEncodedMemoryEffect::WriteReturnAddressBelowStackPointerV1 {
            stack_pointer: RegisterViewId(9),
            byte_count: 8,
        };
        encoded.stack = MachineEncodedStackEffect::CallReturnAddressLifecycleV1 {
            stack_pointer: RegisterViewId(9),
            return_address_byte_count: 8,
        };
        encoded.trap = MachineEncodedTrapBehavior::MayArchitecturalFaultV1;
        encoded.control = MachineEncodedControlEffect::DirectRelativeCallV1;
        encoded
    };
    let row = foreign_row(3000, 2, true);
    let source = declaration(row.key, encoded(&row));
    validate_declaration(&row, &source).unwrap();
    // The Unit-result row admits the same surfaces with no Def operand.
    let unit_row = foreign_row(3010, 1, false);
    validate_declaration(&unit_row, &declaration(unit_row.key, encoded(&unit_row))).unwrap();

    // An internal or absent call effect cannot stand in for the evaluated
    // foreign boundary's external return contract.
    for call in [
        crate::MachineCallEffect::NoneV1,
        crate::MachineCallEffect::DirectInternalNormalReturnV1 {
            pre_call_stack_alignment: 16,
        },
    ] {
        let mut changed = source.clone();
        changed.call = call;
        assert_eq!(
            validate_declaration(&row, &changed),
            Err(MachineEffectCatalogValidationError::InvalidEncodedEffects(
                MachineSemanticKind::NormalizedForeignCall
            )),
            "{call:?} must reject on a foreign declaration"
        );
    }
    let mut changed = source.clone();
    changed.barrier = MachineBarrier::None;
    assert_eq!(
        validate_declaration(&row, &changed),
        Err(MachineEffectCatalogValidationError::BarrierMismatch(
            MachineSemanticKind::NormalizedForeignCall
        ))
    );
    // Every operand must pin its plan-selected ABI view: an allocatable row
    // cannot launder a foreign placement.
    let mut unpinned = row.clone();
    unpinned.operands[0].fixed_view = None;
    assert_eq!(
        validate_declaration(&unpinned, &source),
        Err(MachineEffectCatalogValidationError::InvalidEncodedEffects(
            MachineSemanticKind::NormalizedForeignCall
        ))
    );
    // A two-result aggregate shape is not a normalized foreign plan row.
    let mut doubled = row.clone();
    doubled.operands.push(RegisterOperandConstraint {
        operand: 3,
        access: RegisterOperandAccess::Def,
        class: RegisterClassId(0),
        fixed_view: Some(RegisterViewId(3)),
        tied_to: None,
        early_clobber: false,
    });
    assert_eq!(
        validate_declaration(&doubled, &source),
        Err(MachineEffectCatalogValidationError::InvalidEncodedEffects(
            MachineSemanticKind::NormalizedForeignCall
        ))
    );
    // The encoded surface cannot borrow fall-through control or drop the
    // row's declared clobbers.
    let mut changed = source.clone();
    changed.alternatives[0].encoded.control = MachineEncodedControlEffect::FallThroughV1;
    assert_eq!(
        validate_declaration(&row, &changed),
        Err(MachineEffectCatalogValidationError::InvalidEncodedEffects(
            MachineSemanticKind::NormalizedForeignCall
        ))
    );
    let mut changed = source;
    changed.alternatives[0].encoded.implicit_unit_clobbers.pop();
    assert_eq!(
        validate_declaration(&row, &changed),
        Err(MachineEffectCatalogValidationError::InvalidEncodedEffects(
            MachineSemanticKind::NormalizedForeignCall
        ))
    );
}
