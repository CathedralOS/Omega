use register_model::RegisterConstraintKey;
use selected_instructions::MachineAlternativeFamily;
use selected_instructions::MachineBarrier;
use selected_instructions::MachineEffectDeclaration;
use selected_instructions::MachineEncodedEffects;
use selected_instructions::MachineMemoryEffect;
use selected_instructions::MachineSemanticKind;
use selected_instructions::MachineSizeKnowledge;
use selected_instructions::MachineTrapBehavior;

pub fn declaration(constraint: RegisterConstraintKey) -> MachineEffectDeclaration {
    MachineEffectDeclaration::single(
        MachineSemanticKind::MemoryFence,
        constraint,
        MachineMemoryEffect::CatalogFenceV1,
        MachineTrapBehavior::NeverV1,
        MachineBarrier::ExternalEffect,
        MachineAlternativeFamily::MemoryFence,
        MachineSizeKnowledge::ExactBytes(4),
        MachineEncodedEffects::catalog_fence_v1(),
    )
}
