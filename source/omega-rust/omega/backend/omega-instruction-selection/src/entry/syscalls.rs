//! Outbound syscall footprint derivation.

use omega_calling_conventions::{
    MachineState, MachineStateSet, PlanDiagnostic, RegisterSet, StateFootprintEvidence,
    ValidatedBoundaryEntryPlan,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outbound_syscall_storage_arguments_close_over_runtime_address_shapes() {
        use omega_abstract_operations::{
            InstructionOperand, InstructionOperandKind, RuntimeStorageRegion,
        };

        let runtime_address = InstructionOperand {
            kind: InstructionOperandKind::RuntimeStorageAddress {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 64,
            },
        };
        let descriptor_length = InstructionOperand {
            kind: InstructionOperandKind::RuntimePointeeStringLength {
                region: RuntimeStorageRegion::Machine,
                byte_offset: 32,
            },
        };
        let bounded_small_offset = InstructionOperand {
            kind: InstructionOperandKind::RuntimeStringPointer {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 4087,
                is_bounded_buffer: true,
            },
        };
        let bounded_large_offset = InstructionOperand {
            kind: InstructionOperandKind::RuntimeStringPointer {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 4088,
                is_bounded_buffer: true,
            },
        };
        let data_address = InstructionOperand {
            kind: InstructionOperandKind::DataAddress {
                data: psi_arena::Handle::invalid(),
            },
        };

        for operand in [&runtime_address, &descriptor_length] {
            assert!(abstract_outbound_syscall_storage_argument_is_closed(
                omega_target::Architecture::X86_64,
                operand,
            ));
            assert!(abstract_outbound_syscall_storage_argument_is_closed(
                omega_target::Architecture::Aarch64,
                operand,
            ));
        }
        assert!(abstract_outbound_syscall_storage_argument_is_closed(
            omega_target::Architecture::Aarch64,
            &bounded_small_offset,
        ));
        assert!(abstract_outbound_syscall_storage_argument_is_closed(
            omega_target::Architecture::Aarch64,
            &bounded_large_offset,
        ));
        assert!(abstract_outbound_syscall_storage_argument_is_closed(
            omega_target::Architecture::X86_64,
            &bounded_large_offset,
        ));
        assert!(abstract_outbound_syscall_data_argument_is_closed(
            &data_address,
        ));
        assert!(!abstract_outbound_syscall_storage_argument_is_closed(
            omega_target::Architecture::X86_64,
            &data_address,
        ));
    }
}

/// Derive the semantic leaf footprint of simple outbound syscalls. The target
/// encoder is constrained by the same retained `CallPlan`; the supervisor may
/// realize any ordinary clobber admitted by that plan.
pub fn derive_boundary_compiler_body_outbound_syscall_footprint(
    boundary: &ValidatedBoundaryEntryPlan,
    input: &crate::InstructionSelectionInput<'_>,
    operands: &psi_arena::Arena<omega_abstract_operations::InstructionOperand>,
    instructions: &[omega_abstract_operations::AbstractOperation],
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    use omega_abstract_operations::{AbstractOperationKind, InstructionOperandKind};
    use omega_calling_conventions::{EntryControl, HostBindingMechanism};

    let mut registers = Vec::new();
    let mut has_syscall = false;
    for instruction in instructions {
        let AbstractOperationKind::HostOperation {
            operation_ordinal,
            operands: operand_span,
            ..
        } = &instruction.kind
        else {
            continue;
        };
        let Some((_, host_call)) = input.host_calls.calls.iter().find(|(_, host_call)| {
            host_call.source_key == instruction.source_key
                && host_call.statement_index == instruction.source_statement
        }) else {
            continue;
        };
        let Some(operation) = input
            .host_calls
            .operations
            .span(host_call.operations)
            .and_then(|operations| operations.get(usize::from(*operation_ordinal)))
        else {
            continue;
        };
        let Some((_, binding)) = input
            .host_abi
            .bindings
            .iter()
            .find(|(_, binding)| binding.operation_key == operation.operation_key)
        else {
            continue;
        };
        if !matches!(binding.mechanism, HostBindingMechanism::Syscall { .. })
            || operation.operation_key.uses_linux_timespec_result()
            || operation.operation_key.uses_linux_timespec_argument()
            || (binding.call_plan().result.is_some()
                && !operation.operation_key.discards_native_result())
            || !matches!(
                binding.call_plan().entry_control,
                EntryControl::SupervisorCall { .. }
            )
            || !operands.span(*operand_span).is_some_and(|operands| {
                !operands.is_empty()
                    && operands.iter().all(|operand| {
                        matches!(
                            operand.kind,
                            InstructionOperandKind::ImmediateInteger(_)
                                | InstructionOperandKind::ByteLength(_)
                        )
                    })
            })
        {
            continue;
        }
        has_syscall = true;
        registers.extend_from_slice(binding.call_plan().ordinary_clobbers.as_slice());
    }
    let evidence = StateFootprintEvidence::new(
        RegisterSet::new(registers),
        if has_syscall {
            MachineStateSet::new([
                MachineState::Flags,
                MachineState::InstructionPointer,
                MachineState::ControlState,
            ])
        } else {
            MachineStateSet::empty()
        },
    );
    omega_calling_conventions::validate_outbound_call_footprint(boundary, &evidence)?;
    Ok(evidence)
}

fn abstract_outbound_syscall_storage_argument_is_closed(
    _architecture: omega_target::Architecture,
    operand: &omega_abstract_operations::InstructionOperand,
) -> bool {
    use omega_abstract_operations::InstructionOperandKind;

    match operand.kind {
        InstructionOperandKind::RuntimeStringPointer { .. }
        | InstructionOperandKind::RuntimeStringLength { .. }
        | InstructionOperandKind::RuntimePointeeStringPointer { .. }
        | InstructionOperandKind::RuntimePointeeStringLength { .. }
        | InstructionOperandKind::RuntimeScalarInteger { .. }
        | InstructionOperandKind::RuntimeStorageAddress { .. } => true,
        _ => false,
    }
}

fn abstract_outbound_syscall_data_argument_is_closed(
    operand: &omega_abstract_operations::InstructionOperand,
) -> bool {
    matches!(
        operand.kind,
        omega_abstract_operations::InstructionOperandKind::DataAddress { .. }
    )
}

/// Derive no-result outbound syscall leaves that marshal one or more values,
/// descriptor fields, or addresses from runtime storage. Their marshallers use
/// only the normalized syscall plan's ordinary-clobber set; exact storage
/// relocations are retained later beside the encoded instruction.
pub fn derive_boundary_compiler_body_outbound_syscall_storage_arguments_footprint(
    boundary: &ValidatedBoundaryEntryPlan,
    input: &crate::InstructionSelectionInput<'_>,
    operands: &psi_arena::Arena<omega_abstract_operations::InstructionOperand>,
    instructions: &[omega_abstract_operations::AbstractOperation],
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    derive_boundary_compiler_body_outbound_syscall_relocatable_arguments_footprint(
        boundary,
        input,
        operands,
        instructions,
        false,
    )
}

/// Derive no-result outbound syscall leaves with at least one exact static
/// data-object address. Other parameters may be immediate or use the already
/// closed runtime-storage forms; the final validator retains both relocation
/// target classes independently.
pub fn derive_boundary_compiler_body_outbound_syscall_data_arguments_footprint(
    boundary: &ValidatedBoundaryEntryPlan,
    input: &crate::InstructionSelectionInput<'_>,
    operands: &psi_arena::Arena<omega_abstract_operations::InstructionOperand>,
    instructions: &[omega_abstract_operations::AbstractOperation],
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    derive_boundary_compiler_body_outbound_syscall_relocatable_arguments_footprint(
        boundary,
        input,
        operands,
        instructions,
        true,
    )
}

fn derive_boundary_compiler_body_outbound_syscall_relocatable_arguments_footprint(
    boundary: &ValidatedBoundaryEntryPlan,
    input: &crate::InstructionSelectionInput<'_>,
    operands: &psi_arena::Arena<omega_abstract_operations::InstructionOperand>,
    instructions: &[omega_abstract_operations::AbstractOperation],
    requires_data_argument: bool,
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    use omega_abstract_operations::{AbstractOperationKind, InstructionOperandKind};
    use omega_calling_conventions::{
        EntryControl, HostBindingMechanism, HostOperation, HostOperationKey,
    };

    let mut registers = Vec::new();
    let mut has_syscall = false;
    for instruction in instructions {
        let Some((operation_key, operand_span)) = (match &instruction.kind {
            AbstractOperationKind::HostOperation {
                operation_ordinal,
                operands,
                ..
            } => input
                .host_calls
                .calls
                .iter()
                .find(|(_, host_call)| {
                    host_call.source_key == instruction.source_key
                        && host_call.statement_index == instruction.source_statement
                })
                .and_then(|(_, host_call)| {
                    input
                        .host_calls
                        .operations
                        .span(host_call.operations)
                        .and_then(|operations| operations.get(usize::from(*operation_ordinal)))
                })
                .map(|operation| (operation.operation_key, *operands)),
            AbstractOperationKind::WritePlatformNewline {
                capability,
                use_file_api,
                operands,
            } => Some((
                HostOperationKey::new(
                    *capability,
                    if *use_file_api {
                        HostOperation::WriteFile
                    } else {
                        HostOperation::Write
                    },
                ),
                *operands,
            )),
            _ => None,
        }) else {
            continue;
        };
        let Some((_, binding)) = input
            .host_abi
            .bindings
            .iter()
            .find(|(_, binding)| binding.operation_key == operation_key)
        else {
            continue;
        };
        let Some(arguments) = operands.span(operand_span) else {
            continue;
        };
        let has_storage = arguments.iter().any(|operand| {
            abstract_outbound_syscall_storage_argument_is_closed(input.target.architecture, operand)
        });
        let has_data = arguments
            .iter()
            .any(abstract_outbound_syscall_data_argument_is_closed);
        if !matches!(binding.mechanism, HostBindingMechanism::Syscall { .. })
            || operation_key.uses_linux_timespec_result()
            || operation_key.uses_linux_timespec_argument()
            || (binding.call_plan().result.is_some() && !operation_key.discards_native_result())
            || binding.call_plan().parameters.len() != arguments.len()
            || !matches!(
                binding.call_plan().entry_control,
                EntryControl::SupervisorCall { .. }
            )
            || if requires_data_argument {
                !has_data
            } else {
                !has_storage || has_data
            }
            || !arguments.iter().all(|operand| {
                matches!(
                    operand.kind,
                    InstructionOperandKind::ImmediateInteger(_)
                        | InstructionOperandKind::ByteLength(_)
                ) || abstract_outbound_syscall_storage_argument_is_closed(
                    input.target.architecture,
                    operand,
                ) || abstract_outbound_syscall_data_argument_is_closed(operand)
            })
        {
            continue;
        }
        has_syscall = true;
        registers.extend_from_slice(binding.call_plan().ordinary_clobbers.as_slice());
    }
    let evidence = StateFootprintEvidence::new(
        RegisterSet::new(registers),
        if has_syscall {
            MachineStateSet::new([
                MachineState::Flags,
                MachineState::InstructionPointer,
                MachineState::ControlState,
            ])
        } else {
            MachineStateSet::empty()
        },
    );
    omega_calling_conventions::validate_outbound_call_footprint(boundary, &evidence)?;
    Ok(evidence)
}

/// Derive the first result-bearing outbound syscall leaf. This deliberately
/// covers only a runtime-scalar destination followed by immediate/byte-length
/// parameters; relocatable parameters and composite adapters retain separate
/// footprint classes. AArch64's post-call store owns x16 and, for a large or
/// unscaled destination offset, x17 in addition to the syscall plan ceiling.
pub fn derive_boundary_compiler_body_outbound_syscall_result_footprint(
    boundary: &ValidatedBoundaryEntryPlan,
    input: &crate::InstructionSelectionInput<'_>,
    operands: &psi_arena::Arena<omega_abstract_operations::InstructionOperand>,
    instructions: &[omega_abstract_operations::AbstractOperation],
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    derive_boundary_compiler_body_outbound_syscall_result_footprint_for_arguments(
        boundary,
        input,
        operands,
        instructions,
        OutboundSyscallResultArgumentClass::Immediate,
    )
}

/// Derive result-bearing outbound syscalls whose ordinary parameters include
/// one or more of the closed runtime-storage forms. The plan still owns the
/// syscall marshaller; AArch64's post-call destination materializer contributes
/// its offset-sensitive x16/x17 scratch separately.
pub fn derive_boundary_compiler_body_outbound_syscall_result_storage_arguments_footprint(
    boundary: &ValidatedBoundaryEntryPlan,
    input: &crate::InstructionSelectionInput<'_>,
    operands: &psi_arena::Arena<omega_abstract_operations::InstructionOperand>,
    instructions: &[omega_abstract_operations::AbstractOperation],
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    derive_boundary_compiler_body_outbound_syscall_result_footprint_for_arguments(
        boundary,
        input,
        operands,
        instructions,
        OutboundSyscallResultArgumentClass::Storage,
    )
}

/// Derive result-bearing outbound syscall leaves with at least one exact
/// static data-object address and any otherwise-closed runtime-storage or
/// immediate parameters.
pub fn derive_boundary_compiler_body_outbound_syscall_result_data_arguments_footprint(
    boundary: &ValidatedBoundaryEntryPlan,
    input: &crate::InstructionSelectionInput<'_>,
    operands: &psi_arena::Arena<omega_abstract_operations::InstructionOperand>,
    instructions: &[omega_abstract_operations::AbstractOperation],
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    derive_boundary_compiler_body_outbound_syscall_result_footprint_for_arguments(
        boundary,
        input,
        operands,
        instructions,
        OutboundSyscallResultArgumentClass::Data,
    )
}

#[derive(Clone, Copy)]
enum OutboundSyscallResultArgumentClass {
    Immediate,
    Storage,
    Data,
}

#[derive(Clone, Copy)]
enum OutboundSyscallTimespecClass {
    Argument,
    Result,
}

/// Derive the Linux nanosleep adapter leaf. The concrete two-pointer syscall
/// plan owns the supervisor boundary while the compiler-owned request builder
/// additionally mutates balanced stack state and target-specific arithmetic
/// scratch.
pub fn derive_boundary_compiler_body_outbound_syscall_timespec_argument_footprint(
    boundary: &ValidatedBoundaryEntryPlan,
    input: &crate::InstructionSelectionInput<'_>,
    operands: &psi_arena::Arena<omega_abstract_operations::InstructionOperand>,
    instructions: &[omega_abstract_operations::AbstractOperation],
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    derive_boundary_compiler_body_outbound_syscall_timespec_footprint(
        boundary,
        input,
        operands,
        instructions,
        OutboundSyscallTimespecClass::Argument,
    )
}

/// Derive the Linux clock_gettime adapter leaf. Its private two-word result is
/// reduced to nanoseconds and stored into the semantic scalar destination.
pub fn derive_boundary_compiler_body_outbound_syscall_timespec_result_footprint(
    boundary: &ValidatedBoundaryEntryPlan,
    input: &crate::InstructionSelectionInput<'_>,
    operands: &psi_arena::Arena<omega_abstract_operations::InstructionOperand>,
    instructions: &[omega_abstract_operations::AbstractOperation],
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    derive_boundary_compiler_body_outbound_syscall_timespec_footprint(
        boundary,
        input,
        operands,
        instructions,
        OutboundSyscallTimespecClass::Result,
    )
}

fn derive_boundary_compiler_body_outbound_syscall_timespec_footprint(
    boundary: &ValidatedBoundaryEntryPlan,
    input: &crate::InstructionSelectionInput<'_>,
    operands: &psi_arena::Arena<omega_abstract_operations::InstructionOperand>,
    instructions: &[omega_abstract_operations::AbstractOperation],
    class: OutboundSyscallTimespecClass,
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    use omega_abstract_operations::{AbstractOperationKind, InstructionOperandKind};
    use omega_calling_conventions::{EntryControl, HostBindingMechanism, MachineRegister};

    let mut registers = Vec::new();
    let mut has_syscall = false;
    for instruction in instructions {
        let AbstractOperationKind::HostOperation {
            operation_ordinal,
            operands: operand_span,
            ..
        } = &instruction.kind
        else {
            continue;
        };
        let Some((_, host_call)) = input.host_calls.calls.iter().find(|(_, host_call)| {
            host_call.source_key == instruction.source_key
                && host_call.statement_index == instruction.source_statement
        }) else {
            continue;
        };
        let Some(operation) = input
            .host_calls
            .operations
            .span(host_call.operations)
            .and_then(|operations| operations.get(usize::from(*operation_ordinal)))
        else {
            continue;
        };
        let Some((_, binding)) = input
            .host_abi
            .bindings
            .iter()
            .find(|(_, binding)| binding.operation_key == operation.operation_key)
        else {
            continue;
        };
        let Some(call_operands) = operands.span(*operand_span) else {
            continue;
        };
        let shape_matches = match (class, call_operands) {
            (
                OutboundSyscallTimespecClass::Result,
                [
                    omega_abstract_operations::InstructionOperand {
                        kind: InstructionOperandKind::RuntimeScalarInteger { byte_count: 8, .. },
                    },
                    omega_abstract_operations::InstructionOperand {
                        kind: InstructionOperandKind::ImmediateInteger(_),
                    },
                ],
            ) => true,
            (
                OutboundSyscallTimespecClass::Argument,
                [
                    omega_abstract_operations::InstructionOperand {
                        kind:
                            InstructionOperandKind::RuntimeScalarInteger {
                                byte_count: 4 | 8, ..
                            }
                            | InstructionOperandKind::ImmediateInteger(0..),
                    },
                ],
            ) => true,
            _ => false,
        };
        let operation_matches = match class {
            OutboundSyscallTimespecClass::Argument => {
                operation.operation_key.uses_linux_timespec_argument()
            }
            OutboundSyscallTimespecClass::Result => {
                operation.operation_key.uses_linux_timespec_result()
            }
        };
        if !operation_matches
            || !shape_matches
            || !matches!(binding.mechanism, HostBindingMechanism::Syscall { .. })
            || binding.call_plan().parameters.len() != 2
            || binding.call_plan().result.is_none()
            || !matches!(
                binding.call_plan().entry_control,
                EntryControl::SupervisorCall { .. }
            )
        {
            continue;
        }
        has_syscall = true;
        registers.extend_from_slice(binding.call_plan().ordinary_clobbers.as_slice());
        match (input.target.architecture, class, call_operands) {
            (omega_target::Architecture::X86_64, OutboundSyscallTimespecClass::Result, _) => {
                registers.push(MachineRegister::X86Rsp)
            }
            (omega_target::Architecture::X86_64, OutboundSyscallTimespecClass::Argument, _) => {
                registers.extend([MachineRegister::X86Rdx, MachineRegister::X86Rsp])
            }
            (
                omega_target::Architecture::Aarch64,
                OutboundSyscallTimespecClass::Result,
                [
                    omega_abstract_operations::InstructionOperand {
                        kind:
                            InstructionOperandKind::RuntimeScalarInteger {
                                byte_offset,
                                byte_count,
                                ..
                            },
                    },
                    _,
                ],
            ) => registers.extend_from_slice(
                omega_isa_aarch64::constant_host_result_clobbers(*byte_offset, *byte_count)
                    .as_slice(),
            ),
            _ => {}
        }
    }
    let evidence = StateFootprintEvidence::new(
        RegisterSet::new(registers),
        if has_syscall {
            MachineStateSet::new([
                MachineState::Flags,
                MachineState::InstructionPointer,
                MachineState::StackPointer,
                MachineState::ControlState,
            ])
        } else {
            MachineStateSet::empty()
        },
    );
    omega_calling_conventions::validate_outbound_call_footprint(boundary, &evidence)?;
    Ok(evidence)
}

fn derive_boundary_compiler_body_outbound_syscall_result_footprint_for_arguments(
    boundary: &ValidatedBoundaryEntryPlan,
    input: &crate::InstructionSelectionInput<'_>,
    operands: &psi_arena::Arena<omega_abstract_operations::InstructionOperand>,
    instructions: &[omega_abstract_operations::AbstractOperation],
    argument_class: OutboundSyscallResultArgumentClass,
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    use omega_abstract_operations::{AbstractOperationKind, InstructionOperandKind};
    use omega_calling_conventions::{EntryControl, HostBindingMechanism};

    let mut registers = Vec::new();
    let mut has_syscall = false;
    for instruction in instructions {
        let AbstractOperationKind::HostOperation {
            operation_ordinal,
            operands: operand_span,
            ..
        } = &instruction.kind
        else {
            continue;
        };
        let Some((_, host_call)) = input.host_calls.calls.iter().find(|(_, host_call)| {
            host_call.source_key == instruction.source_key
                && host_call.statement_index == instruction.source_statement
        }) else {
            continue;
        };
        let Some(operation) = input
            .host_calls
            .operations
            .span(host_call.operations)
            .and_then(|operations| operations.get(usize::from(*operation_ordinal)))
        else {
            continue;
        };
        let Some((_, binding)) = input
            .host_abi
            .bindings
            .iter()
            .find(|(_, binding)| binding.operation_key == operation.operation_key)
        else {
            continue;
        };
        let Some(call_operands) = operands.span(*operand_span) else {
            continue;
        };
        let Some((result, arguments)) = call_operands.split_first() else {
            continue;
        };
        let InstructionOperandKind::RuntimeScalarInteger {
            byte_offset,
            byte_count,
            ..
        } = &result.kind
        else {
            continue;
        };
        let has_storage_argument = arguments.iter().any(|operand| {
            abstract_outbound_syscall_storage_argument_is_closed(input.target.architecture, operand)
        });
        let has_data_argument = arguments
            .iter()
            .any(abstract_outbound_syscall_data_argument_is_closed);
        if !matches!(binding.mechanism, HostBindingMechanism::Syscall { .. })
            || operation.operation_key.uses_linux_timespec_result()
            || operation.operation_key.uses_linux_timespec_argument()
            || binding.call_plan().result.is_none()
            || operation.operation_key.discards_native_result()
            || binding.call_plan().parameters.len() != arguments.len()
            || !matches!(
                binding.call_plan().entry_control,
                EntryControl::SupervisorCall { .. }
            )
            || !match argument_class {
                OutboundSyscallResultArgumentClass::Immediate => {
                    !has_storage_argument && !has_data_argument
                }
                OutboundSyscallResultArgumentClass::Storage => {
                    has_storage_argument && !has_data_argument
                }
                OutboundSyscallResultArgumentClass::Data => has_data_argument,
            }
            || !arguments.iter().all(|operand| {
                matches!(
                    operand.kind,
                    InstructionOperandKind::ImmediateInteger(_)
                        | InstructionOperandKind::ByteLength(_)
                ) || abstract_outbound_syscall_storage_argument_is_closed(
                    input.target.architecture,
                    operand,
                ) || abstract_outbound_syscall_data_argument_is_closed(operand)
            })
        {
            continue;
        }
        has_syscall = true;
        registers.extend_from_slice(binding.call_plan().ordinary_clobbers.as_slice());
        if input.target.architecture == omega_target::Architecture::Aarch64 {
            registers.extend_from_slice(
                omega_isa_aarch64::constant_host_result_clobbers(*byte_offset, *byte_count)
                    .as_slice(),
            );
        }
    }
    let evidence = StateFootprintEvidence::new(
        RegisterSet::new(registers),
        if has_syscall {
            MachineStateSet::new([
                MachineState::Flags,
                MachineState::InstructionPointer,
                MachineState::ControlState,
            ])
        } else {
            MachineStateSet::empty()
        },
    );
    omega_calling_conventions::validate_outbound_call_footprint(boundary, &evidence)?;
    Ok(evidence)
}
