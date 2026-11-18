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

pub fn size(target: NativeTarget) -> MachineSizeKnowledge {
    let address = crate::form_effects::MAX_ADDRESS_BYTES;
    if target == NativeTarget::macos_arm64() {
        MachineSizeKnowledge::EncoderResolved {
            minimum_bytes: 64,
            maximum_bytes: Some(56 + 2 * address),
        }
    } else {
        MachineSizeKnowledge::EncoderResolved {
            minimum_bytes: 56,
            maximum_bytes: Some(52 + address),
        }
    }
}

pub fn effects(target: NativeTarget) -> MachineEncodedEffects {
    let physical = crate::aarch64_physical_register_model();
    let view = |name: &str| {
        physical
            .view_named(name)
            .expect("canonical hosted read register")
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
    for name in ["x0", "x1", "x2", syscall_register, "x9", "nzcv"] {
        clobbers.extend(view(name).units.iter().copied());
    }
    clobbers.sort_unstable();
    clobbers.dedup();
    MachineEncodedEffects {
        external_operand_reads: Vec::new(),
        external_operand_writes: Vec::new(),
        implicit_unit_uses: uses,
        implicit_unit_defs: Vec::new(),
        implicit_unit_clobbers: clobbers,
        memory: MachineEncodedMemoryEffect::HostedReadByteV1 {
            stack_pointer: view("sp").id,
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
        size(target),
        effects(target),
    )
}
