//! Compact-binary wire append/read boundary-footprint derivation.

use omega_abstract_operations::SelectedInstructionKind;
use omega_calling_conventions::{
    MachineStateSet, PlanDiagnostic, RegisterSet, StateFootprintEvidence,
    ValidatedBoundaryEntryPlan, validate_state_footprint,
};

/// Derive the exact scratch footprint of compiler-generated compact-binary
/// framing-byte appends. Final-image validation replays the same closed
/// encoder while independently binding both relocated storage roots.
pub fn derive_boundary_compiler_body_wire_literal_byte_append_footprint<'instruction>(
    boundary: &ValidatedBoundaryEntryPlan,
    instructions: impl IntoIterator<Item = &'instruction SelectedInstructionKind>,
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    let architecture = boundary.plan().call.policy.architecture();
    let mut registers = Vec::new();
    let mut additional_state = MachineStateSet::empty();
    for instruction in instructions {
        if !matches!(
            instruction,
            SelectedInstructionKind::AppendWireLiteralByte { .. }
        ) {
            continue;
        }
        match architecture {
            omega_target::Architecture::X86_64 => {
                registers.extend_from_slice(
                    omega_isa_x86_64::append_wire_literal_byte_clobbers().as_slice(),
                );
                additional_state = additional_state
                    .union(omega_isa_x86_64::append_wire_literal_byte_additional_machine_state());
            }
            omega_target::Architecture::Aarch64 => registers.extend_from_slice(
                omega_isa_aarch64::append_wire_literal_byte_clobbers().as_slice(),
            ),
        }
    }
    let evidence = StateFootprintEvidence::new(RegisterSet::new(registers), additional_state);
    validate_state_footprint(boundary, &evidence)?;
    Ok(evidence)
}

/// Derive the exact scratch footprint of compiler-generated compact-binary
/// scalar-varint appends. Final-image validation replays the closed encoder
/// while independently binding the source, output, and cursor storage roots.
pub fn derive_boundary_compiler_body_wire_scalar_varint_append_footprint<'instruction>(
    boundary: &ValidatedBoundaryEntryPlan,
    instructions: impl IntoIterator<Item = &'instruction SelectedInstructionKind>,
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    let architecture = boundary.plan().call.policy.architecture();
    let mut registers = Vec::new();
    let mut additional_state = MachineStateSet::empty();
    for instruction in instructions {
        if !matches!(
            instruction,
            SelectedInstructionKind::AppendWireScalarVarint { .. }
        ) {
            continue;
        }
        match architecture {
            omega_target::Architecture::X86_64 => {
                registers.extend_from_slice(
                    omega_isa_x86_64::append_wire_scalar_varint_clobbers().as_slice(),
                );
                additional_state = additional_state
                    .union(omega_isa_x86_64::append_wire_scalar_varint_additional_machine_state());
            }
            omega_target::Architecture::Aarch64 => registers.extend_from_slice(
                omega_isa_aarch64::append_wire_scalar_varint_clobbers().as_slice(),
            ),
        }
    }
    let evidence = StateFootprintEvidence::new(RegisterSet::new(registers), additional_state);
    validate_state_footprint(boundary, &evidence)?;
    Ok(evidence)
}

/// Derive the exact scratch footprint of compiler-generated compact-binary
/// text appends, including the length-varint and capacity-bounded copy loops.
pub fn derive_boundary_compiler_body_wire_text_bytes_append_footprint<'instruction>(
    boundary: &ValidatedBoundaryEntryPlan,
    instructions: impl IntoIterator<Item = &'instruction SelectedInstructionKind>,
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    let architecture = boundary.plan().call.policy.architecture();
    let mut registers = Vec::new();
    let mut additional_state = MachineStateSet::empty();
    for instruction in instructions {
        if !matches!(
            instruction,
            SelectedInstructionKind::AppendWireTextBytes { .. }
        ) {
            continue;
        }
        match architecture {
            omega_target::Architecture::X86_64 => {
                registers.extend_from_slice(
                    omega_isa_x86_64::append_wire_text_bytes_clobbers().as_slice(),
                );
                additional_state = additional_state
                    .union(omega_isa_x86_64::append_wire_text_bytes_additional_machine_state());
            }
            omega_target::Architecture::Aarch64 => {
                registers.extend_from_slice(
                    omega_isa_aarch64::append_wire_text_bytes_clobbers().as_slice(),
                );
                additional_state = additional_state
                    .union(omega_isa_aarch64::append_wire_text_bytes_additional_machine_state());
            }
        }
    }
    let evidence = StateFootprintEvidence::new(RegisterSet::new(registers), additional_state);
    validate_state_footprint(boundary, &evidence)?;
    Ok(evidence)
}

/// Derive the exact scratch footprint of compiler-generated compact-binary
/// borrowed scalar-slice appends, including both the measurement and emission
/// passes over the descriptor.
pub fn derive_boundary_compiler_body_wire_scalar_slice_append_footprint<'instruction>(
    boundary: &ValidatedBoundaryEntryPlan,
    instructions: impl IntoIterator<Item = &'instruction SelectedInstructionKind>,
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    let architecture = boundary.plan().call.policy.architecture();
    let mut registers = Vec::new();
    let mut additional_state = MachineStateSet::empty();
    for instruction in instructions {
        if !matches!(
            instruction,
            SelectedInstructionKind::AppendWireScalarSlice { .. }
        ) {
            continue;
        }
        match architecture {
            omega_target::Architecture::X86_64 => {
                registers.extend_from_slice(
                    omega_isa_x86_64::append_wire_scalar_slice_clobbers().as_slice(),
                );
                additional_state = additional_state
                    .union(omega_isa_x86_64::append_wire_scalar_slice_additional_machine_state());
            }
            omega_target::Architecture::Aarch64 => {
                registers.extend_from_slice(
                    omega_isa_aarch64::append_wire_scalar_slice_clobbers().as_slice(),
                );
                additional_state = additional_state
                    .union(omega_isa_aarch64::append_wire_scalar_slice_additional_machine_state());
            }
        }
    }
    let evidence = StateFootprintEvidence::new(RegisterSet::new(registers), additional_state);
    validate_state_footprint(boundary, &evidence)?;
    Ok(evidence)
}

/// Derive the exact scratch footprint of compiler-generated compact-binary
/// repeated scalar appends, including the runtime-count guard around each
/// statically unrolled element.
pub fn derive_boundary_compiler_body_wire_repeated_scalar_varint_append_footprint<'instruction>(
    boundary: &ValidatedBoundaryEntryPlan,
    instructions: impl IntoIterator<Item = &'instruction SelectedInstructionKind>,
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    let architecture = boundary.plan().call.policy.architecture();
    let mut registers = Vec::new();
    let mut additional_state = MachineStateSet::empty();
    for instruction in instructions {
        if !matches!(
            instruction,
            SelectedInstructionKind::AppendWireRepeatedScalarVarint { .. }
        ) {
            continue;
        }
        match architecture {
            omega_target::Architecture::X86_64 => {
                registers.extend_from_slice(
                    omega_isa_x86_64::append_wire_repeated_scalar_varint_clobbers().as_slice(),
                );
                additional_state = additional_state.union(
                    omega_isa_x86_64::append_wire_repeated_scalar_varint_additional_machine_state(),
                );
            }
            omega_target::Architecture::Aarch64 => {
                registers.extend_from_slice(
                    omega_isa_aarch64::append_wire_repeated_scalar_varint_clobbers().as_slice(),
                );
                additional_state = additional_state.union(
                    omega_isa_aarch64::append_wire_repeated_scalar_varint_additional_machine_state(
                    ),
                );
            }
        }
    }
    let evidence = StateFootprintEvidence::new(RegisterSet::new(registers), additional_state);
    validate_state_footprint(boundary, &evidence)?;
    Ok(evidence)
}

/// Derive the exact scratch footprint of compiler-generated compact-binary
/// framing-byte reads. The AArch64 cursor/verdict offset forms determine
/// whether the address scratch register participates in the sequence.
pub fn derive_boundary_compiler_body_wire_expected_byte_read_footprint<'instruction>(
    boundary: &ValidatedBoundaryEntryPlan,
    instructions: impl IntoIterator<Item = &'instruction SelectedInstructionKind>,
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    let architecture = boundary.plan().call.policy.architecture();
    let mut registers = Vec::new();
    let mut additional_state = MachineStateSet::empty();
    for instruction in instructions {
        let SelectedInstructionKind::ReadWireExpectedByte {
            read_offset,
            ok_offset,
            ..
        } = instruction
        else {
            continue;
        };
        match architecture {
            omega_target::Architecture::X86_64 => {
                registers.extend_from_slice(
                    omega_isa_x86_64::read_wire_expected_byte_clobbers().as_slice(),
                );
                additional_state = additional_state
                    .union(omega_isa_x86_64::read_wire_expected_byte_additional_machine_state());
            }
            omega_target::Architecture::Aarch64 => {
                registers.extend_from_slice(
                    omega_isa_aarch64::read_wire_expected_byte_clobbers(*read_offset, *ok_offset)
                        .as_slice(),
                );
                additional_state = additional_state
                    .union(omega_isa_aarch64::read_wire_expected_byte_additional_machine_state());
            }
        }
    }
    let evidence = StateFootprintEvidence::new(RegisterSet::new(registers), additional_state);
    validate_state_footprint(boundary, &evidence)?;
    Ok(evidence)
}

/// Derive the exact scratch footprint of compiler-generated compact-binary
/// scalar-varint reads, including the arithmetic flags consumed by canonical,
/// range, and signed-decode branches.
pub fn derive_boundary_compiler_body_wire_scalar_varint_read_footprint<'instruction>(
    boundary: &ValidatedBoundaryEntryPlan,
    instructions: impl IntoIterator<Item = &'instruction SelectedInstructionKind>,
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    let architecture = boundary.plan().call.policy.architecture();
    let mut registers = Vec::new();
    let mut additional_state = MachineStateSet::empty();
    for instruction in instructions {
        if !matches!(
            instruction,
            SelectedInstructionKind::ReadWireScalarVarint { .. }
        ) {
            continue;
        }
        match architecture {
            omega_target::Architecture::X86_64 => {
                registers.extend_from_slice(
                    omega_isa_x86_64::read_wire_scalar_varint_clobbers().as_slice(),
                );
                additional_state = additional_state
                    .union(omega_isa_x86_64::read_wire_scalar_varint_additional_machine_state());
            }
            omega_target::Architecture::Aarch64 => {
                registers.extend_from_slice(
                    omega_isa_aarch64::read_wire_scalar_varint_clobbers().as_slice(),
                );
                additional_state = additional_state
                    .union(omega_isa_aarch64::read_wire_scalar_varint_additional_machine_state());
            }
        }
    }
    let evidence = StateFootprintEvidence::new(RegisterSet::new(registers), additional_state);
    validate_state_footprint(boundary, &evidence)?;
    Ok(evidence)
}

/// Derive the exact scratch footprint of compiler-generated compact-binary
/// borrowed byte-slice reads, including length decoding, bounds checks,
/// predicate validation, and zero-copy descriptor construction.
pub fn derive_boundary_compiler_body_wire_byte_slice_read_footprint<'instruction>(
    boundary: &ValidatedBoundaryEntryPlan,
    instructions: impl IntoIterator<Item = &'instruction SelectedInstructionKind>,
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    let architecture = boundary.plan().call.policy.architecture();
    let mut registers = Vec::new();
    let mut additional_state = MachineStateSet::empty();
    for instruction in instructions {
        if !matches!(
            instruction,
            SelectedInstructionKind::ReadWireByteSlice { .. }
        ) {
            continue;
        }
        match architecture {
            omega_target::Architecture::X86_64 => {
                registers.extend_from_slice(
                    omega_isa_x86_64::read_wire_byte_slice_clobbers().as_slice(),
                );
                additional_state = additional_state
                    .union(omega_isa_x86_64::read_wire_byte_slice_additional_machine_state());
            }
            omega_target::Architecture::Aarch64 => {
                registers.extend_from_slice(
                    omega_isa_aarch64::read_wire_byte_slice_clobbers().as_slice(),
                );
                additional_state = additional_state
                    .union(omega_isa_aarch64::read_wire_byte_slice_additional_machine_state());
            }
        }
    }
    let evidence = StateFootprintEvidence::new(RegisterSet::new(registers), additional_state);
    validate_state_footprint(boundary, &evidence)?;
    Ok(evidence)
}

/// Derive the exact scratch footprint of compiler-generated compact-binary
/// nested-open checks, which turn a decoded length into an absolute end bound.
pub fn derive_boundary_compiler_body_wire_nested_open_footprint<'instruction>(
    boundary: &ValidatedBoundaryEntryPlan,
    instructions: impl IntoIterator<Item = &'instruction SelectedInstructionKind>,
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    let architecture = boundary.plan().call.policy.architecture();
    let mut registers = Vec::new();
    let mut additional_state = MachineStateSet::empty();
    for instruction in instructions {
        if !matches!(
            instruction,
            SelectedInstructionKind::ReadWireNestedOpen { .. }
        ) {
            continue;
        }
        match architecture {
            omega_target::Architecture::X86_64 => {
                registers.extend_from_slice(
                    omega_isa_x86_64::read_wire_nested_open_clobbers().as_slice(),
                );
                additional_state = additional_state
                    .union(omega_isa_x86_64::read_wire_nested_open_additional_machine_state());
            }
            omega_target::Architecture::Aarch64 => {
                registers.extend_from_slice(
                    omega_isa_aarch64::read_wire_nested_open_clobbers().as_slice(),
                );
                additional_state = additional_state
                    .union(omega_isa_aarch64::read_wire_nested_open_additional_machine_state());
            }
        }
    }
    let evidence = StateFootprintEvidence::new(RegisterSet::new(registers), additional_state);
    validate_state_footprint(boundary, &evidence)?;
    Ok(evidence)
}

/// Derive the exact scratch footprint of compiler-generated compact-binary
/// nested-close checks, which require the live cursor to equal the end bound.
pub fn derive_boundary_compiler_body_wire_nested_close_footprint<'instruction>(
    boundary: &ValidatedBoundaryEntryPlan,
    instructions: impl IntoIterator<Item = &'instruction SelectedInstructionKind>,
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    let architecture = boundary.plan().call.policy.architecture();
    let mut registers = Vec::new();
    let mut additional_state = MachineStateSet::empty();
    for instruction in instructions {
        if !matches!(
            instruction,
            SelectedInstructionKind::ReadWireNestedClose { .. }
        ) {
            continue;
        }
        match architecture {
            omega_target::Architecture::X86_64 => {
                registers.extend_from_slice(
                    omega_isa_x86_64::read_wire_nested_close_clobbers().as_slice(),
                );
                additional_state = additional_state
                    .union(omega_isa_x86_64::read_wire_nested_close_additional_machine_state());
            }
            omega_target::Architecture::Aarch64 => {
                registers.extend_from_slice(
                    omega_isa_aarch64::read_wire_nested_close_clobbers().as_slice(),
                );
                additional_state = additional_state
                    .union(omega_isa_aarch64::read_wire_nested_close_additional_machine_state());
            }
        }
    }
    let evidence = StateFootprintEvidence::new(RegisterSet::new(registers), additional_state);
    validate_state_footprint(boundary, &evidence)?;
    Ok(evidence)
}

/// Derive the exact scratch footprint of compiler-generated compact-binary
/// guarded repeated-scalar reads, including the end-bound guard, canonical
/// decode, range check, target store, count bump, and sticky verdict merge.
pub fn derive_boundary_compiler_body_wire_repeated_scalar_varint_read_footprint<'instruction>(
    boundary: &ValidatedBoundaryEntryPlan,
    instructions: impl IntoIterator<Item = &'instruction SelectedInstructionKind>,
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    let architecture = boundary.plan().call.policy.architecture();
    let mut registers = Vec::new();
    let mut additional_state = MachineStateSet::empty();
    for instruction in instructions {
        if !matches!(
            instruction,
            SelectedInstructionKind::ReadWireRepeatedScalarVarint { .. }
        ) {
            continue;
        }
        match architecture {
            omega_target::Architecture::X86_64 => {
                registers.extend_from_slice(
                    omega_isa_x86_64::read_wire_repeated_scalar_varint_clobbers().as_slice(),
                );
                additional_state = additional_state.union(
                    omega_isa_x86_64::read_wire_repeated_scalar_varint_additional_machine_state(),
                );
            }
            omega_target::Architecture::Aarch64 => {
                registers.extend_from_slice(
                    omega_isa_aarch64::read_wire_repeated_scalar_varint_clobbers().as_slice(),
                );
                additional_state = additional_state.union(
                    omega_isa_aarch64::read_wire_repeated_scalar_varint_additional_machine_state(),
                );
            }
        }
    }
    let evidence = StateFootprintEvidence::new(RegisterSet::new(registers), additional_state);
    validate_state_footprint(boundary, &evidence)?;
    Ok(evidence)
}
