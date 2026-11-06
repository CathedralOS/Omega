//! Selected installation steps over explicitly admitted backing and geometry.
//! These encodings supply no authority: the owning Install operation must order
//! its validator, root retention, task commit and table publication.

use super::{
    ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError as Error,
    X86_64SelectedFormFootprint,
};
use crate::register_model::units_of;
use register_model::{RegisterConstraintKey, RegisterViewId, ValidatedPhysicalRegisterModel};
use selected_instructions::{
    MachineAlternativeKey, MachineBarrier, MachineEffectDeclaration, MachineEncodedControlEffect,
    MachineEncodedEffects, MachineEncodedMemoryEffect, MachineEncodedStackEffect,
    MachineEncodedTrapBehavior, MachineMemoryEffect, MachineSemanticKind, MachineTrapBehavior,
    SelectedInstructionKind,
};

impl From<crate::X86InstalledTableEncodingError> for Error {
    fn from(error: crate::X86InstalledTableEncodingError) -> Self {
        Self::InstalledTable(Box::new(error))
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Form {
    CpuCheck,
    TaskCommit,
    TablePublication,
}
impl Form {
    pub(crate) fn from_kind(kind: SelectedInstructionKind) -> Option<Self> {
        Some(match kind {
            SelectedInstructionKind::InstalledInterruptCpuCheck { .. } => Self::CpuCheck,
            SelectedInstructionKind::InstalledInterruptTaskCommit { .. } => Self::TaskCommit,
            SelectedInstructionKind::InstalledInterruptTablePublication { .. } => {
                Self::TablePublication
            }
            _ => return None,
        })
    }
    pub(crate) fn from_semantic(semantic: MachineSemanticKind) -> Option<Self> {
        Some(match semantic {
            MachineSemanticKind::InstalledInterruptCpuCheck => Self::CpuCheck,
            MachineSemanticKind::InstalledInterruptTaskCommit => Self::TaskCommit,
            MachineSemanticKind::InstalledInterruptTablePublication => Self::TablePublication,
            _ => return None,
        })
    }
    fn semantic(self) -> MachineSemanticKind {
        match self {
            Self::CpuCheck => MachineSemanticKind::InstalledInterruptCpuCheck,
            Self::TaskCommit => MachineSemanticKind::InstalledInterruptTaskCommit,
            Self::TablePublication => MachineSemanticKind::InstalledInterruptTablePublication,
        }
    }
    fn memory(self) -> MachineMemoryEffect {
        match self {
            Self::CpuCheck => MachineMemoryEffect::InstalledInterruptCpuCheckV1,
            Self::TaskCommit => MachineMemoryEffect::InstalledInterruptTaskCommitV1,
            Self::TablePublication => MachineMemoryEffect::InstalledInterruptTablePublicationV1,
        }
    }
    fn registers(self) -> &'static [u8] {
        match self {
            Self::CpuCheck => &[7, 6, 1, 0],
            Self::TaskCommit => &[7, 6, 1],
            Self::TablePublication => &[7, 1],
        }
    }
    fn input_count(self) -> usize {
        if self == Self::TablePublication { 2 } else { 3 }
    }
}

pub(crate) fn effects(form: Form) -> MachineEncodedEffects {
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

pub(crate) fn declaration(
    form: Form,
    constraint: RegisterConstraintKey,
) -> MachineEffectDeclaration {
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

fn request(
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
) -> Result<Form, Error> {
    let form = Form::from_kind(kind).ok_or(Error::AlternativeMismatch)?;
    super::request_validation::validate_request(physical, kind, alternative, operands)?;
    let registers = super::request_validation::resolve_scalar_registers(physical, kind, operands)?;
    if registers != form.registers() {
        return Err(Error::EncodedFormMismatch);
    }
    Ok(form)
}

pub(crate) fn encode(
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
) -> Result<ValidatedX86_64SelectedFormEncoding, Error> {
    request(physical, kind, alternative, operands)?;
    let bytes = match kind {
        SelectedInstructionKind::InstalledInterruptCpuCheck { gdt_bytes } => {
            crate::encode_x86_64_installed_cpu_state_check(
                crate::X86InstalledCpuStateRegisters::FIXED,
                crate::X86InstalledCpuStateGeometry { gdt_bytes },
            )?
        }
        SelectedInstructionKind::InstalledInterruptTaskCommit { gdt_bytes } => {
            crate::encode_x86_64_installed_task_commit(
                crate::X86InstalledTaskCommitRegisters::FIXED,
                crate::X86InstalledCpuStateGeometry { gdt_bytes },
            )?
        }
        SelectedInstructionKind::InstalledInterruptTablePublication { table_bytes } => {
            crate::encode_x86_64_installed_table_publication(
                crate::X86InstalledTablePublicationRegisters::FIXED,
                table_bytes,
            )?
        }
        _ => return Err(Error::AlternativeMismatch),
    };
    validate(physical, kind, alternative, operands, &bytes)
}

pub(crate) fn validate(
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
    bytes: &[u8],
) -> Result<ValidatedX86_64SelectedFormEncoding, Error> {
    let form = request(physical, kind, alternative, operands)?;
    match kind {
        SelectedInstructionKind::InstalledInterruptCpuCheck { gdt_bytes } => {
            crate::validate_x86_64_installed_cpu_state_check(
                crate::X86InstalledCpuStateRegisters::FIXED,
                crate::X86InstalledCpuStateGeometry { gdt_bytes },
                bytes,
            )?
        }
        SelectedInstructionKind::InstalledInterruptTaskCommit { gdt_bytes } => {
            crate::validate_x86_64_installed_task_commit(
                crate::X86InstalledTaskCommitRegisters::FIXED,
                crate::X86InstalledCpuStateGeometry { gdt_bytes },
                bytes,
            )?;
        }
        SelectedInstructionKind::InstalledInterruptTablePublication { table_bytes } => {
            crate::validate_x86_64_installed_table_publication(
                crate::X86InstalledTablePublicationRegisters::FIXED,
                table_bytes,
                bytes,
            )?
        }
        _ => return Err(Error::AlternativeMismatch),
    }
    Ok(ValidatedX86_64SelectedFormEncoding {
        bytes: bytes.to_vec(),
        footprint: footprint(form, operands),
    })
}

pub(crate) fn footprint(form: Form, operands: &[RegisterViewId]) -> X86_64SelectedFormFootprint {
    X86_64SelectedFormFootprint {
        register_reads: operands[..form.input_count()].to_vec(),
        register_writes: if form == Form::CpuCheck {
            vec![operands[3]]
        } else {
            Vec::new()
        },
        writes_rflags: form != Form::TablePublication,
        encoded: effects(form),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn operands(form: Form) -> Vec<RegisterViewId> {
        let names: &[&str] = match form {
            Form::CpuCheck => &["rdi", "rsi", "rcx", "rax"],
            Form::TaskCommit => &["rdi", "rsi", "rcx"],
            Form::TablePublication => &["rdi", "rcx"],
        };
        let model = crate::validated_x86_64_physical_register_model();
        names
            .iter()
            .map(|name| model.model().view_named(name).unwrap().id)
            .collect()
    }

    #[test]
    fn installed_selected_forms_replay_exact_geometry_and_registers() {
        let model = crate::validated_x86_64_physical_register_model();
        let kinds = [
            SelectedInstructionKind::InstalledInterruptCpuCheck { gdt_bytes: 40 },
            SelectedInstructionKind::InstalledInterruptTaskCommit { gdt_bytes: 40 },
            SelectedInstructionKind::InstalledInterruptTablePublication { table_bytes: 1024 },
        ];
        for kind in kinds {
            let form = Form::from_kind(kind).unwrap();
            let registers = operands(form);
            let alternative = MachineAlternativeKey {
                family: form.semantic().into(),
                variant: 0,
            };
            let encoded =
                crate::encode_x86_64_selected_form(model, kind, alternative, &registers).unwrap();
            assert!(
                crate::validate_x86_64_selected_form_encoding(
                    model,
                    kind,
                    alternative,
                    &registers,
                    encoded.bytes()
                )
                .is_ok()
            );
            let changed = match kind {
                SelectedInstructionKind::InstalledInterruptCpuCheck { .. } => {
                    SelectedInstructionKind::InstalledInterruptCpuCheck { gdt_bytes: 48 }
                }
                SelectedInstructionKind::InstalledInterruptTaskCommit { .. } => {
                    SelectedInstructionKind::InstalledInterruptTaskCommit { gdt_bytes: 48 }
                }
                _ => SelectedInstructionKind::InstalledInterruptTablePublication {
                    table_bytes: 1040,
                },
            };
            assert!(
                crate::validate_x86_64_selected_form_encoding(
                    model,
                    changed,
                    alternative,
                    &registers,
                    encoded.bytes()
                )
                .is_err()
            );
            let mut wrong_registers = registers.clone();
            wrong_registers[0] = model.model().view_named("rbx").unwrap().id;
            assert!(
                crate::validate_x86_64_selected_form_encoding(
                    model,
                    kind,
                    alternative,
                    &wrong_registers,
                    encoded.bytes()
                )
                .is_err()
            );
        }
    }

    #[test]
    fn selected_commit_cannot_drop_its_refusal_stop() {
        let model = crate::validated_x86_64_physical_register_model();
        let kind = SelectedInstructionKind::InstalledInterruptTaskCommit { gdt_bytes: 40 };
        let alternative = MachineAlternativeKey {
            family: MachineSemanticKind::InstalledInterruptTaskCommit.into(),
            variant: 0,
        };
        let operands = operands(Form::TaskCommit);
        let encoded =
            crate::encode_x86_64_selected_form(model, kind, alternative, &operands).unwrap();
        let bytes = encoded.bytes();
        assert!(
            crate::validate_x86_64_selected_form_encoding(
                model,
                kind,
                alternative,
                &operands,
                &bytes[..bytes.len() - crate::PRIVILEGED_STOP_BYTES.len()]
            )
            .is_err()
        );
        let mut reversed = bytes.to_vec();
        let stop = reversed.len() - crate::PRIVILEGED_STOP_BYTES.len();
        // NOPs would let a failed recheck return to the caller's LIDT.
        reversed[stop..].fill(0x90);
        assert!(
            crate::validate_x86_64_selected_form_encoding(
                model,
                kind,
                alternative,
                &operands,
                &reversed
            )
            .is_err()
        );
        assert_eq!(
            encoded.footprint().encoded.control,
            MachineEncodedControlEffect::FallThroughOrStopV1
        );
    }
}
