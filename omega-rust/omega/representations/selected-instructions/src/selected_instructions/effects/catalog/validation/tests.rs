//! Fail-closed catalog admission for the closed Linux byte-output leaf.
use super::*;
use crate::{MachineAlternative, MachineAlternativeKey, MachineLatencyKnowledge};
use register_model::{
    RegisterClassId, RegisterConstraintFamily, RegisterConstraintId, RegisterConstraintKey,
    RegisterOperandConstraint, RegisterViewId,
};

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
    encoded.memory = MachineEncodedMemoryEffect::LinuxWriteByteV1 {
        stack_pointer: RegisterViewId(7),
    };
    encoded.control = MachineEncodedControlEffect::LinuxWriteReturnOrTrapV1;
    encoded.trap = MachineEncodedTrapBehavior::LinuxWriteFailureV1;
    let source = MachineEffectDeclaration {
        semantic: MachineSemanticKind::LinuxWriteByteI32,
        constraint: constraint.key,
        memory: crate::MachineMemoryEffect::LinuxWriteByteV1,
        trap: crate::MachineTrapBehavior::LinuxWriteFailureV1,
        barrier: MachineBarrier::ExternalEffect,
        call: crate::MachineCallEffect::NoneV1,
        cleanup: crate::MachineCleanupEffect::NoneV1,
        alternatives: vec![MachineAlternative {
            key: MachineAlternativeKey {
                family: MachineSemanticKind::LinuxWriteByteI32.into(),
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
