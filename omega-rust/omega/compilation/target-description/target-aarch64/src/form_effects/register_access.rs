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

pub fn effects(read: bool) -> MachineEncodedEffects {
    let physical = crate::aarch64_physical_register_model_ref();
    let scratch = if read {
        &["x9", "x15"][..]
    } else {
        &["x9", "x10", "x11", "x15"][..]
    };
    let mut clobbers = scratch
        .iter()
        .flat_map(|name| {
            physical
                .view_named(name)
                .expect("canonical AArch64 scratch register")
                .units
                .iter()
                .copied()
        })
        .collect::<Vec<_>>();
    clobbers.sort_unstable();
    clobbers.dedup();
    MachineEncodedEffects {
        implicit_unit_clobbers: clobbers,
        trap: MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
        ..MachineEncodedEffects::fallthrough_v1(
            if read { vec![] } else { vec![0] },
            if read { vec![0] } else { vec![] },
        )
    }
}

pub fn declaration(read: bool, constraint: RegisterConstraintKey) -> MachineEffectDeclaration {
    MachineEffectDeclaration::single(
        if read {
            MachineSemanticKind::SystemRegisterRead
        } else {
            MachineSemanticKind::SystemRegisterWrite
        },
        constraint,
        MachineMemoryEffect::NoneV1,
        MachineTrapBehavior::MayArchitecturalFaultV1,
        MachineBarrier::ExternalEffect,
        if read {
            MachineAlternativeFamily::SystemRegisterRead
        } else {
            MachineAlternativeFamily::SystemRegisterWrite
        },
        MachineSizeKnowledge::ExactBytes(4),
        effects(read),
    )
}
