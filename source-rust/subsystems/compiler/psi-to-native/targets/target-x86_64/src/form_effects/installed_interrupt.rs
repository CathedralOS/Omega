use crate::register_model::units_of;
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
use selected_instructions::SelectedInstructionKind;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Form {
    CpuCheck,
    TaskCommit,
    TablePublication,
}

impl Form {
    pub fn from_kind(kind: SelectedInstructionKind) -> Option<Self> {
        Some(match kind {
            SelectedInstructionKind::InstalledInterruptCpuCheck { .. } => Self::CpuCheck,
            SelectedInstructionKind::InstalledInterruptTaskCommit { .. } => Self::TaskCommit,
            SelectedInstructionKind::InstalledInterruptTablePublication { .. } => {
                Self::TablePublication
            }
            _ => return None,
        })
    }
    pub fn from_semantic(semantic: MachineSemanticKind) -> Option<Self> {
        Some(match semantic {
            MachineSemanticKind::InstalledInterruptCpuCheck => Self::CpuCheck,
            MachineSemanticKind::InstalledInterruptTaskCommit => Self::TaskCommit,
            MachineSemanticKind::InstalledInterruptTablePublication => Self::TablePublication,
            _ => return None,
        })
    }
    pub fn semantic(self) -> MachineSemanticKind {
        match self {
            Self::CpuCheck => MachineSemanticKind::InstalledInterruptCpuCheck,
            Self::TaskCommit => MachineSemanticKind::InstalledInterruptTaskCommit,
            Self::TablePublication => MachineSemanticKind::InstalledInterruptTablePublication,
        }
    }
    pub fn memory(self) -> MachineMemoryEffect {
        match self {
            Self::CpuCheck => MachineMemoryEffect::InstalledInterruptCpuCheckV1,
            Self::TaskCommit => MachineMemoryEffect::InstalledInterruptTaskCommitV1,
            Self::TablePublication => MachineMemoryEffect::InstalledInterruptTablePublicationV1,
        }
    }
    pub fn registers(self) -> &'static [u8] {
        match self {
            Self::CpuCheck => &[7, 6, 1, 0],
            Self::TaskCommit => &[7, 6, 1],
            Self::TablePublication => &[7, 1],
        }
    }
    pub fn input_count(self) -> usize {
        if self == Self::TablePublication { 2 } else { 3 }
    }
}

pub fn effects(form: Form) -> MachineEncodedEffects {
    let mut effects = MachineEncodedEffects::fallthrough_v1(
        (0..form.input_count() as u16).collect(),
        if form == Form::CpuCheck {
            vec![3]
        } else {
            Vec::new()
        },
    );
    effects.trap = MachineEncodedTrapBehavior::MayArchitecturalFaultV1;
    effects.implicit_unit_clobbers = units_of(match form {
        Form::CpuCheck => &["r9", "r10", "r11", "condition_flags"],
        Form::TaskCommit => &[
            "rax",
            "rdx",
            "r8",
            "r9",
            "r10",
            "r11",
            "condition_flags",
            "control_flags",
        ],
        Form::TablePublication => &["rax"],
    });
    if form == Form::TablePublication {
        effects.memory = MachineEncodedMemoryEffect::InstalledInterruptTablePublicationV1 {
            table_pointer_operand: 0,
            scratch_pointer_operand: 1,
        };
    } else {
        let stack_pointer = crate::x86_64_physical_register_model_ref()
            .view_named("rsp")
            .expect("canonical stack pointer")
            .id;
        effects.implicit_unit_uses = units_of(&["rsp", "control_flags"]);
        effects.stack = MachineEncodedStackEffect::TemporaryBytesV1 {
            stack_pointer,
            byte_count: 8,
        };
        effects.memory = if form == Form::CpuCheck {
            MachineEncodedMemoryEffect::InstalledInterruptCpuCheckV1 {
                table_pointer_operand: 0,
                task_pointer_operand: 1,
                scratch_pointer_operand: 2,
                stack_pointer,
            }
        } else {
            effects.control = MachineEncodedControlEffect::FallThroughOrStopV1;
            MachineEncodedMemoryEffect::InstalledInterruptTaskCommitV1 {
                table_pointer_operand: 0,
                task_pointer_operand: 1,
                scratch_pointer_operand: 2,
                stack_pointer,
            }
        };
    }
    effects
}

pub fn declaration(form: Form, constraint: RegisterConstraintKey) -> MachineEffectDeclaration {
    MachineEffectDeclaration::single(
        form.semantic(),
        constraint,
        form.memory(),
        MachineTrapBehavior::MayArchitecturalFaultV1,
        MachineBarrier::ExternalEffect,
        form.semantic().into(),
        crate::encoder_resolved(1, Some(2048)),
        effects(form),
    )
}
