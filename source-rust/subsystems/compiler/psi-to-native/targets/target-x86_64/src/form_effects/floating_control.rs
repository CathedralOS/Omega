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
use selected_instructions::MachineTrapBehavior;

pub fn effects(restore: bool) -> MachineEncodedEffects {
    let physical = crate::x86_64_physical_register_model_ref();
    let view = |name| {
        physical
            .view_named(name)
            .expect("canonical floating-control register")
    };
    let mut uses = view("rsp").units.clone();
    if !restore {
        uses.extend(view("mxcsr").units.iter().copied());
    }
    uses.sort_unstable();
    MachineEncodedEffects {
        external_operand_reads: Vec::new(),
        external_operand_writes: Vec::new(),
        implicit_unit_uses: uses,
        implicit_unit_defs: if restore {
            view("mxcsr").units.clone()
        } else {
            Vec::new()
        },
        implicit_unit_clobbers: Vec::new(),
        memory: if restore {
            MachineEncodedMemoryEffect::ReadFrameStorageV1 {
                stack_pointer: view("rsp").id,
                byte_count: 4,
            }
        } else {
            MachineEncodedMemoryEffect::WriteFrameStorageV1 {
                stack_pointer: view("rsp").id,
                byte_count: 4,
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
        crate::encoder_resolved(4, Some(8)),
        effects(restore),
    )
}
