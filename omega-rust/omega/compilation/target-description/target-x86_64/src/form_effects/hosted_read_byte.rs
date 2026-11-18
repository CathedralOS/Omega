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
    if target == NativeTarget::macos_x64() {
        crate::hosted_kernel::DARWIN_READ_BYTE_LEAF_BYTES as u16
    } else {
        62
    }
}

pub fn effects() -> MachineEncodedEffects {
    MachineEncodedEffects {
        external_operand_reads: Vec::new(),
        external_operand_writes: Vec::new(),
        implicit_unit_uses: units_of(&["rsp", "rip"]),
        implicit_unit_defs: Vec::new(),
        implicit_unit_clobbers: units_of(&["rax", "rdi", "rsi", "rdx", "rcx", "r11", "rflags"]),
        memory: MachineEncodedMemoryEffect::HostedReadByteV1 {
            stack_pointer: view_of("rsp").id,
        },
        stack: MachineEncodedStackEffect::UnchangedV1,
        trap: MachineEncodedTrapBehavior::HostedReadFailureV1,
        control: MachineEncodedControlEffect::HostedReadReturnOrTrapV1,
    }
}

pub fn declaration(
    target: NativeTarget,
    constraint: RegisterConstraintKey,
) -> MachineEffectDeclaration {
    MachineEffectDeclaration::single(
        MachineSemanticKind::HostedReadByte,
        constraint,
        MachineMemoryEffect::HostedReadByteV1,
        MachineTrapBehavior::HostedReadFailureV1,
        MachineBarrier::ExternalEffect,
        MachineAlternativeFamily::HostedReadByte,
        MachineSizeKnowledge::ExactBytes(leaf_bytes(target)),
        effects(),
    )
}
