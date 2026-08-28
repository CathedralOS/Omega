//! Derives imported-call, runtime-I/O, indirect-call, and syscall footprints.

use super::*;

pub(super) fn outbound_call_footprint_parts(
    architecture: Architecture,
    _runtime_value_operands: &psi_arena::Arena<omega_target_operations::RuntimeValueOperand>,
    kind: omega_machine_bytes::CompilerInstructionValidationKind,
) -> Option<CompilerInstructionFootprintParts> {
    use omega_calling_conventions::{MachineRegister, MachineState, MachineStateSet, RegisterSet};
    use omega_machine_bytes::CompilerInstructionValidationKind;
    use omega_machine_instructions::BoundaryFootprintFragmentOrigin;
    use omega_target_operations::InstructionOperandLike;

    let parts = match kind {
        CompilerInstructionValidationKind::CompilerBodyOutboundImmediateImport { plan, .. } => (
            BoundaryFootprintFragmentOrigin::CompilerBodyOutboundImmediateImport,
            RegisterSet::new(plan.ordinary_clobbers.as_slice().iter().copied().chain(
                match architecture {
                    Architecture::X86_64 => vec![MachineRegister::X86Rsp],
                    Architecture::Aarch64 => vec![MachineRegister::Aarch64X(16)],
                },
            )),
            MachineStateSet::new([
                MachineState::Flags,
                MachineState::InstructionPointer,
                MachineState::StackPointer,
                MachineState::ControlState,
            ]),
        ),
        CompilerInstructionValidationKind::CompilerBodyOutboundImmediateImportResult {
            operands,
            plan,
            ..
        } => {
            let (_, result_offset, result_byte_size) =
                operands.first()?.runtime_scalar_integer()?;
            let envelope_and_store_scratch = match architecture {
                Architecture::X86_64 => vec![MachineRegister::X86Rsp],
                Architecture::Aarch64 => Vec::from_iter(
                    [MachineRegister::Aarch64X(16)].into_iter().chain(
                        omega_isa_aarch64::constant_host_result_clobbers(
                            result_offset,
                            result_byte_size,
                        )
                        .as_slice()
                        .iter()
                        .copied(),
                    ),
                ),
            };
            (
                BoundaryFootprintFragmentOrigin::CompilerBodyOutboundImmediateImportResult,
                RegisterSet::new(
                    plan.ordinary_clobbers
                        .as_slice()
                        .iter()
                        .copied()
                        .chain(envelope_and_store_scratch),
                ),
                MachineStateSet::new([
                    MachineState::Flags,
                    MachineState::InstructionPointer,
                    MachineState::StackPointer,
                    MachineState::ControlState,
                ]),
            )
        }
        CompilerInstructionValidationKind::CompilerBodyOutboundFloatImportResult {
            operands,
            plan,
            ..
        } => {
            let (_, result_offset, result_byte_size) =
                operands.first()?.runtime_scalar_integer()?;
            let envelope_and_store_scratch = match architecture {
                Architecture::X86_64 => vec![MachineRegister::X86Rsp],
                Architecture::Aarch64 => Vec::from_iter(
                    [MachineRegister::Aarch64X(16)].into_iter().chain(
                        omega_isa_aarch64::constant_host_result_clobbers(
                            result_offset,
                            result_byte_size,
                        )
                        .as_slice()
                        .iter()
                        .copied(),
                    ),
                ),
            };
            (
                BoundaryFootprintFragmentOrigin::CompilerBodyOutboundFloatImportResult,
                RegisterSet::new(
                    plan.ordinary_clobbers
                        .as_slice()
                        .iter()
                        .copied()
                        .chain(envelope_and_store_scratch),
                ),
                MachineStateSet::new([
                    MachineState::Flags,
                    MachineState::InstructionPointer,
                    MachineState::StackPointer,
                    MachineState::ControlState,
                ]),
            )
        }
        CompilerInstructionValidationKind::CompilerBodyOutboundDereferencedImportResult {
            operands,
            plan,
            ..
        } => {
            let (_, result_offset, result_byte_size) =
                operands.first()?.runtime_scalar_integer()?;
            let envelope_and_store_scratch = match architecture {
                Architecture::X86_64 => vec![MachineRegister::X86Rsp],
                Architecture::Aarch64 => Vec::from_iter(
                    [MachineRegister::Aarch64X(16)].into_iter().chain(
                        omega_isa_aarch64::constant_host_result_clobbers(
                            result_offset,
                            result_byte_size,
                        )
                        .as_slice()
                        .iter()
                        .copied(),
                    ),
                ),
            };
            (
                BoundaryFootprintFragmentOrigin::CompilerBodyOutboundDereferencedImportResult,
                RegisterSet::new(
                    plan.ordinary_clobbers
                        .as_slice()
                        .iter()
                        .copied()
                        .chain(envelope_and_store_scratch),
                ),
                MachineStateSet::new([
                    MachineState::Flags,
                    MachineState::InstructionPointer,
                    MachineState::StackPointer,
                    MachineState::ControlState,
                ]),
            )
        }
        CompilerInstructionValidationKind::CompilerBodyOutboundDataImport { plan, .. } => (
            BoundaryFootprintFragmentOrigin::CompilerBodyOutboundDataImport,
            RegisterSet::new(plan.ordinary_clobbers.as_slice().iter().copied().chain(
                match architecture {
                    Architecture::X86_64 => vec![MachineRegister::X86Rsp],
                    Architecture::Aarch64 => vec![MachineRegister::Aarch64X(16)],
                },
            )),
            MachineStateSet::new([
                MachineState::Flags,
                MachineState::InstructionPointer,
                MachineState::StackPointer,
                MachineState::ControlState,
            ]),
        ),
        CompilerInstructionValidationKind::CompilerBodyOutboundIndirectCall {
            operands,
            identity,
            plan,
            ..
        } => {
            let dispatch_only = usize::from(match identity {
                omega_machine_bytes::CompilerIndirectCallValidationIdentity::Foreign {
                    mechanism,
                } => matches!(
                    mechanism,
                    omega_calling_conventions::HostBindingMechanism::TableFunction { .. }
                ),
                omega_machine_bytes::CompilerIndirectCallValidationIdentity::PrivateDynamic {
                    ..
                } => true,
            });
            let result_present = operands.len() == plan.parameters.len() + dispatch_only + 1;
            let envelope_and_store_scratch = match architecture {
                Architecture::X86_64 => vec![MachineRegister::X86Rsp],
                Architecture::Aarch64 => {
                    let mut registers = vec![MachineRegister::Aarch64X(16)];
                    if result_present
                        && let Some((_, result_offset, result_byte_size)) = operands
                            .first()
                            .and_then(InstructionOperandLike::runtime_scalar_integer)
                            .or_else(|| {
                                operands
                                    .first()
                                    .and_then(InstructionOperandLike::runtime_scalar_float)
                            })
                    {
                        registers.extend_from_slice(
                            omega_isa_aarch64::constant_host_result_clobbers(
                                result_offset,
                                result_byte_size,
                            )
                            .as_slice(),
                        );
                    }
                    registers
                }
            };
            (
                BoundaryFootprintFragmentOrigin::CompilerBodyOutboundIndirectCall,
                RegisterSet::new(
                    plan.ordinary_clobbers
                        .as_slice()
                        .iter()
                        .copied()
                        .chain(envelope_and_store_scratch),
                ),
                MachineStateSet::new([
                    MachineState::Flags,
                    MachineState::InstructionPointer,
                    MachineState::StackPointer,
                    MachineState::ControlState,
                ]),
            )
        }
        CompilerInstructionValidationKind::CompilerBodyOutboundDataImportResult {
            operands,
            plan,
            ..
        } => {
            let (_, result_offset, result_byte_size) =
                operands.first()?.runtime_scalar_integer()?;
            let envelope_and_store_scratch = match architecture {
                Architecture::X86_64 => vec![MachineRegister::X86Rsp],
                Architecture::Aarch64 => Vec::from_iter(
                    [MachineRegister::Aarch64X(16)].into_iter().chain(
                        omega_isa_aarch64::constant_host_result_clobbers(
                            result_offset,
                            result_byte_size,
                        )
                        .as_slice()
                        .iter()
                        .copied(),
                    ),
                ),
            };
            (
                BoundaryFootprintFragmentOrigin::CompilerBodyOutboundDataImportResult,
                RegisterSet::new(
                    plan.ordinary_clobbers
                        .as_slice()
                        .iter()
                        .copied()
                        .chain(envelope_and_store_scratch),
                ),
                MachineStateSet::new([
                    MachineState::Flags,
                    MachineState::InstructionPointer,
                    MachineState::StackPointer,
                    MachineState::ControlState,
                ]),
            )
        }
        CompilerInstructionValidationKind::CompilerBodyOutboundAuthoredImport { plan, .. } => (
            BoundaryFootprintFragmentOrigin::CompilerBodyOutboundAuthoredImport,
            RegisterSet::new(plan.ordinary_clobbers.as_slice().iter().copied().chain(
                match architecture {
                    Architecture::X86_64 => vec![MachineRegister::X86Rsp],
                    Architecture::Aarch64 => vec![MachineRegister::Aarch64X(16)],
                },
            )),
            MachineStateSet::new([
                MachineState::Flags,
                MachineState::InstructionPointer,
                MachineState::StackPointer,
                MachineState::ControlState,
            ]),
        ),
        CompilerInstructionValidationKind::CompilerBodyOutboundAuthoredImportResult {
            operands,
            plan,
            ..
        } => {
            let (_, result_offset, result_byte_size) =
                operands.first()?.runtime_scalar_integer()?;
            let envelope_and_store_scratch = match architecture {
                Architecture::X86_64 => vec![MachineRegister::X86Rsp],
                Architecture::Aarch64 => Vec::from_iter(
                    [MachineRegister::Aarch64X(16)].into_iter().chain(
                        omega_isa_aarch64::constant_host_result_clobbers(
                            result_offset,
                            result_byte_size,
                        )
                        .as_slice()
                        .iter()
                        .copied(),
                    ),
                ),
            };
            (
                BoundaryFootprintFragmentOrigin::CompilerBodyOutboundAuthoredImportResult,
                RegisterSet::new(
                    plan.ordinary_clobbers
                        .as_slice()
                        .iter()
                        .copied()
                        .chain(envelope_and_store_scratch),
                ),
                MachineStateSet::new([
                    MachineState::Flags,
                    MachineState::InstructionPointer,
                    MachineState::StackPointer,
                    MachineState::ControlState,
                ]),
            )
        }
        CompilerInstructionValidationKind::CompilerBodyOutboundAuthoredFloatImport {
            plan, ..
        } => (
            BoundaryFootprintFragmentOrigin::CompilerBodyOutboundAuthoredFloatImport,
            RegisterSet::new(plan.ordinary_clobbers.as_slice().iter().copied().chain(
                match architecture {
                    Architecture::X86_64 => vec![MachineRegister::X86Rsp],
                    Architecture::Aarch64 => vec![MachineRegister::Aarch64X(16)],
                },
            )),
            MachineStateSet::new([
                MachineState::Flags,
                MachineState::InstructionPointer,
                MachineState::StackPointer,
                MachineState::ControlState,
            ]),
        ),
        CompilerInstructionValidationKind::CompilerBodyOutboundAuthoredFloatImportResult {
            operands,
            plan,
            ..
        } => {
            let (_, result_offset, result_byte_size) =
                operands
                    .first()?
                    .runtime_scalar_integer()
                    .or_else(|| operands.first()?.runtime_scalar_float())?;
            let envelope_and_store_scratch = match architecture {
                Architecture::X86_64 => vec![MachineRegister::X86Rsp],
                Architecture::Aarch64 => Vec::from_iter(
                    [MachineRegister::Aarch64X(16)].into_iter().chain(
                        omega_isa_aarch64::constant_host_result_clobbers(
                            result_offset,
                            result_byte_size,
                        )
                        .as_slice()
                        .iter()
                        .copied(),
                    ),
                ),
            };
            (
                BoundaryFootprintFragmentOrigin::CompilerBodyOutboundAuthoredFloatImportResult,
                RegisterSet::new(
                    plan.ordinary_clobbers
                        .as_slice()
                        .iter()
                        .copied()
                        .chain(envelope_and_store_scratch),
                ),
                MachineStateSet::new([
                    MachineState::Flags,
                    MachineState::InstructionPointer,
                    MachineState::StackPointer,
                    MachineState::ControlState,
                ]),
            )
        }
        CompilerInstructionValidationKind::CompilerBodyOutboundAuthoredAggregateImport {
            plan,
            ..
        } => (
            BoundaryFootprintFragmentOrigin::CompilerBodyOutboundAuthoredAggregateImport,
            RegisterSet::new(plan.ordinary_clobbers.as_slice().iter().copied().chain(
                match architecture {
                    Architecture::X86_64 => vec![MachineRegister::X86Rsp],
                    Architecture::Aarch64 => vec![MachineRegister::Aarch64X(16)],
                },
            )),
            MachineStateSet::new([
                MachineState::Flags,
                MachineState::InstructionPointer,
                MachineState::StackPointer,
                MachineState::ControlState,
            ]),
        ),
        CompilerInstructionValidationKind::CompilerBodyOutboundAuthoredAggregateImportResult {
            operands,
            plan,
            ..
        } => {
            let (_, result_offset, result_byte_size) =
                operands
                    .first()?
                    .runtime_scalar_integer()
                    .or_else(|| operands.first()?.runtime_scalar_float())?;
            let envelope_and_store_scratch = match architecture {
                Architecture::X86_64 => vec![MachineRegister::X86Rsp],
                Architecture::Aarch64 => Vec::from_iter(
                    [MachineRegister::Aarch64X(16)].into_iter().chain(
                        omega_isa_aarch64::constant_host_result_clobbers(
                            result_offset,
                            result_byte_size,
                        )
                        .as_slice()
                        .iter()
                        .copied(),
                    ),
                ),
            };
            (
                BoundaryFootprintFragmentOrigin::CompilerBodyOutboundAuthoredAggregateImportResult,
                RegisterSet::new(
                    plan.ordinary_clobbers
                        .as_slice()
                        .iter()
                        .copied()
                        .chain(envelope_and_store_scratch),
                ),
                MachineStateSet::new([
                    MachineState::Flags,
                    MachineState::InstructionPointer,
                    MachineState::StackPointer,
                    MachineState::ControlState,
                ]),
            )
        }
        CompilerInstructionValidationKind::CompilerBodyOutboundAuthoredAggregateResult {
            plan,
            ..
        } => (
            BoundaryFootprintFragmentOrigin::CompilerBodyOutboundAuthoredAggregateResult,
            RegisterSet::new(plan.ordinary_clobbers.as_slice().iter().copied().chain(
                match architecture {
                    Architecture::X86_64 => vec![MachineRegister::X86Rsp],
                    Architecture::Aarch64 => vec![MachineRegister::Aarch64X(16)],
                },
            )),
            MachineStateSet::new([
                MachineState::Flags,
                MachineState::InstructionPointer,
                MachineState::StackPointer,
                MachineState::ControlState,
            ]),
        ),
        CompilerInstructionValidationKind::CompilerBodyOutboundOpenCreateImport {
            operands,
            plan,
            ..
        } => {
            let (_, result_offset, result_byte_size) =
                operands.first()?.runtime_scalar_integer()?;
            (
                BoundaryFootprintFragmentOrigin::CompilerBodyOutboundOpenCreateImport,
                RegisterSet::new(
                    plan.ordinary_clobbers
                        .as_slice()
                        .iter()
                        .copied()
                        .chain([MachineRegister::Aarch64X(16)])
                        .chain(
                            omega_isa_aarch64::constant_host_result_clobbers(
                                result_offset,
                                result_byte_size,
                            )
                            .as_slice()
                            .iter()
                            .copied(),
                        ),
                ),
                MachineStateSet::new([
                    MachineState::Flags,
                    MachineState::InstructionPointer,
                    MachineState::StackPointer,
                    MachineState::ControlState,
                ]),
            )
        }
        CompilerInstructionValidationKind::CompilerBodyRuntimeByteRead {
            mechanism,
            plan,
            get_std_handle,
            ..
        } => {
            let mut registers = plan.ordinary_clobbers.as_slice().to_vec();
            match architecture {
                Architecture::X86_64 => {
                    registers.push(MachineRegister::X86R14);
                    if matches!(
                        mechanism,
                        omega_calling_conventions::HostBindingMechanism::Import { .. }
                    ) {
                        registers.push(MachineRegister::X86Rsp);
                        if let Some(handle) = get_std_handle {
                            registers.extend_from_slice(handle.plan.ordinary_clobbers.as_slice());
                        }
                    }
                }
                Architecture::Aarch64 => {
                    registers.extend([MachineRegister::Aarch64X(20), MachineRegister::Aarch64X(9)]);
                }
            }
            let mut states = vec![
                MachineState::Flags,
                MachineState::InstructionPointer,
                MachineState::ControlState,
            ];
            if matches!(
                mechanism,
                omega_calling_conventions::HostBindingMechanism::Import { .. }
            ) {
                states.push(MachineState::StackPointer);
            }
            (
                BoundaryFootprintFragmentOrigin::CompilerBodyRuntimeByteRead,
                RegisterSet::new(registers),
                MachineStateSet::new(states),
            )
        }
        CompilerInstructionValidationKind::CompilerBodyRuntimeByteWrite {
            source_offset,
            mechanism,
            plan,
            get_std_handle,
            ..
        } => {
            let mut registers = plan.ordinary_clobbers.as_slice().to_vec();
            match architecture {
                Architecture::X86_64 => {
                    registers.push(MachineRegister::X86R14);
                    if matches!(
                        mechanism,
                        omega_calling_conventions::HostBindingMechanism::Import { .. }
                    ) {
                        registers.push(MachineRegister::X86Rsp);
                        if let Some(handle) = get_std_handle {
                            registers.extend_from_slice(handle.plan.ordinary_clobbers.as_slice());
                        }
                    }
                }
                Architecture::Aarch64 => {
                    registers.push(MachineRegister::Aarch64X(20));
                    if source_offset > 4095 {
                        registers.push(MachineRegister::Aarch64X(9));
                    }
                }
            }
            let mut states = vec![
                MachineState::Flags,
                MachineState::InstructionPointer,
                MachineState::ControlState,
            ];
            if matches!(
                mechanism,
                omega_calling_conventions::HostBindingMechanism::Import { .. }
            ) {
                states.push(MachineState::StackPointer);
            }
            (
                BoundaryFootprintFragmentOrigin::CompilerBodyRuntimeByteWrite,
                RegisterSet::new(registers),
                MachineStateSet::new(states),
            )
        }
        CompilerInstructionValidationKind::CompilerBodyRuntimeLineRead {
            target_offset,
            target,
            mechanism,
            plan,
            get_std_handle,
            ..
        } => {
            use omega_target_operations::RuntimeTextReadTarget;

            let mut registers = plan.ordinary_clobbers.as_slice().to_vec();
            match architecture {
                Architecture::X86_64 => {
                    registers.extend([MachineRegister::X86R14, MachineRegister::X86R15]);
                    let is_import = matches!(
                        mechanism,
                        omega_calling_conventions::HostBindingMechanism::Import { .. }
                    );
                    if is_import || target == RuntimeTextReadTarget::StringDescriptor {
                        registers.push(MachineRegister::X86R13);
                    }
                    if is_import {
                        registers.push(MachineRegister::X86Rsp);
                        if let Some(handle) = get_std_handle {
                            registers.extend_from_slice(handle.plan.ordinary_clobbers.as_slice());
                        }
                    }
                }
                Architecture::Aarch64 => {
                    registers.extend([
                        MachineRegister::Aarch64X(20),
                        MachineRegister::Aarch64X(21),
                        MachineRegister::Aarch64X(22),
                        MachineRegister::Aarch64X(24),
                    ]);
                    match target {
                        RuntimeTextReadTarget::StringDescriptor => {
                            registers.push(MachineRegister::Aarch64X(16));
                            let direct_descriptor_stores = (target_offset + 8).is_multiple_of(8)
                                && (target_offset + 8) / 8 <= 4095;
                            if !direct_descriptor_stores && target_offset > 4095 {
                                registers.push(MachineRegister::Aarch64X(9));
                            }
                        }
                        RuntimeTextReadTarget::BoundedByteBuffer => {
                            if target_offset + 8 > 4095 {
                                registers.push(MachineRegister::Aarch64X(19));
                            }
                        }
                        RuntimeTextReadTarget::FixedByteArray => {
                            if target_offset > 4095 {
                                registers.push(MachineRegister::Aarch64X(19));
                            }
                        }
                    }
                }
            }
            let mut states = vec![
                MachineState::Flags,
                MachineState::InstructionPointer,
                MachineState::ControlState,
            ];
            if matches!(
                mechanism,
                omega_calling_conventions::HostBindingMechanism::Import { .. }
            ) {
                states.push(MachineState::StackPointer);
            }
            (
                BoundaryFootprintFragmentOrigin::CompilerBodyRuntimeLineRead,
                RegisterSet::new(registers),
                MachineStateSet::new(states),
            )
        }
        CompilerInstructionValidationKind::CompilerBodyOutboundStorageImport { plan, .. } => (
            BoundaryFootprintFragmentOrigin::CompilerBodyOutboundStorageImport,
            RegisterSet::new(plan.ordinary_clobbers.as_slice().iter().copied().chain(
                match architecture {
                    Architecture::X86_64 => vec![MachineRegister::X86Rsp],
                    Architecture::Aarch64 => vec![MachineRegister::Aarch64X(16)],
                },
            )),
            MachineStateSet::new([
                MachineState::Flags,
                MachineState::InstructionPointer,
                MachineState::StackPointer,
                MachineState::ControlState,
            ]),
        ),
        CompilerInstructionValidationKind::CompilerBodyOutboundStorageImportResult {
            operands,
            plan,
            ..
        } => {
            let (_, result_offset, result_byte_size) =
                operands.first()?.runtime_scalar_integer()?;
            let envelope_and_store_scratch = match architecture {
                Architecture::X86_64 => vec![MachineRegister::X86Rsp],
                Architecture::Aarch64 => Vec::from_iter(
                    [MachineRegister::Aarch64X(16)].into_iter().chain(
                        omega_isa_aarch64::constant_host_result_clobbers(
                            result_offset,
                            result_byte_size,
                        )
                        .as_slice()
                        .iter()
                        .copied(),
                    ),
                ),
            };
            (
                BoundaryFootprintFragmentOrigin::CompilerBodyOutboundStorageImportResult,
                RegisterSet::new(
                    plan.ordinary_clobbers
                        .as_slice()
                        .iter()
                        .copied()
                        .chain(envelope_and_store_scratch),
                ),
                MachineStateSet::new([
                    MachineState::Flags,
                    MachineState::InstructionPointer,
                    MachineState::StackPointer,
                    MachineState::ControlState,
                ]),
            )
        }
        CompilerInstructionValidationKind::CompilerBodyOutboundSyscall { plan, .. } => (
            BoundaryFootprintFragmentOrigin::CompilerBodyOutboundSyscall,
            plan.ordinary_clobbers,
            MachineStateSet::new([
                MachineState::Flags,
                MachineState::InstructionPointer,
                MachineState::ControlState,
            ]),
        ),
        CompilerInstructionValidationKind::CompilerBodyOutboundSyscallDataArguments {
            plan,
            ..
        } => (
            BoundaryFootprintFragmentOrigin::CompilerBodyOutboundSyscallDataArguments,
            plan.ordinary_clobbers,
            MachineStateSet::new([
                MachineState::Flags,
                MachineState::InstructionPointer,
                MachineState::ControlState,
            ]),
        ),
        CompilerInstructionValidationKind::CompilerBodyOutboundSyscallStorageArguments {
            plan,
            ..
        } => (
            BoundaryFootprintFragmentOrigin::CompilerBodyOutboundSyscallStorageArguments,
            plan.ordinary_clobbers,
            MachineStateSet::new([
                MachineState::Flags,
                MachineState::InstructionPointer,
                MachineState::ControlState,
            ]),
        ),
        CompilerInstructionValidationKind::CompilerBodyOutboundSyscallResult {
            operands,
            plan,
            ..
        } => {
            let (_, result_offset, result_byte_size) = operands
                .first()
                .and_then(InstructionOperandLike::runtime_scalar_integer)?;
            let result_store = match architecture {
                Architecture::X86_64 => RegisterSet::default(),
                Architecture::Aarch64 => omega_isa_aarch64::constant_host_result_clobbers(
                    result_offset,
                    result_byte_size,
                ),
            };
            (
                BoundaryFootprintFragmentOrigin::CompilerBodyOutboundSyscallResult,
                RegisterSet::new(
                    plan.ordinary_clobbers
                        .as_slice()
                        .iter()
                        .copied()
                        .chain(result_store.as_slice().iter().copied()),
                ),
                MachineStateSet::new([
                    MachineState::Flags,
                    MachineState::InstructionPointer,
                    MachineState::ControlState,
                ]),
            )
        }
        CompilerInstructionValidationKind::CompilerBodyOutboundSyscallResultStorageArguments {
            operands,
            plan,
            ..
        } => {
            let (_, result_offset, result_byte_size) = operands
                .first()
                .and_then(InstructionOperandLike::runtime_scalar_integer)?;
            let result_store = match architecture {
                Architecture::X86_64 => RegisterSet::default(),
                Architecture::Aarch64 => omega_isa_aarch64::constant_host_result_clobbers(
                    result_offset,
                    result_byte_size,
                ),
            };
            (
                BoundaryFootprintFragmentOrigin::CompilerBodyOutboundSyscallResultStorageArguments,
                RegisterSet::new(
                    plan.ordinary_clobbers
                        .as_slice()
                        .iter()
                        .copied()
                        .chain(result_store.as_slice().iter().copied()),
                ),
                MachineStateSet::new([
                    MachineState::Flags,
                    MachineState::InstructionPointer,
                    MachineState::ControlState,
                ]),
            )
        }
        CompilerInstructionValidationKind::CompilerBodyOutboundSyscallResultDataArguments {
            operands,
            plan,
            ..
        } => {
            let (_, result_offset, result_byte_size) = operands
                .first()
                .and_then(InstructionOperandLike::runtime_scalar_integer)?;
            let result_store = match architecture {
                Architecture::X86_64 => RegisterSet::default(),
                Architecture::Aarch64 => omega_isa_aarch64::constant_host_result_clobbers(
                    result_offset,
                    result_byte_size,
                ),
            };
            (
                BoundaryFootprintFragmentOrigin::CompilerBodyOutboundSyscallResultDataArguments,
                RegisterSet::new(
                    plan.ordinary_clobbers
                        .as_slice()
                        .iter()
                        .copied()
                        .chain(result_store.as_slice().iter().copied()),
                ),
                MachineStateSet::new([
                    MachineState::Flags,
                    MachineState::InstructionPointer,
                    MachineState::ControlState,
                ]),
            )
        }
        CompilerInstructionValidationKind::CompilerBodyOutboundSyscallTimespecArgument {
            plan,
            ..
        } => (
            BoundaryFootprintFragmentOrigin::CompilerBodyOutboundSyscallTimespecArgument,
            RegisterSet::new(
                plan.ordinary_clobbers.as_slice().iter().copied().chain(
                    (architecture == Architecture::X86_64)
                        .then_some([MachineRegister::X86Rdx, MachineRegister::X86Rsp])
                        .into_iter()
                        .flatten(),
                ),
            ),
            MachineStateSet::new([
                MachineState::Flags,
                MachineState::InstructionPointer,
                MachineState::StackPointer,
                MachineState::ControlState,
            ]),
        ),
        CompilerInstructionValidationKind::CompilerBodyOutboundSyscallTimespecResult {
            operands,
            plan,
            ..
        } => {
            let (_, result_offset, result_byte_size) = operands
                .first()
                .and_then(InstructionOperandLike::runtime_scalar_integer)?;
            let adapter_scratch = match architecture {
                Architecture::X86_64 => RegisterSet::new([MachineRegister::X86Rsp]),
                Architecture::Aarch64 => omega_isa_aarch64::constant_host_result_clobbers(
                    result_offset,
                    result_byte_size,
                ),
            };
            (
                BoundaryFootprintFragmentOrigin::CompilerBodyOutboundSyscallTimespecResult,
                RegisterSet::new(
                    plan.ordinary_clobbers
                        .as_slice()
                        .iter()
                        .copied()
                        .chain(adapter_scratch.as_slice().iter().copied()),
                ),
                MachineStateSet::new([
                    MachineState::Flags,
                    MachineState::InstructionPointer,
                    MachineState::StackPointer,
                    MachineState::ControlState,
                ]),
            )
        }
        _ => return None,
    };
    Some(parts)
}
