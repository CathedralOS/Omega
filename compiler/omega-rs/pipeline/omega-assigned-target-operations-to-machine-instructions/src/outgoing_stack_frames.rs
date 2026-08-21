//! Fail-closed sequencing for compiler-private outgoing stack frames.

use omega_assigned_target_operations::{AssignedOperation, AssignedOperationKind};
use psi_diagnostics::Diagnostic;

#[derive(Clone, Copy)]
struct ActiveFrame {
    byte_count: u32,
    next_write: usize,
    next_address: usize,
}

pub(crate) fn validate_outgoing_stack_frames(
    target: omega_target::NativeTarget,
    instructions: &[AssignedOperation],
) -> Result<(), Diagnostic> {
    let mut active: Option<ActiveFrame> = None;
    for instruction in instructions {
        match instruction.kind {
            AssignedOperationKind::ReserveOutgoingStackFrame { byte_count } => {
                validate_frame_byte_count(target, byte_count)?;
                if active
                    .replace(ActiveFrame {
                        byte_count,
                        next_write: 0,
                        next_address: 0,
                    })
                    .is_some()
                {
                    return Err(Diagnostic::error(
                        "compiler-private outgoing stack frames may not nest",
                    ));
                }
            }
            AssignedOperationKind::ReleaseOutgoingStackFrame { byte_count } => {
                validate_frame_byte_count(target, byte_count)?;
                let Some(frame) = active else {
                    return Err(Diagnostic::error(
                        "compiler-private outgoing stack-frame release has no exact matching reservation",
                    ));
                };
                if frame.byte_count != byte_count
                    || (frame.next_write != 0 && (frame.next_write != 4 || frame.next_address != 2))
                {
                    return Err(Diagnostic::error(
                        "compiler-private outgoing stack-frame release precedes its exact writes and address bindings",
                    ));
                }
                active = None;
            }
            AssignedOperationKind::WriteOutgoingStackU64 {
                stack_byte_offset, ..
            } => {
                let Some(frame) = active.as_mut() else {
                    return Err(Diagnostic::error(
                        "outgoing stack u64 write occurs outside a live reserved frame",
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
                        "outgoing stack u64 write violates exact 72-byte frame order or writable ranges",
                    ));
                }
                frame.next_write += 1;
            }
            AssignedOperationKind::LoadOutgoingStackAddress {
                register,
                stack_byte_offset,
            } => {
                let Some(frame) = active.as_mut() else {
                    return Err(Diagnostic::error(
                        "outgoing stack-address load occurs outside a live reserved frame",
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
                        "outgoing stack-address load precedes or drifts from exact caller-copy writes",
                    ));
                }
                frame.next_address += 1;
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

    fn exact_writes() -> Vec<AssignedOperation> {
        [32, 40, 48, 56]
            .into_iter()
            .enumerate()
            .map(|(index, stack_byte_offset)| {
                operation(AssignedOperationKind::WriteOutgoingStackU64 {
                    stack_byte_offset,
                    value: index as u64 + 1,
                })
            })
            .collect()
    }

    #[test]
    fn exact_balanced_frame_and_addresses_are_admitted() {
        let mut instructions = vec![operation(
            AssignedOperationKind::ReserveOutgoingStackFrame { byte_count: 72 },
        )];
        instructions.extend(exact_writes());
        instructions.extend([
            operation(AssignedOperationKind::LoadOutgoingStackAddress {
                register: MachineRegister::X86Rcx,
                stack_byte_offset: 32,
            }),
            operation(AssignedOperationKind::LoadOutgoingStackAddress {
                register: MachineRegister::X86Rdx,
                stack_byte_offset: 48,
            }),
            operation(AssignedOperationKind::ReleaseOutgoingStackFrame { byte_count: 72 }),
        ]);
        validate_outgoing_stack_frames(omega_target::NativeTarget::uefi_x64(), &instructions)
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

    #[test]
    fn shadow_padding_reorder_and_incomplete_write_sequences_reject() {
        let target = omega_target::NativeTarget::uefi_x64();
        for offset in [0, 24, 33, 40, 64] {
            let instructions = [
                operation(AssignedOperationKind::ReserveOutgoingStackFrame { byte_count: 72 }),
                operation(AssignedOperationKind::WriteOutgoingStackU64 {
                    stack_byte_offset: offset,
                    value: 1,
                }),
                operation(AssignedOperationKind::ReleaseOutgoingStackFrame { byte_count: 72 }),
            ];
            assert!(validate_outgoing_stack_frames(target, &instructions).is_err());
        }
        let mut incomplete = vec![operation(
            AssignedOperationKind::ReserveOutgoingStackFrame { byte_count: 72 },
        )];
        incomplete.extend(exact_writes().into_iter().take(3));
        incomplete.push(operation(
            AssignedOperationKind::ReleaseOutgoingStackFrame { byte_count: 72 },
        ));
        assert!(validate_outgoing_stack_frames(target, &incomplete).is_err());
    }
}
