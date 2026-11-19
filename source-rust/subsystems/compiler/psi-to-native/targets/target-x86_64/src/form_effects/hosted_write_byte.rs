use crate::register_model::units_of;
use crate::register_model::view_of;
use register_model::RegisterConstraintKey;
use selected_instructions::MachineAlternativeFamily;
use selected_instructions::MachineBarrier;
use selected_instructions::MachineEffectDeclaration;
use selected_instructions::MachineEncodedControlEffect;
use selected_instructions::MachineEncodedEffects;
use selected_instructions::MachineEncodedMemoryEffect;
use selected_instructions::MachineEncodedStackEffect;
use selected_instructions::MachineEncodedTrapBehavior;
use selected_instructions::MachineMemoryEffect;
use selected_instructions::MachineSemanticKind;
use selected_instructions::MachineSizeKnowledge;
use selected_instructions::MachineTrapBehavior;
use target::NativeTarget;

pub fn leaf_bytes(target: NativeTarget) -> u16 {
    if crate::hosted_kernel::hosted_kernel(target)
        .is_some_and(|kernel| kernel.carry_reports_failure)
    {
        42
    } else {
        40
    }
}

pub fn effects() -> MachineEncodedEffects {
    MachineEncodedEffects {
        external_operand_reads: vec![0],
        external_operand_writes: Vec::new(),
        implicit_unit_uses: units_of(&["rsp", "rip"]),
        implicit_unit_defs: Vec::new(),
        implicit_unit_clobbers: units_of(&["rax", "rdi", "rsi", "rdx", "rcx", "r11", "rflags"]),
        memory: MachineEncodedMemoryEffect::HostedWriteByteV1 {
            stack_pointer: view_of("rsp").id,
        },
        stack: MachineEncodedStackEffect::UnchangedV1,
        trap: MachineEncodedTrapBehavior::HostedWriteFailureV1,
        control: MachineEncodedControlEffect::HostedWriteReturnOrTrapV1,
    }
}

pub fn declaration(
    target: NativeTarget,
    constraint: RegisterConstraintKey,
) -> MachineEffectDeclaration {
    MachineEffectDeclaration::single(
        MachineSemanticKind::HostedWriteByteI32,
        constraint,
        MachineMemoryEffect::HostedWriteByteV1,
        MachineTrapBehavior::HostedWriteFailureV1,
        MachineBarrier::ExternalEffect,
        MachineAlternativeFamily::HostedWriteByteI32,
        MachineSizeKnowledge::ExactBytes(leaf_bytes(target)),
        effects(),
    )
}
