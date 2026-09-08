//! Fail-closed catalog admission for the closed hosted byte-output leaf.
use super::*;
use crate::{MachineAlternative, MachineAlternativeKey, MachineLatencyKnowledge};
use register_model::{
    RegisterClassId, RegisterConstraintFamily, RegisterConstraintId, RegisterConstraintKey,
    RegisterOperandConstraint, RegisterViewId,
};

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
        (MachineSemanticKind::Load32, 4),
        (MachineSemanticKind::Load64, 8),
    ] {
        let mut encoded = MachineEncodedEffects::fallthrough_v1(vec![0], vec![1]);
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
