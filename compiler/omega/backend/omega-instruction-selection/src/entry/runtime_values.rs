//! Runtime-operand atomic and conversion-write boundary footprints.

use omega_abstract_operations::SelectedInstructionKind;
use omega_calling_conventions::{
    MachineState, MachineStateSet, PlanDiagnostic, RegisterSet, StateFootprintEvidence,
    ValidatedBoundaryEntryPlan, validate_runtime_value_guard_footprint,
};

/// Derive the closed encoder-family footprint of compiler-owned atomic loads,
/// stores, RMWs, swaps, and compare-exchanges from the same retained operand
/// arena consumed by byte emission.
pub fn derive_boundary_compiler_body_atomic_footprint<'instruction>(
    boundary: &ValidatedBoundaryEntryPlan,
    runtime_value_operands: &impl omega_target_operations::RuntimeValueOperandSource,
    instructions: impl IntoIterator<Item = &'instruction SelectedInstructionKind>,
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    use omega_calling_conventions::MachineRegister;

    let architecture = boundary.plan().call.policy.architecture();
    let mut registers = Vec::new();
    let mut additional_state = MachineStateSet::empty();
    for instruction in instructions {
        if matches!(instruction, SelectedInstructionKind::AtomicLoad { .. }) {
            registers.extend(match architecture {
                omega_target::Architecture::X86_64 => {
                    [MachineRegister::X86R10, MachineRegister::X86R14].as_slice()
                }
                omega_target::Architecture::Aarch64 => {
                    [MachineRegister::Aarch64X(16), MachineRegister::Aarch64X(17)].as_slice()
                }
            });
            continue;
        }
        let (operands, writes_flags, writes_stack) = match instruction {
            SelectedInstructionKind::AtomicStore { value, .. }
            | SelectedInstructionKind::AtomicSwap {
                new_value: value, ..
            } => (vec![*value], false, false),
            SelectedInstructionKind::AtomicFetchXor { value, .. }
            | SelectedInstructionKind::AtomicFetchOr { value, .. }
            | SelectedInstructionKind::AtomicFetchAnd { value, .. } => (vec![*value], true, false),
            SelectedInstructionKind::AtomicFetchAdd { delta, .. }
            | SelectedInstructionKind::AtomicFetchSub { delta, .. } => (vec![*delta], true, false),
            SelectedInstructionKind::AtomicCompareExchange {
                expected,
                new_value,
                ..
            } => (vec![*new_value, *expected], true, true),
            _ => continue,
        };
        let (writes, state) = match architecture {
            omega_target::Architecture::X86_64 => {
                let mut state = MachineStateSet::empty();
                for operand in operands {
                    state = state.union(
                        omega_isa_x86_64::runtime_value_operand_additional_machine_state(
                            runtime_value_operands,
                            operand,
                        ),
                    );
                }
                if writes_flags {
                    state = state.union(MachineStateSet::new([MachineState::Flags]));
                }
                if writes_stack {
                    state = state.union(MachineStateSet::new([MachineState::StackPointer]));
                }
                (
                    omega_isa_x86_64::place_binary_write_register_write_ceiling(),
                    state,
                )
            }
            omega_target::Architecture::Aarch64 => {
                let mut state = MachineStateSet::empty();
                for operand in operands {
                    state = state.union(
                        omega_isa_aarch64::runtime_value_operand_additional_machine_state(
                            runtime_value_operands,
                            operand,
                        ),
                    );
                }
                (
                    omega_isa_aarch64::place_binary_write_register_write_ceiling(),
                    state,
                )
            }
        };
        registers.extend_from_slice(writes.as_slice());
        additional_state = additional_state.union(state);
    }
    let evidence = StateFootprintEvidence::new(RegisterSet::new(registers), additional_state);
    validate_runtime_value_guard_footprint(boundary, &evidence)?;
    Ok(evidence)
}

/// Derive the closed encoder-family footprint for direct compiler-body
/// conversion writes from the same operand arena consumed by emission.
pub fn derive_boundary_compiler_body_storage_convert_write_footprint<'instruction>(
    boundary: &ValidatedBoundaryEntryPlan,
    runtime_value_operands: &impl omega_target_operations::RuntimeValueOperandSource,
    instructions: impl IntoIterator<Item = &'instruction SelectedInstructionKind>,
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    let architecture = boundary.plan().call.policy.architecture();
    let mut registers = Vec::new();
    let mut additional_state = MachineStateSet::empty();
    for instruction in instructions {
        let source = match instruction {
            SelectedInstructionKind::WriteRuntimeStorageConvert { source, .. } => *source,
            SelectedInstructionKind::WritePlaceConvert { target, source, .. }
                if architecture == omega_target::Architecture::X86_64
                    || matches!(
                        crate::classify_write_place_shape(target),
                        crate::WritePlaceShape::Direct { .. }
                            | crate::WritePlaceShape::Pointee { .. }
                            | crate::WritePlaceShape::FrameIndexed { .. }
                            | crate::WritePlaceShape::FrameIndexedByRegion { .. }
                            | crate::WritePlaceShape::FrameBaseIndexed { .. }
                            | crate::WritePlaceShape::MachineIndexed { .. }
                            | crate::WritePlaceShape::MachineDoubleIndexed { .. }
                    )
                    || crate::classify_frame_base_indexed_convert_shape(target).is_some()
                    || crate::classify_frame_base_double_indexed_convert_shape(target)
                        .is_some() =>
            {
                *source
            }
            _ => continue,
        };
        let (writes, state) = match architecture {
            omega_target::Architecture::X86_64 => (
                omega_isa_x86_64::storage_convert_write_register_write_ceiling(),
                omega_isa_x86_64::storage_convert_write_additional_machine_state(
                    runtime_value_operands,
                    source,
                ),
            ),
            omega_target::Architecture::Aarch64 => (
                omega_isa_aarch64::storage_convert_write_register_write_ceiling(),
                omega_isa_aarch64::storage_convert_write_additional_machine_state(
                    runtime_value_operands,
                    source,
                ),
            ),
        };
        registers.extend_from_slice(writes.as_slice());
        additional_state = additional_state.union(state);
    }
    let evidence = StateFootprintEvidence::new(RegisterSet::new(registers), additional_state);
    validate_runtime_value_guard_footprint(boundary, &evidence)?;
    Ok(evidence)
}
