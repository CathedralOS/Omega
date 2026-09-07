//! Ordinary pointer memory, symbolic outgoing storage, and Unit call effects.

use register_model::{RegisterConstraintKey, ValidatedRegisterConstraintCatalog};
use selected_instructions::{
    MachineAlternative, MachineAlternativeApplicability, MachineAlternativeKey, MachineBarrier,
    MachineCallEffect, MachineCleanupEffect, MachineEffectDeclaration, MachineEncodedControlEffect,
    MachineEncodedEffects, MachineEncodedMemoryEffect, MachineEncodedStackEffect,
    MachineEncodedTrapBehavior, MachineLatencyKnowledge, MachineMemoryEffect, MachineSemanticKind,
    MachineSizeKnowledge, MachineTrapBehavior,
};

pub(super) fn declaration(
    semantic: MachineSemanticKind,
    constraint: RegisterConstraintKey,
    constraints: &ValidatedRegisterConstraintCatalog,
) -> MachineEffectDeclaration {
    let row = constraints
        .catalog()
        .constraints
        .iter()
        .find(|row| row.key == constraint)
        .expect("canonical memory instruction constraint");
    let stack_pointer = crate::x86_64_physical_register_model()
        .view_named("rsp")
        .expect("canonical stack pointer")
        .id;
    let (memory, trap, reads, writes, encoded_memory, size) = match semantic {
        MachineSemanticKind::Load8Indexed => (
            MachineMemoryEffect::ReadPointerV1,
            MachineTrapBehavior::MayArchitecturalFaultV1,
            vec![0, 1],
            vec![2],
            MachineEncodedMemoryEffect::ReadIndexedPointerV1 {
                pointer_operand: 0,
                index_operand: 1,
                byte_count: 1,
            },
            MachineSizeKnowledge::ExactBytes(9),
        ),
        MachineSemanticKind::Load64 => (
            MachineMemoryEffect::ReadPointerV1,
            MachineTrapBehavior::MayArchitecturalFaultV1,
            vec![0],
            vec![1],
            MachineEncodedMemoryEffect::ReadPointerV1 {
                pointer_operand: 0,
                byte_count: 8,
            },
            MachineSizeKnowledge::EncoderResolved {
                minimum_bytes: 7,
                maximum_bytes: Some(8),
            },
        ),
        MachineSemanticKind::Store64 => (
            MachineMemoryEffect::WriteOutgoingArgumentV1,
            MachineTrapBehavior::MayArchitecturalFaultV1,
            vec![0],
            vec![],
            MachineEncodedMemoryEffect::WriteOutgoingArgumentV1 {
                stack_pointer,
                byte_count: 8,
            },
            MachineSizeKnowledge::ExactBytes(8),
        ),
        MachineSemanticKind::FrameAddress => (
            MachineMemoryEffect::NoneV1,
            MachineTrapBehavior::NeverV1,
            vec![],
            vec![0],
            MachineEncodedMemoryEffect::NoneV1,
            MachineSizeKnowledge::ExactBytes(8),
        ),
        MachineSemanticKind::CallUnit => (
            MachineMemoryEffect::NoneV1,
            MachineTrapBehavior::NeverV1,
            row.operands.iter().map(|operand| operand.operand).collect(),
            vec![],
            MachineEncodedMemoryEffect::WriteReturnAddressBelowStackPointerV1 {
                stack_pointer,
                byte_count: 8,
            },
            MachineSizeKnowledge::ExactBytes(5),
        ),
        _ => unreachable!("ordinary address/call declaration only"),
    };
    let is_call = semantic == MachineSemanticKind::CallUnit;
    MachineEffectDeclaration {
        semantic,
        constraint,
        memory,
        trap,
        barrier: if is_call {
            MachineBarrier::Call
        } else {
            MachineBarrier::None
        },
        call: if is_call {
            MachineCallEffect::DirectInternalNormalReturnV1 {
                pre_call_stack_alignment: 16,
            }
        } else {
            MachineCallEffect::NoneV1
        },
        cleanup: MachineCleanupEffect::NoneV1,
        alternatives: vec![MachineAlternative {
            key: MachineAlternativeKey {
                family: semantic.into(),
                variant: 0,
            },
            applicability: MachineAlternativeApplicability::Always,
            size,
            latency: MachineLatencyKnowledge::StableBaselineUnavailable,
            encoded: MachineEncodedEffects {
                external_operand_reads: reads,
                external_operand_writes: writes,
                implicit_unit_uses: row.implicit_uses.clone(),
                implicit_unit_defs: row.implicit_defs.clone(),
                implicit_unit_clobbers: row.clobbers.clone(),
                memory: encoded_memory,
                stack: if is_call {
                    MachineEncodedStackEffect::CallReturnAddressLifecycleV1 {
                        stack_pointer,
                        return_address_byte_count: 8,
                    }
                } else {
                    MachineEncodedStackEffect::UnchangedV1
                },
                trap: if semantic == MachineSemanticKind::FrameAddress {
                    MachineEncodedTrapBehavior::NeverV1
                } else {
                    MachineEncodedTrapBehavior::MayArchitecturalFaultV1
                },
                control: if is_call {
                    MachineEncodedControlEffect::DirectRelativeCallV1
                } else {
                    MachineEncodedControlEffect::FallThroughV1
                },
            },
        }],
    }
}
