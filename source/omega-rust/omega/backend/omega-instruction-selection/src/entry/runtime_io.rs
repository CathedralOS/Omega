//! Runtime byte and line host-adapter footprint derivation.

use omega_calling_conventions::{
    MachineState, MachineStateSet, PlanDiagnostic, RegisterSet, StateFootprintEvidence,
    ValidatedBoundaryEntryPlan,
};

/// Derive the retained stdin byte adapter. This is a composite implementation
/// leaf, not a second language boundary: Linux owns one normalized read
/// syscall plan, Darwin one AAPCS64 read plan, and Win64 the complete
/// GetStdHandle + ReadFile pair.
pub fn derive_boundary_compiler_body_runtime_byte_read_footprint(
    boundary: &ValidatedBoundaryEntryPlan,
    input: &crate::InstructionSelectionInput<'_>,
    instructions: &[omega_abstract_operations::AbstractOperation],
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    derive_boundary_compiler_body_runtime_byte_footprint(boundary, input, instructions, true)
}

/// Derive the retained stdout byte adapter under the same target-owned plan
/// rules as [`derive_boundary_compiler_body_runtime_byte_read_footprint`].
pub fn derive_boundary_compiler_body_runtime_byte_write_footprint(
    boundary: &ValidatedBoundaryEntryPlan,
    input: &crate::InstructionSelectionInput<'_>,
    instructions: &[omega_abstract_operations::AbstractOperation],
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    derive_boundary_compiler_body_runtime_byte_footprint(boundary, input, instructions, false)
}

/// Derive the complete target-owned line-read adapter without exposing its
/// byte-at-a-time native subcalls as an outer Omega ABI.
pub fn derive_boundary_compiler_body_runtime_line_read_footprint(
    boundary: &ValidatedBoundaryEntryPlan,
    input: &crate::InstructionSelectionInput<'_>,
    instructions: &[omega_abstract_operations::AbstractOperation],
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    use omega_abstract_operations::{AbstractOperationKind, RuntimeTextReadTarget};
    use omega_calling_conventions::{
        HostBindingMechanism, HostCapability, HostOperation, HostOperationKey, MachineRegister,
    };

    let operation_key = HostOperationKey::new(HostCapability::Stdin, HostOperation::Read);
    let binding = input
        .host_abi
        .bindings
        .iter()
        .find_map(|(_, binding)| (binding.operation_key == operation_key).then_some(binding));
    let mut registers = Vec::new();
    let mut has_adapter = false;
    let mut has_import = false;
    for instruction in instructions {
        let (target_offset, target) = match &instruction.kind {
            AbstractOperationKind::ReadRuntimeTextLine {
                target_offset,
                target,
                ..
            } => (*target_offset, *target),
            _ => continue,
        };
        let Some(binding) = binding else {
            continue;
        };
        if !matches!(
            binding.mechanism,
            HostBindingMechanism::Import { .. } | HostBindingMechanism::Syscall { .. }
        ) {
            continue;
        }
        has_adapter = true;
        let is_import = matches!(binding.mechanism, HostBindingMechanism::Import { .. });
        has_import |= is_import;
        registers.extend_from_slice(binding.call_plan().ordinary_clobbers.as_slice());
        match input.target.architecture {
            omega_target::Architecture::X86_64 => {
                registers.extend([MachineRegister::X86R14, MachineRegister::X86R15]);
                if is_import || target == RuntimeTextReadTarget::StringDescriptor {
                    registers.push(MachineRegister::X86R13);
                }
                if is_import {
                    registers.push(MachineRegister::X86Rsp);
                    let handle_key = HostOperationKey::new(
                        operation_key.capability,
                        HostOperation::GetStdHandle,
                    );
                    if let Some(handle_binding) =
                        input.host_abi.bindings.iter().find_map(|(_, candidate)| {
                            (candidate.operation_key == handle_key).then_some(candidate)
                        })
                    {
                        registers.extend_from_slice(
                            handle_binding.call_plan().ordinary_clobbers.as_slice(),
                        );
                    }
                }
            }
            omega_target::Architecture::Aarch64 => {
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
    }
    let mut machine_state = if has_adapter {
        MachineStateSet::new([
            MachineState::Flags,
            MachineState::InstructionPointer,
            MachineState::ControlState,
        ])
    } else {
        MachineStateSet::empty()
    };
    if has_import {
        machine_state = machine_state.union(MachineStateSet::new([MachineState::StackPointer]));
    }
    let evidence = StateFootprintEvidence::new(RegisterSet::new(registers), machine_state);
    omega_calling_conventions::validate_outbound_call_footprint(boundary, &evidence)?;
    Ok(evidence)
}

fn derive_boundary_compiler_body_runtime_byte_footprint(
    boundary: &ValidatedBoundaryEntryPlan,
    input: &crate::InstructionSelectionInput<'_>,
    instructions: &[omega_abstract_operations::AbstractOperation],
    read: bool,
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    use omega_abstract_operations::AbstractOperationKind;
    use omega_calling_conventions::{
        HostBindingMechanism, HostCapability, HostOperation, HostOperationKey, MachineRegister,
    };

    let operation_key = HostOperationKey::new(
        if read {
            HostCapability::Stdin
        } else {
            HostCapability::Stdout
        },
        if read {
            HostOperation::Read
        } else {
            HostOperation::Write
        },
    );
    let binding = input
        .host_abi
        .bindings
        .iter()
        .find_map(|(_, binding)| (binding.operation_key == operation_key).then_some(binding));
    let mut registers = Vec::new();
    let mut has_adapter = false;
    let mut has_import = false;
    for instruction in instructions {
        let source_offset = match (&instruction.kind, read) {
            (AbstractOperationKind::ReadRuntimeByte { .. }, true) => Some(0),
            (AbstractOperationKind::WriteRuntimeByte { source_offset, .. }, false) => {
                Some(*source_offset)
            }
            _ => None,
        };
        let Some(source_offset) = source_offset else {
            continue;
        };
        let Some(binding) = binding else {
            continue;
        };
        if !matches!(
            binding.mechanism,
            HostBindingMechanism::Import { .. } | HostBindingMechanism::Syscall { .. }
        ) {
            continue;
        }
        has_adapter = true;
        has_import |= matches!(binding.mechanism, HostBindingMechanism::Import { .. });
        registers.extend_from_slice(binding.call_plan().ordinary_clobbers.as_slice());
        match input.target.architecture {
            omega_target::Architecture::X86_64 => {
                registers.push(MachineRegister::X86R14);
                if matches!(binding.mechanism, HostBindingMechanism::Import { .. }) {
                    registers.push(MachineRegister::X86Rsp);
                    let get_std_handle_key = HostOperationKey::new(
                        operation_key.capability,
                        HostOperation::GetStdHandle,
                    );
                    if let Some(handle_binding) =
                        input.host_abi.bindings.iter().find_map(|(_, candidate)| {
                            (candidate.operation_key == get_std_handle_key).then_some(candidate)
                        })
                    {
                        registers.extend_from_slice(
                            handle_binding.call_plan().ordinary_clobbers.as_slice(),
                        );
                    }
                }
            }
            omega_target::Architecture::Aarch64 => {
                registers.push(MachineRegister::Aarch64X(20));
                if read || source_offset > 4095 {
                    registers.push(MachineRegister::Aarch64X(9));
                }
            }
        }
    }
    let mut machine_state = if has_adapter {
        MachineStateSet::new([
            MachineState::Flags,
            MachineState::InstructionPointer,
            MachineState::ControlState,
        ])
    } else {
        MachineStateSet::empty()
    };
    if has_import {
        machine_state = machine_state.union(MachineStateSet::new([MachineState::StackPointer]));
    }
    let evidence = StateFootprintEvidence::new(RegisterSet::new(registers), machine_state);
    omega_calling_conventions::validate_outbound_call_footprint(boundary, &evidence)?;
    Ok(evidence)
}
