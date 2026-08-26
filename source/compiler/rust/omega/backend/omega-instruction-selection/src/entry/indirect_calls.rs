//! Indirect table and vtable call footprint derivation.

use omega_calling_conventions::{
    MachineState, MachineStateSet, PlanDiagnostic, RegisterSet, StateFootprintEvidence,
    ValidatedBoundaryEntryPlan,
};

/// Derive the compiler-owned footprint for indirect foreign calls whose
/// callee is loaded from a retained vtable or service-table mechanism. These
/// calls have no import relocation, but otherwise own the same call/return
/// machine-state envelope as direct imports.
pub fn derive_boundary_compiler_body_outbound_indirect_call_footprint(
    boundary: &ValidatedBoundaryEntryPlan,
    input: &crate::InstructionSelectionInput<'_>,
    operands: &psi_arena::Arena<omega_abstract_operations::InstructionOperand>,
    instructions: &[omega_abstract_operations::AbstractOperation],
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    use omega_abstract_operations::{AbstractOperationKind, InstructionOperandKind};
    use omega_calling_conventions::{EntryControl, HostBindingMechanism, MachineRegister};

    let mut registers = Vec::new();
    let mut has_call = false;
    for instruction in instructions {
        if let AbstractOperationKind::DynamicTableCall {
            call_plan,
            result_present,
            operands: operand_span,
            ..
        } = &instruction.kind
        {
            let Some(selected_operands) = operands.span(*operand_span) else {
                continue;
            };
            if !matches!(call_plan.entry_control, EntryControl::CallReturn)
                || selected_operands.len()
                    != call_plan.parameters.len() + 1 + usize::from(*result_present)
            {
                continue;
            }
            has_call = true;
            registers.extend_from_slice(call_plan.ordinary_clobbers.as_slice());
            match input.target.architecture {
                omega_target::Architecture::X86_64 => registers.push(MachineRegister::X86Rsp),
                omega_target::Architecture::Aarch64 => {
                    registers.push(MachineRegister::Aarch64X(16));
                    if *result_present
                        && let Some((byte_offset, byte_count)) =
                            selected_operands
                                .first()
                                .and_then(|operand| match &operand.kind {
                                    InstructionOperandKind::RuntimeScalarInteger {
                                        byte_offset,
                                        byte_count,
                                        ..
                                    }
                                    | InstructionOperandKind::RuntimeScalarFloat {
                                        byte_offset,
                                        byte_count,
                                        ..
                                    } => Some((*byte_offset, *byte_count)),
                                    _ => None,
                                })
                    {
                        registers.extend_from_slice(
                            omega_isa_aarch64::constant_host_result_clobbers(
                                byte_offset,
                                byte_count,
                            )
                            .as_slice(),
                        );
                    }
                }
            }
            continue;
        }
        let AbstractOperationKind::HostOperation {
            operation_ordinal,
            operands: operand_span,
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
        let Some(selected_operands) = operands.span(*operand_span) else {
            continue;
        };
        let dispatch_only = match binding.mechanism {
            HostBindingMechanism::VtableSlot { .. } | HostBindingMechanism::VtableField { .. } => 0,
            HostBindingMechanism::TableFunction { .. } => 1,
            _ => continue,
        };
        if !matches!(binding.call_plan().entry_control, EntryControl::CallReturn)
            || selected_operands.is_empty()
        {
            continue;
        }
        let parameter_count = binding.call_plan().parameters.len() + dispatch_only;
        let result_present = selected_operands.len() == parameter_count + 1;
        if selected_operands.len() != parameter_count && !result_present {
            continue;
        }
        has_call = true;
        registers.extend_from_slice(binding.call_plan().ordinary_clobbers.as_slice());
        match input.target.architecture {
            omega_target::Architecture::X86_64 => registers.push(MachineRegister::X86Rsp),
            omega_target::Architecture::Aarch64 => {
                registers.push(MachineRegister::Aarch64X(16));
                if result_present {
                    let result_range = selected_operands.first().and_then(|operand| match &operand
                        .kind
                    {
                        InstructionOperandKind::RuntimeScalarInteger {
                            byte_offset,
                            byte_count,
                            ..
                        }
                        | InstructionOperandKind::RuntimeScalarFloat {
                            byte_offset,
                            byte_count,
                            ..
                        } => Some((*byte_offset, *byte_count)),
                        _ => None,
                    });
                    if let Some((byte_offset, byte_count)) = result_range {
                        registers.extend_from_slice(
                            omega_isa_aarch64::constant_host_result_clobbers(
                                byte_offset,
                                byte_count,
                            )
                            .as_slice(),
                        );
                    }
                }
            }
        }
    }
    let evidence = StateFootprintEvidence::new(
        RegisterSet::new(registers),
        if has_call {
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
