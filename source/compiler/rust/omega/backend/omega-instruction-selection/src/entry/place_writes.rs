//! Compiler-body scalar, address, binary, and bit-field place-write footprints.

use omega_abstract_operations::SelectedInstructionKind;
use omega_calling_conventions::{
    MachineStateSet, PlanDiagnostic, RegisterSet, StateFootprintEvidence,
    ValidatedBoundaryEntryPlan, validate_runtime_value_guard_footprint, validate_state_footprint,
};

/// Derive the exact scratch footprint of compiler-body immediate integer
/// writes whose final replay contracts have landed. Other place shapes remain
/// separate until their retained target encoder publishes and tests an exact
/// clobber contract.
pub fn derive_boundary_compiler_body_place_integer_write_footprint<'instruction>(
    boundary: &ValidatedBoundaryEntryPlan,
    instructions: impl IntoIterator<Item = &'instruction SelectedInstructionKind>,
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    let architecture = boundary.plan().call.policy.architecture();
    let mut registers = Vec::new();
    for instruction in instructions {
        let SelectedInstructionKind::WritePlaceInteger { target, .. } = instruction else {
            continue;
        };
        let shape = crate::classify_write_place_shape(target);
        let frame_indexed = crate::classify_frame_base_indexed_integer_shape(target);
        let frame_double = crate::classify_frame_base_double_indexed_integer_shape(target);
        if architecture == omega_target::Architecture::Aarch64
            && let Some(frame_indexed) = frame_indexed
        {
            registers.extend_from_slice(
                omega_isa_aarch64::runtime_frame_base_indexed_integer_write_with_index_region_clobbers(
                    frame_indexed.index_region,
                )
                .as_slice(),
            );
            continue;
        }
        let clobbers = match (architecture, shape) {
            (
                omega_target::Architecture::X86_64,
                crate::WritePlaceShape::Direct { .. } | crate::WritePlaceShape::Pointee { .. },
            ) => omega_isa_x86_64::place_integer_write_clobbers(target),
            (
                omega_target::Architecture::Aarch64,
                crate::WritePlaceShape::Direct { byte_offset },
            ) => omega_isa_aarch64::runtime_machine_integer_write_clobbers(byte_offset),
            (
                omega_target::Architecture::Aarch64,
                crate::WritePlaceShape::Pointee {
                    pointer_byte_offset,
                    field_byte_offset,
                },
            ) => omega_isa_aarch64::runtime_pointee_integer_write_clobbers(
                pointer_byte_offset,
                field_byte_offset,
            ),
            (omega_target::Architecture::Aarch64, crate::WritePlaceShape::FrameIndexed { .. }) => {
                omega_isa_aarch64::runtime_frame_indexed_integer_write_clobbers(
                    omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
                )
            }
            (
                omega_target::Architecture::Aarch64,
                crate::WritePlaceShape::FrameIndexedByRegion { index_region, .. },
            ) => omega_isa_aarch64::runtime_frame_indexed_integer_write_clobbers(index_region),
            (
                omega_target::Architecture::X86_64,
                crate::WritePlaceShape::FrameIndexed { .. }
                | crate::WritePlaceShape::FrameIndexedByRegion { .. },
            ) => omega_isa_x86_64::place_integer_write_clobbers(target),
            (
                omega_target::Architecture::Aarch64,
                crate::WritePlaceShape::FrameBaseIndexed { .. },
            ) => omega_isa_aarch64::runtime_frame_base_indexed_integer_write_clobbers(),
            (
                omega_target::Architecture::X86_64,
                crate::WritePlaceShape::FrameBaseIndexed { .. },
            ) => omega_isa_x86_64::place_integer_write_clobbers(target),
            (
                omega_target::Architecture::Aarch64,
                crate::WritePlaceShape::MachineIndexed { .. },
            ) => omega_isa_aarch64::runtime_machine_indexed_integer_write_clobbers(),
            (omega_target::Architecture::X86_64, crate::WritePlaceShape::MachineIndexed { .. }) => {
                omega_isa_x86_64::place_integer_write_clobbers(target)
            }
            (
                omega_target::Architecture::Aarch64,
                crate::WritePlaceShape::MachineDoubleIndexed {
                    outer_index_region,
                    inner_index_region,
                    ..
                },
            ) => omega_isa_aarch64::runtime_machine_double_indexed_integer_write_clobbers(
                outer_index_region,
                inner_index_region,
            ),
            (
                omega_target::Architecture::X86_64,
                crate::WritePlaceShape::MachineDoubleIndexed { .. },
            ) => omega_isa_x86_64::place_integer_write_clobbers(target),
            (
                omega_target::Architecture::Aarch64,
                crate::WritePlaceShape::PointeeDoubleIndexed {
                    outer_index_region,
                    inner_index_region,
                    ..
                },
            ) => omega_isa_aarch64::runtime_pointee_double_indexed_integer_write_clobbers(
                outer_index_region,
                inner_index_region,
            ),
            (
                omega_target::Architecture::X86_64,
                crate::WritePlaceShape::PointeeDoubleIndexed { .. },
            ) => omega_isa_x86_64::place_integer_write_clobbers(target),
            (omega_target::Architecture::X86_64, crate::WritePlaceShape::Unsupported) => {
                omega_isa_x86_64::place_integer_write_clobbers(target)
            }
            (omega_target::Architecture::Aarch64, crate::WritePlaceShape::Unsupported)
                if frame_double.is_some() =>
            {
                omega_isa_aarch64::runtime_frame_base_double_indexed_integer_write_clobbers()
            }
            _ => continue,
        };
        registers.extend_from_slice(clobbers.as_slice());
    }
    let evidence =
        StateFootprintEvidence::new(RegisterSet::new(registers), MachineStateSet::empty());
    validate_state_footprint(boundary, &evidence)?;
    Ok(evidence)
}

/// Derive the exact scratch and machine-state footprint of compiler-body
/// address writes. These operations materialize the address of one canonical
/// `Place` and store it into a runtime-frame pointer slot.
pub fn derive_boundary_compiler_body_place_address_write_footprint<'instruction>(
    boundary: &ValidatedBoundaryEntryPlan,
    instructions: impl IntoIterator<Item = &'instruction SelectedInstructionKind>,
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    let architecture = boundary.plan().call.policy.architecture();
    let mut registers = Vec::new();
    let mut additional_state = MachineStateSet::empty();
    for instruction in instructions {
        match instruction {
            SelectedInstructionKind::WritePlaceAddress {
                source,
                target_offset,
            } => {
                let Ok(clobbers) = crate::write_place_address_register_writes(
                    architecture,
                    source,
                    *target_offset,
                ) else {
                    continue;
                };
                registers.extend_from_slice(clobbers.as_slice());
                additional_state = additional_state.union(
                    crate::write_place_address_additional_machine_state(architecture),
                );
            }
            SelectedInstructionKind::WriteFunctionAddressToRuntimeStorage { .. } => {
                registers.extend_from_slice(match architecture {
                    omega_target::Architecture::X86_64 => &[
                        omega_calling_conventions::MachineRegister::X86R14,
                        omega_calling_conventions::MachineRegister::X86R15,
                    ],
                    omega_target::Architecture::Aarch64 => &[
                        omega_calling_conventions::MachineRegister::Aarch64X(16),
                        omega_calling_conventions::MachineRegister::Aarch64X(17),
                    ],
                });
            }
            _ => {}
        }
    }
    let evidence = StateFootprintEvidence::new(RegisterSet::new(registers), additional_state);
    validate_state_footprint(boundary, &evidence)?;
    Ok(evidence)
}

/// Derive the closed encoder-family footprint for retained compiler-body binary
/// writes. The retained operand arena is the one byte emission consumes, so
/// nested evaluator stack/control-state needs cannot drift from this evidence.
pub fn derive_boundary_compiler_body_place_binary_write_footprint<'instruction>(
    boundary: &ValidatedBoundaryEntryPlan,
    runtime_value_operands: &impl omega_target_operations::RuntimeValueOperandSource,
    instructions: impl IntoIterator<Item = &'instruction SelectedInstructionKind>,
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    let architecture = boundary.plan().call.policy.architecture();
    let mut registers = Vec::new();
    let mut additional_state = MachineStateSet::empty();
    for instruction in instructions {
        let SelectedInstructionKind::WritePlaceBinary {
            target,
            left,
            operator,
            right,
            ..
        } = instruction
        else {
            continue;
        };
        let supported = architecture == omega_target::Architecture::X86_64
            || matches!(
                crate::classify_write_place_shape(target),
                crate::WritePlaceShape::Direct { .. }
                    | crate::WritePlaceShape::Pointee { .. }
                    | crate::WritePlaceShape::FrameIndexed { .. }
                    | crate::WritePlaceShape::FrameIndexedByRegion { .. }
                    | crate::WritePlaceShape::FrameBaseIndexed { .. }
                    | crate::WritePlaceShape::MachineIndexed { .. }
                    | crate::WritePlaceShape::MachineDoubleIndexed { .. },
            )
            || crate::classify_frame_base_indexed_binary_shape(target).is_some()
            || crate::classify_frame_base_double_indexed_binary_shape(target).is_some();
        if !supported {
            continue;
        }
        let (writes, state) = match architecture {
            omega_target::Architecture::X86_64 => (
                omega_isa_x86_64::place_binary_write_register_write_ceiling(),
                omega_isa_x86_64::place_binary_write_additional_machine_state(
                    runtime_value_operands,
                    *left,
                    *operator,
                    *right,
                ),
            ),
            omega_target::Architecture::Aarch64 => (
                omega_isa_aarch64::place_binary_write_register_write_ceiling(),
                omega_isa_aarch64::place_binary_write_additional_machine_state(
                    runtime_value_operands,
                    *left,
                    *operator,
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

/// Derive the closed encoder-family footprint for retained compiler-body
/// immediate bit-field writes.
pub fn derive_boundary_compiler_body_storage_bit_field_write_footprint<'instruction>(
    boundary: &ValidatedBoundaryEntryPlan,
    instructions: impl IntoIterator<Item = &'instruction SelectedInstructionKind>,
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    let architecture = boundary.plan().call.policy.architecture();
    let mut registers = Vec::new();
    let mut additional_state = MachineStateSet::empty();
    for instruction in instructions {
        if !matches!(
            instruction,
            SelectedInstructionKind::WriteStorageBitField { .. }
        ) {
            continue;
        }
        let (writes, state) = match architecture {
            omega_target::Architecture::X86_64 => (
                omega_isa_x86_64::runtime_storage_bit_field_write_register_write_ceiling(),
                omega_isa_x86_64::runtime_storage_bit_field_write_additional_machine_state(),
            ),
            omega_target::Architecture::Aarch64 => (
                omega_isa_aarch64::runtime_storage_bit_field_write_register_write_ceiling(),
                omega_isa_aarch64::runtime_storage_bit_field_write_additional_machine_state(),
            ),
        };
        registers.extend_from_slice(writes.as_slice());
        additional_state = additional_state.union(state);
    }
    let evidence = StateFootprintEvidence::new(RegisterSet::new(registers), additional_state);
    validate_state_footprint(boundary, &evidence)?;
    Ok(evidence)
}
