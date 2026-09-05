//! Exact System V AMD64 scalar-call semantic and encoded machine effects.

use register_model::ValidatedRegisterConstraintCatalog;
use selected_instructions::{
    MachineAlternative, MachineAlternativeApplicability, MachineAlternativeFamily,
    MachineAlternativeKey, MachineBarrier, MachineCallEffect, MachineCleanupEffect,
    MachineEffectDeclaration, MachineEncodedControlEffect, MachineEncodedEffects,
    MachineEncodedMemoryEffect, MachineEncodedStackEffect, MachineEncodedTrapBehavior,
    MachineLatencyKnowledge, MachineMemoryEffect, MachineSemanticKind, MachineSizeKnowledge,
    MachineTrapBehavior, SelectedConstraintKeys,
};

pub(super) fn declaration(
    keys: SelectedConstraintKeys,
    constraints: &ValidatedRegisterConstraintCatalog,
) -> MachineEffectDeclaration {
    let constraint = keys
        .call_i64_2_u64_to_u64
        .expect("scalar-call declaration requires a selected constraint");
    let row = constraints
        .catalog()
        .constraints
        .iter()
        .find(|row| row.key == constraint)
        .expect("canonical x86-64 catalog contains its scalar-call constraint");
    let physical = crate::x86_64_physical_register_model();
    let stack_pointer = physical
        .view_named("rsp")
        .expect("canonical x86-64 model declares rsp")
        .id;
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
            size: MachineSizeKnowledge::ExactBytes(5),
            latency: MachineLatencyKnowledge::StableBaselineUnavailable,
            encoded: MachineEncodedEffects {
                external_operand_reads: vec![0, 1],
                external_operand_writes: vec![2],
                implicit_unit_uses: row.implicit_uses.clone(),
                implicit_unit_defs: row.implicit_defs.clone(),
                implicit_unit_clobbers: row.clobbers.clone(),
                memory: MachineEncodedMemoryEffect::WriteReturnAddressBelowStackPointerV1 {
                    stack_pointer,
                    byte_count: 8,
                },
                stack: MachineEncodedStackEffect::CallReturnAddressLifecycleV1 {
                    stack_pointer,
                    return_address_byte_count: 8,
                },
                trap: MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
                control: MachineEncodedControlEffect::DirectRelativeCallV1,
            },
        }],
    }
}
