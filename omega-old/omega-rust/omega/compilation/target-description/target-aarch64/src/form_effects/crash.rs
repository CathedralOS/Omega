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
        MachineSizeKnowledge::ExactBytes(4),
        MachineEncodedEffects::crash_v1(&constraint.implicit_uses),
    )
}
