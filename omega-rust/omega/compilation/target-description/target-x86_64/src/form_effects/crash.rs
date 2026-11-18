use register_model::RegisterInstructionConstraint;
use selected_instructions::MachineAlternativeFamily;
use selected_instructions::MachineBarrier;
use selected_instructions::MachineEffectDeclaration;
use selected_instructions::MachineEncodedEffects;
use selected_instructions::MachineMemoryEffect;
use selected_instructions::MachineSemanticKind;
use selected_instructions::MachineSizeKnowledge;
use selected_instructions::MachineTrapBehavior;

pub fn declaration(constraint: &RegisterInstructionConstraint) -> MachineEffectDeclaration {
    MachineEffectDeclaration::single(
        MachineSemanticKind::Crash,
        constraint.key,
        MachineMemoryEffect::NoneV1,
        MachineTrapBehavior::ExplicitCrashV1,
        MachineBarrier::ControlFlow,
        MachineAlternativeFamily::Crash,
        MachineSizeKnowledge::ExactBytes(2),
        MachineEncodedEffects::crash_v1(&constraint.implicit_uses),
    )
}

pub fn privileged_declaration(
    constraint: &RegisterInstructionConstraint,
) -> MachineEffectDeclaration {
    MachineEffectDeclaration::single(
        MachineSemanticKind::PrivilegedCrash,
        constraint.key,
        MachineMemoryEffect::NoneV1,
        MachineTrapBehavior::MayArchitecturalFaultV1,
        MachineBarrier::ControlFlow,
        MachineAlternativeFamily::PrivilegedCrash,
        MachineSizeKnowledge::ExactBytes(4),
        MachineEncodedEffects::privileged_stop_v1(&constraint.implicit_uses, &constraint.clobbers),
    )
}
