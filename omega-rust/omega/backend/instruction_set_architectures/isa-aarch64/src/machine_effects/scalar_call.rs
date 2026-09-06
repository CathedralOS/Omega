//! Exact AAPCS64 scalar-call semantic and encoded machine effects.

use register_model::{RegisterConstraintKey, ValidatedRegisterConstraintCatalog};
use selected_instructions::{
    MachineAlternative, MachineAlternativeApplicability, MachineAlternativeFamily,
    MachineAlternativeKey, MachineBarrier, MachineCallEffect, MachineCleanupEffect,
    MachineEffectDeclaration, MachineEncodedControlEffect, MachineEncodedEffects,
    MachineEncodedMemoryEffect, MachineEncodedStackEffect, MachineEncodedTrapBehavior,
    MachineLatencyKnowledge, MachineMemoryEffect, MachineSemanticKind, MachineSizeKnowledge,
    MachineTrapBehavior,
};

pub(super) fn declaration(
    constraint: RegisterConstraintKey,
    constraints: &ValidatedRegisterConstraintCatalog,
) -> MachineEffectDeclaration {
    let row = constraints
        .catalog()
        .constraints
        .iter()
        .find(|row| row.key == constraint)
        .expect("canonical AArch64 catalog contains its scalar-call constraint");
    let arity = row.operands.len() - 1;
    MachineEffectDeclaration {
        semantic: MachineSemanticKind::CallI64,
        constraint,
        memory: MachineMemoryEffect::NoneV1,
        trap: MachineTrapBehavior::NeverV1,
        barrier: MachineBarrier::Call,
        call: MachineCallEffect::DirectInternalNormalReturnV1 {
            pre_call_stack_alignment: 16,
        },
        cleanup: MachineCleanupEffect::NoneV1,
        alternatives: vec![MachineAlternative {
            key: MachineAlternativeKey {
                family: MachineAlternativeFamily::CallI64,
                variant: 0,
            },
            applicability: MachineAlternativeApplicability::Always,
            size: MachineSizeKnowledge::ExactBytes(4),
            latency: MachineLatencyKnowledge::StableBaselineUnavailable,
            encoded: MachineEncodedEffects {
                external_operand_reads: (0..arity as u16).collect(),
                external_operand_writes: vec![arity as u16],
                implicit_unit_uses: row.implicit_uses.clone(),
                implicit_unit_defs: row.implicit_defs.clone(),
                implicit_unit_clobbers: row.clobbers.clone(),
                memory: MachineEncodedMemoryEffect::NoneV1,
                stack: MachineEncodedStackEffect::UnchangedV1,
                trap: MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
                control: MachineEncodedControlEffect::DirectRelativeCallV1,
            },
        }],
    }
}
