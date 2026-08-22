//! Compiler-generated boundary control-mechanics footprints.

use omega_abstract_operations::SelectedInstructionKind;
use omega_calling_conventions::{
    EntryControl, MachineStateSet, PlanDiagnostic, RegisterSet, StateFootprintEvidence,
    ValidatedBoundaryEntryPlan, validate_call_return_mechanics_footprint, validate_state_footprint,
};

/// Derive the exact prologue, epilogue, and compiler-private call instruction
/// register/machine-state writes for an ordinary call-return boundary.
pub fn derive_boundary_call_return_mechanics_footprint<'instruction>(
    boundary: &ValidatedBoundaryEntryPlan,
    instructions: impl IntoIterator<Item = &'instruction SelectedInstructionKind>,
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    if boundary.plan().call.entry_control != EntryControl::CallReturn {
        return Err(PlanDiagnostic(
            "ordinary function entry/return lowering requires CallReturn entry control".into(),
        ));
    }
    let architecture = boundary.plan().call.policy.architecture();
    let mut registers = Vec::new();
    let mut additional_state = MachineStateSet::empty();
    let mut enter_count = 0usize;
    let mut return_count = 0usize;
    for instruction in instructions {
        let (writes, state) = match instruction {
            SelectedInstructionKind::EnterFunction => {
                enter_count += 1;
                match architecture {
                    omega_target::Architecture::X86_64 => (
                        omega_isa_x86_64::function_enter_register_writes(),
                        omega_isa_x86_64::function_enter_additional_machine_state(),
                    ),
                    omega_target::Architecture::Aarch64 => (
                        omega_isa_aarch64::function_enter_register_writes(),
                        omega_isa_aarch64::function_enter_additional_machine_state(),
                    ),
                }
            }
            SelectedInstructionKind::LeaveFunction => {
                return_count += 1;
                match architecture {
                    omega_target::Architecture::X86_64 => (
                        omega_isa_x86_64::return_register_writes(),
                        omega_isa_x86_64::return_additional_machine_state(),
                    ),
                    omega_target::Architecture::Aarch64 => (
                        omega_isa_aarch64::return_register_writes(),
                        omega_isa_aarch64::return_additional_machine_state(),
                    ),
                }
            }
            SelectedInstructionKind::CallInternalFunction { .. } => match architecture {
                omega_target::Architecture::X86_64 => (
                    omega_isa_x86_64::internal_function_call_register_writes(),
                    omega_isa_x86_64::internal_function_call_additional_machine_state(),
                ),
                omega_target::Architecture::Aarch64 => (
                    omega_isa_aarch64::internal_function_call_register_writes(),
                    omega_isa_aarch64::internal_function_call_additional_machine_state(),
                ),
            },
            SelectedInstructionKind::CopyEntryIndirectU64ToOutgoingStack { .. } => {
                if architecture != omega_target::Architecture::X86_64 {
                    return Err(PlanDiagnostic(
                        "entry-indirect outgoing stack copies are supported only on x86-64".into(),
                    ));
                }
                (
                    omega_isa_x86_64::entry_indirect_u64_to_outgoing_stack_copy_register_writes(),
                    omega_isa_x86_64::entry_indirect_u64_to_outgoing_stack_copy_additional_machine_state(),
                )
            }
            SelectedInstructionKind::LoadOutgoingStackAddress { register, .. } => {
                if architecture != omega_target::Architecture::X86_64 {
                    return Err(PlanDiagnostic(
                        "outgoing stack-address loads are supported only on x86-64".into(),
                    ));
                }
                (
                    omega_isa_x86_64::outgoing_stack_address_load_register_writes(*register),
                    omega_isa_x86_64::outgoing_stack_address_load_additional_machine_state(),
                )
            }
            SelectedInstructionKind::ReserveOutgoingStackFrame { .. }
            | SelectedInstructionKind::ReleaseOutgoingStackFrame { .. } => {
                if architecture != omega_target::Architecture::X86_64 {
                    return Err(PlanDiagnostic(
                        "outgoing stack frames are supported only on x86-64".into(),
                    ));
                }
                (
                    omega_isa_x86_64::outgoing_stack_frame_adjust_register_writes(),
                    omega_isa_x86_64::outgoing_stack_frame_adjust_additional_machine_state(),
                )
            }
            SelectedInstructionKind::WriteOutgoingStackU64 { .. } => {
                if architecture != omega_target::Architecture::X86_64 {
                    return Err(PlanDiagnostic(
                        "outgoing stack u64 writes are supported only on x86-64".into(),
                    ));
                }
                (
                    omega_isa_x86_64::outgoing_stack_u64_write_register_writes(),
                    omega_isa_x86_64::outgoing_stack_u64_write_additional_machine_state(),
                )
            }
            _ => continue,
        };
        registers.extend_from_slice(writes.as_slice());
        additional_state = additional_state.union(state);
    }
    if enter_count != 1 || return_count != 1 {
        return Err(PlanDiagnostic(format!(
            "ordinary boundary mechanics require exactly one function entry and return (found {enter_count} entries and {return_count} returns)"
        )));
    }
    let evidence = StateFootprintEvidence::new(RegisterSet::new(registers), additional_state);
    validate_call_return_mechanics_footprint(boundary, &evidence)?;
    Ok(evidence)
}

/// Derive the compiler-generated runtime-dispatch scaffold separately from
/// authored/body operations. The scaffold owns the dispatch-state register;
/// case-entry comparisons additionally write condition flags. Guard operand
/// evaluation remains a later whole-body evidence slice.
pub fn derive_boundary_dispatch_scaffold_footprint<'instruction>(
    boundary: &ValidatedBoundaryEntryPlan,
    instructions: impl IntoIterator<Item = &'instruction SelectedInstructionKind>,
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    let architecture = boundary.plan().call.policy.architecture();
    let mut registers = Vec::new();
    let mut additional_state = MachineStateSet::empty();
    let mut loop_enter_count = 0usize;
    let mut loop_leave_count = 0usize;
    for instruction in instructions {
        let (writes, state) = match instruction {
            SelectedInstructionKind::EnterDispatchLoop { .. } => {
                loop_enter_count += 1;
                match architecture {
                    omega_target::Architecture::X86_64 => (
                        omega_isa_x86_64::dispatch_loop_enter_register_writes(),
                        MachineStateSet::empty(),
                    ),
                    omega_target::Architecture::Aarch64 => (
                        omega_isa_aarch64::dispatch_loop_enter_register_writes(),
                        MachineStateSet::empty(),
                    ),
                }
            }
            SelectedInstructionKind::EnterDispatchCase { .. } => match architecture {
                omega_target::Architecture::X86_64 => (
                    omega_isa_x86_64::dispatch_case_enter_register_writes(),
                    omega_isa_x86_64::dispatch_case_enter_additional_machine_state(),
                ),
                omega_target::Architecture::Aarch64 => (
                    omega_isa_aarch64::dispatch_case_enter_register_writes(),
                    omega_isa_aarch64::dispatch_case_enter_additional_machine_state(),
                ),
            },
            SelectedInstructionKind::SetDispatchState { .. }
            | SelectedInstructionKind::TerminateDispatch => match architecture {
                omega_target::Architecture::X86_64 => (
                    omega_isa_x86_64::dispatch_state_write_register_writes(),
                    MachineStateSet::empty(),
                ),
                omega_target::Architecture::Aarch64 => (
                    omega_isa_aarch64::dispatch_state_write_register_writes(),
                    MachineStateSet::empty(),
                ),
            },
            SelectedInstructionKind::LeaveDispatchCase => match architecture {
                omega_target::Architecture::X86_64 => (
                    omega_isa_x86_64::dispatch_case_leave_register_writes(),
                    MachineStateSet::empty(),
                ),
                omega_target::Architecture::Aarch64 => (
                    omega_isa_aarch64::dispatch_case_leave_register_writes(),
                    MachineStateSet::empty(),
                ),
            },
            SelectedInstructionKind::LeaveDispatchLoop => {
                loop_leave_count += 1;
                (RegisterSet::default(), MachineStateSet::empty())
            }
            _ => continue,
        };
        registers.extend_from_slice(writes.as_slice());
        additional_state = additional_state.union(state);
    }
    if loop_enter_count != 1 || loop_leave_count != 1 {
        return Err(PlanDiagnostic(format!(
            "dispatch scaffold evidence requires exactly one loop entry and leave (found {loop_enter_count} entries and {loop_leave_count} leaves)"
        )));
    }
    let evidence = StateFootprintEvidence::new(RegisterSet::new(registers), additional_state);
    validate_state_footprint(boundary, &evidence)?;
    Ok(evidence)
}
