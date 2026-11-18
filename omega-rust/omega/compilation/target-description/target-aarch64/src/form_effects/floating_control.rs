use register_model::RegisterConstraintKey;
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

pub fn clobbers(restore: bool) -> Vec<register_model::RegisterUnitId> {
    let physical = crate::aarch64_physical_register_model();
    let mut units = physical
        .view_named("x9")
        .expect("canonical floating-control scratch")
        .units
        .clone();
    if !restore {
        units.extend(
            physical
                .view_named("x16")
                .expect("canonical floating-control address scratch")
                .units
                .iter()
                .copied(),
        );
    }
    units.sort_unstable();
    units
}

pub fn effects(restore: bool) -> MachineEncodedEffects {
    let physical = crate::aarch64_physical_register_model();
    let view = |name| {
        physical
            .view_named(name)
            .expect("canonical floating-control register")
    };
    let mut uses = view("sp").units.clone();
    if !restore {
        uses.extend(view("fpcr").units.iter().copied());
    }
    uses.sort_unstable();
    MachineEncodedEffects {
        external_operand_reads: Vec::new(),
        external_operand_writes: Vec::new(),
        implicit_unit_uses: uses,
        implicit_unit_defs: if restore {
            view("fpcr").units.clone()
        } else {
            Vec::new()
        },
        implicit_unit_clobbers: clobbers(restore),
        memory: if restore {
            MachineEncodedMemoryEffect::ReadFrameStorageV1 {
                stack_pointer: view("sp").id,
                byte_count: 8,
            }
        } else {
            MachineEncodedMemoryEffect::WriteFrameStorageV1 {
                stack_pointer: view("sp").id,
                byte_count: 8,
            }
        },
        stack: MachineEncodedStackEffect::UnchangedV1,
        trap: MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
        control: MachineEncodedControlEffect::FallThroughV1,
    }
}

pub fn declaration(
    semantic: MachineSemanticKind,
    constraint: RegisterConstraintKey,
) -> MachineEffectDeclaration {
    let restore = semantic == MachineSemanticKind::RestoreFloatingControl;
    MachineEffectDeclaration::single(
        semantic,
        constraint,
        if restore {
            MachineMemoryEffect::ReadFrameStorageV1
        } else {
            MachineMemoryEffect::WriteFrameStorageV1
        },
        MachineTrapBehavior::MayArchitecturalFaultV1,
        MachineBarrier::ExternalEffect,
        semantic.into(),
        MachineSizeKnowledge::EncoderResolved {
            minimum_bytes: 8,
            maximum_bytes: Some(crate::form_effects::MAX_FPCR_SEQUENCE_BYTES),
        },
        effects(restore),
    )
}
