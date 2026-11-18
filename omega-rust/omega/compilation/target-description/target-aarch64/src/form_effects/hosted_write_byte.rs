use crate::form_effects::MAX_ADDRESS_BYTES;
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

pub fn effects(target: NativeTarget) -> MachineEncodedEffects {
    let physical = crate::aarch64_physical_register_model();
    let view = |name: &str| {
        physical
            .view_named(name)
            .expect("canonical hosted write register")
    };
    let mut uses = view("sp").units.clone();
    uses.extend(view("pc").units.iter().copied());
    uses.sort_unstable();
    uses.dedup();
    let mut clobbers = Vec::new();
    let syscall_register = if target == NativeTarget::macos_arm64() {
        "x16"
    } else {
        "x8"
    };
    for name in ["x0", "x1", "x2", syscall_register, "nzcv"] {
        clobbers.extend(view(name).units.iter().copied());
    }
    clobbers.sort_unstable();
    clobbers.dedup();
    MachineEncodedEffects {
        external_operand_reads: vec![0],
        external_operand_writes: Vec::new(),
        implicit_unit_uses: uses,
        implicit_unit_defs: Vec::new(),
        implicit_unit_clobbers: clobbers,
        memory: MachineEncodedMemoryEffect::HostedWriteByteV1 {
            stack_pointer: view("sp").id,
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
        {
            let short = if target == NativeTarget::macos_arm64() {
                40
            } else {
                36
            };
            MachineSizeKnowledge::EncoderResolved {
                minimum_bytes: short,
                maximum_bytes: Some(short + MAX_ADDRESS_BYTES),
            }
        },
        effects(target),
    )
}
