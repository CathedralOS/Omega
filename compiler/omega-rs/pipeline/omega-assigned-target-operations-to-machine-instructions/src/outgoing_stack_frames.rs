//! Fail-closed sequencing for compiler-private outgoing stack frames.

use omega_assigned_target_operations::{AssignedOperation, AssignedOperationKind};
use psi_diagnostics::Diagnostic;

pub(crate) fn validate_outgoing_stack_frames(
    target: omega_target::NativeTarget,
    instructions: &[AssignedOperation],
) -> Result<(), Diagnostic> {
    let mut active = None;
    for instruction in instructions {
        match instruction.kind {
            AssignedOperationKind::ReserveOutgoingStackFrame { byte_count } => {
                validate_frame_byte_count(target, byte_count)?;
                if active.replace(byte_count).is_some() {
                    return Err(Diagnostic::error(
                        "compiler-private outgoing stack frames may not nest",
                    ));
                }
            }
            AssignedOperationKind::ReleaseOutgoingStackFrame { byte_count } => {
                validate_frame_byte_count(target, byte_count)?;
                if active != Some(byte_count) {
                    return Err(Diagnostic::error(
                        "compiler-private outgoing stack-frame release has no exact matching reservation",
                    ));
                }
                active = None;
            }
            AssignedOperationKind::LoadOutgoingStackAddress {
                stack_byte_offset, ..
            } => {
                let Some(byte_count) = active else {
                    return Err(Diagnostic::error(
                        "outgoing stack-address load occurs outside a live reserved frame",
                    ));
                };
                if stack_byte_offset >= byte_count {
                    return Err(Diagnostic::error(
                        "outgoing stack-address load lies outside its live reserved frame",
                    ));
                }
            }
            _ => {}
        }
    }
    if active.is_some() {
        return Err(Diagnostic::error(
            "compiler-private outgoing stack-frame reservation is not released",
        ));
    }
    Ok(())
}

fn validate_frame_byte_count(
    target: omega_target::NativeTarget,
    byte_count: u32,
) -> Result<(), Diagnostic> {
    if target.architecture != omega_target::Architecture::X86_64 {
        return Err(Diagnostic::error(
            "compiler-private outgoing stack frames are supported only on x86-64",
        ));
    }
    if byte_count < 32 || byte_count > i32::MAX as u32 || byte_count % 16 != 8 {
        return Err(Diagnostic::error(
            "compiler-private outgoing stack frame must cover Microsoft x64 shadow space, fit positive disp32, and align pre-call RSP",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use omega_calling_conventions::MachineRegister;

    fn operation(kind: AssignedOperationKind) -> AssignedOperation {
        AssignedOperation {
            kind,
            ..Default::default()
        }
    }

    #[test]
    fn exact_balanced_frame_and_addresses_are_admitted() {
        validate_outgoing_stack_frames(
            omega_target::NativeTarget::uefi_x64(),
            &[
                operation(AssignedOperationKind::ReserveOutgoingStackFrame { byte_count: 72 }),
                operation(AssignedOperationKind::LoadOutgoingStackAddress {
                    register: MachineRegister::X86Rcx,
                    stack_byte_offset: 32,
                }),
                operation(AssignedOperationKind::LoadOutgoingStackAddress {
                    register: MachineRegister::X86Rdx,
                    stack_byte_offset: 48,
                }),
                operation(AssignedOperationKind::ReleaseOutgoingStackFrame { byte_count: 72 }),
            ],
        )
        .expect("exact balanced caller frame");
    }

    #[test]
    fn orphan_mismatch_nesting_and_unreleased_frames_reject() {
        let target = omega_target::NativeTarget::uefi_x64();
        for instructions in [
            vec![operation(
                AssignedOperationKind::ReleaseOutgoingStackFrame { byte_count: 72 },
            )],
            vec![
                operation(AssignedOperationKind::ReserveOutgoingStackFrame { byte_count: 72 }),
                operation(AssignedOperationKind::ReleaseOutgoingStackFrame { byte_count: 56 }),
            ],
            vec![
                operation(AssignedOperationKind::ReserveOutgoingStackFrame { byte_count: 72 }),
                operation(AssignedOperationKind::ReserveOutgoingStackFrame { byte_count: 72 }),
            ],
            vec![operation(
                AssignedOperationKind::ReserveOutgoingStackFrame { byte_count: 72 },
            )],
        ] {
            assert!(validate_outgoing_stack_frames(target, &instructions).is_err());
        }
    }

    #[test]
    fn invalid_size_target_and_address_range_reject() {
        for byte_count in [0, 24, 64, i32::MAX as u32 + 1] {
            assert!(
                validate_outgoing_stack_frames(
                    omega_target::NativeTarget::uefi_x64(),
                    &[operation(
                        AssignedOperationKind::ReserveOutgoingStackFrame { byte_count }
                    )],
                )
                .is_err()
            );
        }
        let balanced = [
            operation(AssignedOperationKind::ReserveOutgoingStackFrame { byte_count: 72 }),
            operation(AssignedOperationKind::ReleaseOutgoingStackFrame { byte_count: 72 }),
        ];
        assert!(
            validate_outgoing_stack_frames(omega_target::NativeTarget::linux_arm64(), &balanced)
                .is_err()
        );
        let out_of_range = [
            operation(AssignedOperationKind::ReserveOutgoingStackFrame { byte_count: 72 }),
            operation(AssignedOperationKind::LoadOutgoingStackAddress {
                register: MachineRegister::X86Rcx,
                stack_byte_offset: 72,
            }),
            operation(AssignedOperationKind::ReleaseOutgoingStackFrame { byte_count: 72 }),
        ];
        assert!(
            validate_outgoing_stack_frames(omega_target::NativeTarget::uefi_x64(), &out_of_range,)
                .is_err()
        );
    }
}
