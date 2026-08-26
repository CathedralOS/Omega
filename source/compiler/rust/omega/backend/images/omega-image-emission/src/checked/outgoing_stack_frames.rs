use omega_machine_bytes::{CompilerInstructionValidationKind, EncodedMachineInstruction};
use omega_target::Architecture;
use psi_diagnostics::Diagnostic;

#[derive(Clone, Copy)]
struct ActiveFrame {
    byte_count: u32,
    next_write: usize,
    next_address: usize,
    write_mode: Option<WriteMode>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum WriteMode {
    Immediate,
    EntryIndirect,
}

pub(super) fn validate_outgoing_stack_frames(
    architecture: Architecture,
    instructions: &[EncodedMachineInstruction],
) -> Result<(), Diagnostic> {
    let mut active: Option<ActiveFrame> = None;
    for instruction in instructions {
        match instruction.compiler_validation_kind {
            Some(CompilerInstructionValidationKind::OutgoingStackFrameReserve { byte_count }) => {
                validate_byte_count(architecture, byte_count)?;
                if active
                    .replace(ActiveFrame {
                        byte_count,
                        next_write: 0,
                        next_address: 0,
                        write_mode: None,
                    })
                    .is_some()
                {
                    return Err(Diagnostic::error(
                        "final compiler function nests outgoing stack-frame reservations",
                    ));
                }
            }
            Some(CompilerInstructionValidationKind::OutgoingStackFrameRelease { byte_count }) => {
                validate_byte_count(architecture, byte_count)?;
                match active.take() {
                    Some(frame)
                        if frame.byte_count == byte_count
                            && (frame.next_write == 0
                                || (frame.next_write == 4 && frame.next_address == 2)) => {}
                    Some(frame) => {
                        return Err(Diagnostic::error(format!(
                            "final compiler function releases {byte_count} outgoing stack bytes before completing the frame reserved as {} bytes",
                            frame.byte_count
                        )));
                    }
                    None => {
                        return Err(Diagnostic::error(
                            "final compiler function releases an unreserved outgoing stack frame",
                        ));
                    }
                }
            }
            Some(CompilerInstructionValidationKind::OutgoingStackU64Write {
                stack_byte_offset,
                ..
            }) => {
                let Some(frame) = active.as_mut() else {
                    return Err(Diagnostic::error(
                        "final outgoing stack u64 write has no active reserved frame",
                    ));
                };
                let expected_offsets = [32, 40, 48, 56];
                if frame.byte_count != 72
                    || frame.next_address != 0
                    || frame.write_mode == Some(WriteMode::EntryIndirect)
                    || expected_offsets.get(frame.next_write).copied() != Some(stack_byte_offset)
                    || stack_byte_offset < 32
                    || stack_byte_offset % 8 != 0
                    || stack_byte_offset.checked_add(8).is_none_or(|end| end > 64)
                {
                    return Err(Diagnostic::error(
                        "final outgoing stack u64 write violates exact order or writable range",
                    ));
                }
                frame.write_mode = Some(WriteMode::Immediate);
                frame.next_write += 1;
            }
            Some(CompilerInstructionValidationKind::EntryIndirectU64ToOutgoingStackCopy {
                source_register,
                source_byte_offset,
                stack_byte_offset,
            }) => {
                let Some(frame) = active.as_mut() else {
                    return Err(Diagnostic::error(
                        "final entry-indirect outgoing stack copy has no active reserved frame",
                    ));
                };
                let expected = [
                    (omega_calling_conventions::MachineRegister::X86Rcx, 0, 32),
                    (omega_calling_conventions::MachineRegister::X86Rcx, 8, 40),
                    (omega_calling_conventions::MachineRegister::X86Rdx, 0, 48),
                    (omega_calling_conventions::MachineRegister::X86Rdx, 8, 56),
                ];
                if frame.byte_count != 72
                    || frame.next_address != 0
                    || frame.write_mode == Some(WriteMode::Immediate)
                    || expected.get(frame.next_write).copied()
                        != Some((source_register, source_byte_offset, stack_byte_offset))
                {
                    return Err(Diagnostic::error(
                        "final entry-indirect outgoing stack copy violates exact launch-value order or writable range",
                    ));
                }
                frame.write_mode = Some(WriteMode::EntryIndirect);
                frame.next_write += 1;
            }
            Some(CompilerInstructionValidationKind::OutgoingStackAddressLoad {
                register,
                stack_byte_offset,
            }) => {
                let Some(frame) = active.as_mut() else {
                    return Err(Diagnostic::error(
                        "final outgoing stack address load has no active reserved frame",
                    ));
                };
                let expected = [
                    (omega_calling_conventions::MachineRegister::X86Rcx, 32),
                    (omega_calling_conventions::MachineRegister::X86Rdx, 48),
                ];
                if frame.byte_count != 72
                    || frame.next_write != 4
                    || expected.get(frame.next_address).copied()
                        != Some((register, stack_byte_offset))
                {
                    return Err(Diagnostic::error(
                        "final outgoing stack address load precedes or drifts from exact caller-copy writes",
                    ));
                }
                frame.next_address += 1;
            }
            _ => {}
        }
    }
    if let Some(frame) = active {
        return Err(Diagnostic::error(format!(
            "final compiler function leaves a {}-byte outgoing stack frame reserved",
            frame.byte_count
        )));
    }
    Ok(())
}

fn validate_byte_count(architecture: Architecture, byte_count: u32) -> Result<(), Diagnostic> {
    if architecture != Architecture::X86_64 {
        return Err(Diagnostic::error(
            "outgoing stack frames are supported only on x86-64",
        ));
    }
    omega_isa_x86_64::outgoing_stack_frame_adjust_width(byte_count).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;
    use omega_calling_conventions::MachineRegister;

    fn row(kind: CompilerInstructionValidationKind) -> EncodedMachineInstruction {
        EncodedMachineInstruction {
            compiler_validation_kind: Some(kind),
            ..Default::default()
        }
    }

    fn exact_launch_frame() -> Vec<EncodedMachineInstruction> {
        let mut instructions = vec![row(
            CompilerInstructionValidationKind::OutgoingStackFrameReserve { byte_count: 72 },
        )];
        instructions.extend(
            [
                (MachineRegister::X86Rcx, 0, 32),
                (MachineRegister::X86Rcx, 8, 40),
                (MachineRegister::X86Rdx, 0, 48),
                (MachineRegister::X86Rdx, 8, 56),
            ]
            .into_iter()
            .map(|(source_register, source_byte_offset, stack_byte_offset)| {
                row(
                    CompilerInstructionValidationKind::EntryIndirectU64ToOutgoingStackCopy {
                        source_register,
                        source_byte_offset,
                        stack_byte_offset,
                    },
                )
            }),
        );
        instructions.extend([
            row(
                CompilerInstructionValidationKind::OutgoingStackAddressLoad {
                    register: MachineRegister::X86Rcx,
                    stack_byte_offset: 32,
                },
            ),
            row(
                CompilerInstructionValidationKind::OutgoingStackAddressLoad {
                    register: MachineRegister::X86Rdx,
                    stack_byte_offset: 48,
                },
            ),
            row(CompilerInstructionValidationKind::OutgoingStackFrameRelease { byte_count: 72 }),
        ]);
        instructions
    }

    #[test]
    fn final_launch_value_sequence_replays_exactly() {
        validate_outgoing_stack_frames(Architecture::X86_64, &exact_launch_frame())
            .expect("exact launch-value sequence");
    }

    #[test]
    fn final_launch_value_metadata_corruption_and_mixing_reject() {
        for corrupted in [
            CompilerInstructionValidationKind::EntryIndirectU64ToOutgoingStackCopy {
                source_register: MachineRegister::X86Rdx,
                source_byte_offset: 0,
                stack_byte_offset: 32,
            },
            CompilerInstructionValidationKind::EntryIndirectU64ToOutgoingStackCopy {
                source_register: MachineRegister::X86Rcx,
                source_byte_offset: 8,
                stack_byte_offset: 32,
            },
            CompilerInstructionValidationKind::EntryIndirectU64ToOutgoingStackCopy {
                source_register: MachineRegister::X86Rcx,
                source_byte_offset: 0,
                stack_byte_offset: 40,
            },
            CompilerInstructionValidationKind::OutgoingStackU64Write {
                stack_byte_offset: 32,
                value: 0,
            },
        ] {
            let mut instructions = exact_launch_frame();
            instructions[1] = row(corrupted);
            assert!(validate_outgoing_stack_frames(Architecture::X86_64, &instructions).is_err());
        }

        let mut missing = exact_launch_frame();
        missing.remove(2);
        assert!(validate_outgoing_stack_frames(Architecture::X86_64, &missing).is_err());
        assert!(
            validate_outgoing_stack_frames(Architecture::Aarch64, &exact_launch_frame()).is_err()
        );
    }
}
