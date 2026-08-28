//! Checked-assembly catalog boundary footprints.

use omega_abstract_operations::SelectedInstructionKind;
use omega_calling_conventions::{
    MachineRegister, MachineState, MachineStateSet, PlanDiagnostic, RegisterSet,
    StateFootprintEvidence, ValidatedBoundaryEntryPlan, validate_state_footprint,
};

/// Derive the exact checked-assembly catalog footprint before byte emission.
/// Final-image admission independently reconstructs the same register and
/// machine-state union from retained validation kinds and final bytes.
pub fn derive_boundary_checked_assembly_footprint<'instruction>(
    boundary: &ValidatedBoundaryEntryPlan,
    runtime_value_operands: &impl omega_target_operations::RuntimeValueOperandSource,
    instructions: impl IntoIterator<Item = &'instruction SelectedInstructionKind>,
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    if boundary.plan().call.policy.architecture() != omega_target::Architecture::X86_64 {
        return Ok(StateFootprintEvidence::new(
            RegisterSet::default(),
            MachineStateSet::empty(),
        ));
    }

    let mut registers = Vec::new();
    let mut additional_state = MachineStateSet::empty();
    for instruction in instructions {
        let operand_state = match instruction {
            SelectedInstructionKind::PortWrite { port, value }
            | SelectedInstructionKind::MsrWrite { index: port, value } => [*port, *value]
                .into_iter()
                .fold(MachineStateSet::empty(), |state, operand| {
                    state.union(
                        omega_isa_x86_64::runtime_value_operand_additional_machine_state(
                            runtime_value_operands,
                            operand,
                        ),
                    )
                }),
            SelectedInstructionKind::PortRead { port, .. }
            | SelectedInstructionKind::MsrRead { index: port, .. }
            | SelectedInstructionKind::FlagsRestore { source: port }
            | SelectedInstructionKind::ControlRegisterWrite { source: port, .. } => {
                omega_isa_x86_64::runtime_value_operand_additional_machine_state(
                    runtime_value_operands,
                    *port,
                )
            }
            _ => MachineStateSet::empty(),
        };
        let (writes, state) = match instruction {
            SelectedInstructionKind::MachineHalt => (
                RegisterSet::default(),
                MachineStateSet::new([
                    MachineState::InstructionPointer,
                    MachineState::ControlState,
                ]),
            ),
            SelectedInstructionKind::MemoryFence(_) => {
                (RegisterSet::default(), MachineStateSet::empty())
            }
            SelectedInstructionKind::InterruptControl(_) => (
                RegisterSet::default(),
                MachineStateSet::new([MachineState::Flags]),
            ),
            SelectedInstructionKind::PortWrite { .. } => (
                RegisterSet::new([
                    MachineRegister::X86Rax,
                    MachineRegister::X86Rdx,
                    MachineRegister::X86R10,
                    MachineRegister::X86R11,
                    MachineRegister::X86R15,
                ]),
                MachineStateSet::empty(),
            ),
            SelectedInstructionKind::PortRead { .. } => (
                RegisterSet::new([
                    MachineRegister::X86Rax,
                    MachineRegister::X86Rdx,
                    MachineRegister::X86R10,
                    MachineRegister::X86R15,
                ]),
                MachineStateSet::empty(),
            ),
            SelectedInstructionKind::FlagsSnapshot { .. } => (
                RegisterSet::new([MachineRegister::X86R10, MachineRegister::X86R15]),
                MachineStateSet::new([MachineState::StackPointer]),
            ),
            SelectedInstructionKind::FlagsRestore { .. } => (
                RegisterSet::new([MachineRegister::X86R10, MachineRegister::X86R15]),
                MachineStateSet::new([MachineState::Flags, MachineState::StackPointer]),
            ),
            SelectedInstructionKind::MsrRead { .. } => (
                RegisterSet::new([
                    MachineRegister::X86Rax,
                    MachineRegister::X86Rcx,
                    MachineRegister::X86Rdx,
                    MachineRegister::X86R10,
                    MachineRegister::X86R11,
                    MachineRegister::X86R15,
                ]),
                MachineStateSet::new([MachineState::Flags]),
            ),
            SelectedInstructionKind::MsrWrite { .. } => (
                RegisterSet::new([
                    MachineRegister::X86Rax,
                    MachineRegister::X86Rcx,
                    MachineRegister::X86Rdx,
                    MachineRegister::X86R10,
                    MachineRegister::X86R11,
                    MachineRegister::X86R15,
                ]),
                MachineStateSet::new([
                    MachineState::Flags,
                    MachineState::StackPointer,
                    MachineState::ControlState,
                ]),
            ),
            SelectedInstructionKind::ControlRegisterRead { .. } => (
                RegisterSet::new([MachineRegister::X86R10, MachineRegister::X86R15]),
                MachineStateSet::empty(),
            ),
            SelectedInstructionKind::ControlRegisterWrite { .. } => (
                RegisterSet::new([
                    MachineRegister::X86Rax,
                    MachineRegister::X86R10,
                    MachineRegister::X86R11,
                    MachineRegister::X86R15,
                ]),
                MachineStateSet::new([MachineState::ControlState]),
            ),
            _ => continue,
        };
        registers.extend_from_slice(writes.as_slice());
        additional_state = additional_state.union(state).union(operand_state);
    }
    let evidence = StateFootprintEvidence::new(RegisterSet::new(registers), additional_state);
    validate_state_footprint(boundary, &evidence)?;
    Ok(evidence)
}
