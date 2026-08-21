use omega_machine_bytes::{CompilerInstructionValidationKind, EncodedMachineInstruction};
use omega_target::Architecture;
use psi_diagnostics::Diagnostic;

#[derive(Clone, Copy)]
struct ActiveFrame {
    byte_count: u32,
    next_write: usize,
    next_address: usize,
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
                    || expected_offsets.get(frame.next_write).copied() != Some(stack_byte_offset)
                    || stack_byte_offset < 32
                    || stack_byte_offset % 8 != 0
                    || stack_byte_offset.checked_add(8).is_none_or(|end| end > 64)
                {
                    return Err(Diagnostic::error(
                        "final outgoing stack u64 write violates exact order or writable range",
                    ));
                }
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
