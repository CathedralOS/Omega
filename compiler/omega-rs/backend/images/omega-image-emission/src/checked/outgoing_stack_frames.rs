use omega_machine_bytes::{CompilerInstructionValidationKind, EncodedMachineInstruction};
use omega_target::Architecture;
use psi_diagnostics::Diagnostic;

pub(super) fn validate_outgoing_stack_frames(
    architecture: Architecture,
    instructions: &[EncodedMachineInstruction],
) -> Result<(), Diagnostic> {
    let mut active = None;
    for instruction in instructions {
        match instruction.compiler_validation_kind {
            Some(CompilerInstructionValidationKind::OutgoingStackFrameReserve { byte_count }) => {
                validate_byte_count(architecture, byte_count)?;
                if active.replace(byte_count).is_some() {
                    return Err(Diagnostic::error(
                        "final compiler function nests outgoing stack-frame reservations",
                    ));
                }
            }
            Some(CompilerInstructionValidationKind::OutgoingStackFrameRelease { byte_count }) => {
                validate_byte_count(architecture, byte_count)?;
                match active.take() {
                    Some(reserved) if reserved == byte_count => {}
                    Some(reserved) => {
                        return Err(Diagnostic::error(format!(
                            "final compiler function releases {byte_count} outgoing stack bytes after reserving {reserved}"
                        )));
                    }
                    None => {
                        return Err(Diagnostic::error(
                            "final compiler function releases an unreserved outgoing stack frame",
                        ));
                    }
                }
            }
            Some(CompilerInstructionValidationKind::OutgoingStackAddressLoad {
                stack_byte_offset,
                ..
            }) => match active {
                Some(byte_count) if stack_byte_offset < byte_count => {}
                Some(byte_count) => {
                    return Err(Diagnostic::error(format!(
                        "final outgoing stack address offset {stack_byte_offset} escapes reserved {byte_count}-byte frame"
                    )));
                }
                None => {
                    return Err(Diagnostic::error(
                        "final outgoing stack address load has no active reserved frame",
                    ));
                }
            },
            _ => {}
        }
    }
    if let Some(byte_count) = active {
        return Err(Diagnostic::error(format!(
            "final compiler function leaves a {byte_count}-byte outgoing stack frame reserved"
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
