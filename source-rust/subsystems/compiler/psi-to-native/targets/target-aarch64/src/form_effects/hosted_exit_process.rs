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

pub fn effects(
    target: NativeTarget,
) -> Result<MachineEncodedEffects, crate::form_effects::MissingRegister> {
    let physical = crate::aarch64_physical_register_model_ref();
    let view = |name: &str| {
        physical
            .view_named(name)
            .ok_or(crate::form_effects::MissingRegister)
    };
    let mut clobbers = Vec::new();
    let syscall_register = if target == NativeTarget::macos_arm64() {
        "x16"
    } else {
        "x8"
    };
    for name in ["x0", syscall_register, "nzcv"] {
        clobbers.extend(view(name)?.units.iter().copied());
    }
    clobbers.sort_unstable();
    clobbers.dedup();
    Ok(MachineEncodedEffects {
        external_operand_reads: vec![0],
        external_operand_writes: Vec::new(),
        implicit_unit_uses: view("pc")?.units.clone(),
        implicit_unit_defs: Vec::new(),
        implicit_unit_clobbers: clobbers,
        memory: MachineEncodedMemoryEffect::NoneV1,
        stack: MachineEncodedStackEffect::UnchangedV1,
        trap: MachineEncodedTrapBehavior::HostedExitReturnedV1,
        control: MachineEncodedControlEffect::HostedExitOrTrapV1,
    })
}

pub fn declaration(
    target: NativeTarget,
    constraint: RegisterConstraintKey,
) -> Result<MachineEffectDeclaration, crate::form_effects::MissingRegister> {
    Ok(MachineEffectDeclaration::single(
        MachineSemanticKind::HostedExitProcessI32,
        constraint,
        MachineMemoryEffect::NoneV1,
        MachineTrapBehavior::HostedExitReturnedV1,
        MachineBarrier::ExternalEffect,
        MachineAlternativeFamily::HostedExitProcessI32,
        MachineSizeKnowledge::ExactBytes(16),
        effects(target)?,
    ))
}
