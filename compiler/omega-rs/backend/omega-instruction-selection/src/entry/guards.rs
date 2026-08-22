//! Dispatch-guard boundary footprints.

use omega_abstract_operations::SelectedInstructionKind;
use omega_calling_conventions::{
    MachineStateSet, PlanDiagnostic, RegisterSet, StateFootprintEvidence,
    ValidatedBoundaryEntryPlan, validate_runtime_value_guard_footprint, validate_state_footprint,
};

/// Derive storage-backed static guard comparisons without sweeping the other
/// guard-lowering shapes into this fragment. The target encoders own the fixed
/// GPR/vector scratch identities and condition-flag effect.
pub fn derive_boundary_static_guard_footprint<'instruction>(
    boundary: &ValidatedBoundaryEntryPlan,
    instructions: impl IntoIterator<Item = &'instruction SelectedInstructionKind>,
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    let architecture = boundary.plan().call.policy.architecture();
    let mut registers = Vec::new();
    let mut additional_state = MachineStateSet::empty();
    for instruction in instructions {
        let SelectedInstructionKind::EvaluateDispatchGuard {
            guard_lowering: omega_abstract_operations::StateGuardLowering::CompareStaticValue,
            operator,
            has_storage: true,
            is_float,
            ..
        } = instruction
        else {
            continue;
        };
        if !matches!(
            operator,
            omega_abstract_operations::StateGuardOperator::Equal
                | omega_abstract_operations::StateGuardOperator::NotEqual
                | omega_abstract_operations::StateGuardOperator::Greater
                | omega_abstract_operations::StateGuardOperator::GreaterOrEqual
                | omega_abstract_operations::StateGuardOperator::Less
                | omega_abstract_operations::StateGuardOperator::LessOrEqual
                | omega_abstract_operations::StateGuardOperator::GreaterUnsigned
                | omega_abstract_operations::StateGuardOperator::GreaterOrEqualUnsigned
                | omega_abstract_operations::StateGuardOperator::LessUnsigned
                | omega_abstract_operations::StateGuardOperator::LessOrEqualUnsigned
        ) {
            continue;
        }
        let (writes, state) = match architecture {
            omega_target::Architecture::X86_64 => (
                omega_isa_x86_64::dispatch_guard_compare_static_register_writes(*is_float),
                omega_isa_x86_64::dispatch_guard_compare_static_additional_machine_state(),
            ),
            omega_target::Architecture::Aarch64 => (
                omega_isa_aarch64::dispatch_guard_compare_static_register_writes(*is_float),
                omega_isa_aarch64::dispatch_guard_compare_static_additional_machine_state(),
            ),
        };
        registers.extend_from_slice(writes.as_slice());
        additional_state = additional_state.union(state);
    }
    let evidence = StateFootprintEvidence::new(RegisterSet::new(registers), additional_state);
    validate_state_footprint(boundary, &evidence)?;
    Ok(evidence)
}

/// Derive the fixed register/state effects of the two dedicated runtime-text
/// guard encoders. Computed text equality carried as a runtime value operand
/// and place-shaped comparisons remain separate later slices; this fragment
/// is limited to instruction kinds whose complete bytes are owned by the
/// literal and descriptor-vs-literal encoders.
pub fn derive_boundary_runtime_text_guard_footprint<'instruction>(
    boundary: &ValidatedBoundaryEntryPlan,
    instructions: impl IntoIterator<Item = &'instruction SelectedInstructionKind>,
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    let architecture = boundary.plan().call.policy.architecture();
    let mut registers = Vec::new();
    let mut additional_state = MachineStateSet::empty();
    for instruction in instructions {
        let (writes, state) = match (architecture, instruction) {
            (
                omega_target::Architecture::X86_64,
                SelectedInstructionKind::CompareRuntimeTextLiteral { .. },
            ) => (
                omega_isa_x86_64::runtime_text_literal_compare_register_writes(),
                omega_isa_x86_64::runtime_text_literal_compare_additional_machine_state(),
            ),
            (
                omega_target::Architecture::X86_64,
                SelectedInstructionKind::CompareRuntimeTextStorage { .. },
            ) => (
                omega_isa_x86_64::runtime_text_storage_compare_register_writes(),
                omega_isa_x86_64::runtime_text_storage_compare_additional_machine_state(),
            ),
            (
                omega_target::Architecture::Aarch64,
                SelectedInstructionKind::CompareRuntimeTextLiteral { .. },
            ) => (
                omega_isa_aarch64::runtime_text_literal_compare_register_writes(),
                omega_isa_aarch64::runtime_text_literal_compare_additional_machine_state(),
            ),
            (
                omega_target::Architecture::Aarch64,
                SelectedInstructionKind::CompareRuntimeTextStorage { .. },
            ) => (
                omega_isa_aarch64::runtime_text_storage_compare_register_writes(),
                omega_isa_aarch64::runtime_text_storage_compare_additional_machine_state(),
            ),
            _ => continue,
        };
        registers.extend_from_slice(writes.as_slice());
        additional_state = additional_state.union(state);
    }
    let evidence = StateFootprintEvidence::new(RegisterSet::new(registers), additional_state);
    validate_state_footprint(boundary, &evidence)?;
    Ok(evidence)
}

/// Derive the complete effects of place-pair and place-vs-immediate guards.
/// x86 place walks and AArch64's currently admitted direct-place shapes both
/// obtain their scratch identities from the encoder modules that emit them.
pub fn derive_boundary_place_guard_footprint<'instruction>(
    boundary: &ValidatedBoundaryEntryPlan,
    instructions: impl IntoIterator<Item = &'instruction SelectedInstructionKind>,
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    let architecture = boundary.plan().call.policy.architecture();
    let mut registers = Vec::new();
    let mut additional_state = MachineStateSet::empty();
    for instruction in instructions {
        let footprint = match (architecture, instruction) {
            (
                omega_target::Architecture::X86_64,
                SelectedInstructionKind::ComparePlaces { is_float, .. },
            ) => Some((
                omega_isa_x86_64::place_compare_register_writes(*is_float),
                omega_isa_x86_64::place_compare_additional_machine_state(),
            )),
            (
                omega_target::Architecture::X86_64,
                SelectedInstructionKind::ComparePlaceValue { .. },
            ) => Some((
                omega_isa_x86_64::place_value_compare_register_writes(),
                omega_isa_x86_64::place_value_compare_additional_machine_state(),
            )),
            (
                omega_target::Architecture::Aarch64,
                SelectedInstructionKind::ComparePlaces {
                    left,
                    right,
                    byte_size,
                    is_float,
                    ..
                },
            ) => match (left.const_offset(), right.const_offset()) {
                (Some(left_offset), Some(right_offset)) => Some((
                    omega_isa_aarch64::runtime_storage_compare_register_writes(
                        left_offset,
                        right_offset,
                        *byte_size,
                        *is_float,
                    ),
                    omega_isa_aarch64::runtime_storage_compare_additional_machine_state(),
                )),
                _ => None,
            },
            (
                omega_target::Architecture::Aarch64,
                SelectedInstructionKind::ComparePlaceValue { place, .. },
            ) if place.const_offset().is_some() => Some((
                omega_isa_aarch64::runtime_storage_value_compare_register_writes(),
                omega_isa_aarch64::runtime_storage_value_compare_additional_machine_state(),
            )),
            _ => None,
        };
        let Some((writes, state)) = footprint else {
            continue;
        };
        registers.extend_from_slice(writes.as_slice());
        additional_state = additional_state.union(state);
    }
    let evidence = StateFootprintEvidence::new(RegisterSet::new(registers), additional_state);
    validate_state_footprint(boundary, &evidence)?;
    Ok(evidence)
}

/// Derive the recursive runtime-value guard evaluator's closed encoder-family
/// may-write ceiling. The operand arena is the same arena consumed by byte
/// emission; on x86 it also determines whether a nested `Binary` introduces
/// balanced push/pop stack scratch.
pub fn derive_boundary_runtime_value_guard_footprint<'instruction>(
    boundary: &ValidatedBoundaryEntryPlan,
    runtime_value_operands: &impl omega_target_operations::RuntimeValueOperandSource,
    instructions: impl IntoIterator<Item = &'instruction SelectedInstructionKind>,
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    let architecture = boundary.plan().call.policy.architecture();
    let mut registers = Vec::new();
    let mut additional_state = MachineStateSet::empty();
    for instruction in instructions {
        let SelectedInstructionKind::CompareRuntimeValues { left, right, .. } = instruction else {
            continue;
        };
        let (writes, state) = match architecture {
            omega_target::Architecture::X86_64 => (
                omega_isa_x86_64::runtime_value_compare_register_write_ceiling(),
                omega_isa_x86_64::runtime_value_compare_additional_machine_state(
                    runtime_value_operands,
                    *left,
                    *right,
                ),
            ),
            omega_target::Architecture::Aarch64 => (
                omega_isa_aarch64::runtime_value_compare_register_write_ceiling(),
                omega_isa_aarch64::runtime_value_compare_additional_machine_state(
                    runtime_value_operands,
                    *left,
                    *right,
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
