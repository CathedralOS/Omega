use crate::register_model::units_of;
use register_model::RegisterConstraintKey;
use selected_instructions::MachineAlternativeFamily;
use selected_instructions::MachineBarrier;
use selected_instructions::MachineEffectDeclaration;
use selected_instructions::MachineEncodedEffects;
use selected_instructions::MachineEncodedTrapBehavior;
use selected_instructions::MachineMemoryEffect;
use selected_instructions::MachineSemanticKind;
use selected_instructions::MachineSizeKnowledge;
use selected_instructions::MachineTrapBehavior;

pub fn effects() -> MachineEncodedEffects {
    MachineEncodedEffects {
        implicit_unit_clobbers: units_of(&["rax", "rdx", "r10", "r11"]),
        trap: MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
        ..MachineEncodedEffects::fallthrough_v1(Vec::new(), Vec::new())
    }
}

pub fn declaration(constraint: RegisterConstraintKey) -> MachineEffectDeclaration {
    MachineEffectDeclaration::single(
        MachineSemanticKind::PortWrite,
        constraint,
        MachineMemoryEffect::NoneV1,
        MachineTrapBehavior::MayArchitecturalFaultV1,
        MachineBarrier::ExternalEffect,
        MachineAlternativeFamily::PortWrite,
        MachineSizeKnowledge::ExactBytes(IMMEDIATE_PORT_WRITE_WIDTH as u16),
        effects(),
    )
}

pub const IMMEDIATE_PORT_WRITE_WIDTH: usize = 27;
