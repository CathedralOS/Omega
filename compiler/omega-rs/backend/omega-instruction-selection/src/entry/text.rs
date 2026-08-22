//! Bounded-buffer, string-descriptor, and runtime-text boundary footprints.

use omega_abstract_operations::SelectedInstructionKind;
use omega_calling_conventions::{
    MachineStateSet, PlanDiagnostic, RegisterSet, StateFootprintEvidence,
    ValidatedBoundaryEntryPlan, validate_state_footprint,
};

/// Derive the closed encoder-family footprint for retained compiler-body
/// immediate bounded-buffer literal writes.
pub fn derive_boundary_compiler_body_place_bounded_buffer_write_footprint<'instruction>(
    boundary: &ValidatedBoundaryEntryPlan,
    instructions: impl IntoIterator<Item = &'instruction SelectedInstructionKind>,
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    let architecture = boundary.plan().call.policy.architecture();
    let mut registers = Vec::new();
    let mut additional_state = MachineStateSet::empty();
    for instruction in instructions {
        let (target, source, append_kind) = match instruction {
            SelectedInstructionKind::WritePlaceBoundedBuffer { target, .. } => (target, None, 0u8),
            SelectedInstructionKind::AppendPlaceBoundedBufferLiteral { target, .. } => {
                (target, None, 1)
            }
            SelectedInstructionKind::AppendPlaceBoundedBufferSource { target, source } => {
                (target, Some(source), 2)
            }
            _ => continue,
        };
        let target_shape = crate::classify_write_place_shape(target);
        let supported = architecture == omega_target::Architecture::X86_64
            || (!matches!(target_shape, crate::WritePlaceShape::Unsupported)
                && source.map_or(true, |source| {
                    matches!(
                        crate::classify_write_place_shape(source),
                        crate::WritePlaceShape::Direct { .. }
                            | crate::WritePlaceShape::Pointee { .. }
                    )
                }))
            || (append_kind == 0
                && crate::classify_frame_base_indexed_bounded_buffer_shape(target).is_some())
            || (append_kind == 0
                && crate::classify_frame_base_double_indexed_bounded_buffer_shape(target)
                    .is_some())
            || (append_kind == 1
                && crate::classify_frame_base_indexed_bounded_buffer_literal_append_shape(target)
                    .is_some())
            || (append_kind == 1
                && crate::classify_frame_base_double_indexed_bounded_buffer_literal_append_shape(
                    target,
                )
                .is_some())
            || (append_kind == 2
                && (crate::classify_frame_base_indexed_bounded_buffer_source_append_shape(target)
                    .is_some()
                    || crate::classify_frame_base_double_indexed_bounded_buffer_source_append_shape(
                        target,
                    )
                    .is_some())
                && source.is_some_and(|source| {
                    matches!(
                        crate::classify_write_place_shape(source),
                        crate::WritePlaceShape::Direct { .. }
                            | crate::WritePlaceShape::Pointee { .. }
                    )
                }));
        if !supported {
            continue;
        }
        let (writes, state) = match architecture {
            omega_target::Architecture::X86_64 if append_kind == 2 => (
                omega_isa_x86_64::place_bounded_buffer_source_append_register_writes(
                    target,
                    source.expect("source append retains a source"),
                ),
                omega_isa_x86_64::place_bounded_buffer_source_append_additional_machine_state(),
            ),
            omega_target::Architecture::X86_64 if append_kind == 1 => (
                omega_isa_x86_64::place_bounded_buffer_literal_append_register_writes(target),
                omega_isa_x86_64::place_bounded_buffer_literal_append_additional_machine_state(),
            ),
            omega_target::Architecture::X86_64 => (
                omega_isa_x86_64::place_bounded_buffer_write_register_writes(target),
                omega_isa_x86_64::place_bounded_buffer_write_additional_machine_state(target),
            ),
            omega_target::Architecture::Aarch64 if append_kind == 2 => (
                omega_isa_aarch64::place_bounded_buffer_source_append_register_write_ceiling(),
                omega_isa_aarch64::place_bounded_buffer_source_append_additional_machine_state(),
            ),
            omega_target::Architecture::Aarch64 if append_kind == 1 => (
                omega_isa_aarch64::place_bounded_buffer_literal_append_register_write_ceiling(),
                omega_isa_aarch64::place_bounded_buffer_literal_append_additional_machine_state(),
            ),
            omega_target::Architecture::Aarch64 => (
                omega_isa_aarch64::place_bounded_buffer_write_register_write_ceiling(),
                omega_isa_aarch64::place_bounded_buffer_write_additional_machine_state(),
            ),
        };
        registers.extend_from_slice(writes.as_slice());
        additional_state = additional_state.union(state);
    }
    let evidence = StateFootprintEvidence::new(RegisterSet::new(registers), additional_state);
    validate_state_footprint(boundary, &evidence)?;
    Ok(evidence)
}

/// Derive the closed encoder-family footprint for retained compiler-body
/// string-descriptor writes.
pub fn derive_boundary_compiler_body_place_string_write_footprint<'instruction>(
    boundary: &ValidatedBoundaryEntryPlan,
    instructions: impl IntoIterator<Item = &'instruction SelectedInstructionKind>,
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    let architecture = boundary.plan().call.policy.architecture();
    let mut registers = Vec::new();
    let mut additional_state = MachineStateSet::empty();
    for instruction in instructions {
        let SelectedInstructionKind::WritePlaceString { target, .. } = instruction else {
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
                    | crate::WritePlaceShape::MachineDoubleIndexed { .. }
            )
            || crate::classify_frame_base_double_indexed_string_shape(target).is_some()
            || crate::classify_frame_base_indexed_string_shape(target).is_some();
        if !supported {
            continue;
        }
        let (writes, state) = match architecture {
            omega_target::Architecture::X86_64 => (
                omega_isa_x86_64::place_string_write_register_writes(target),
                omega_isa_x86_64::place_string_write_additional_machine_state(target),
            ),
            omega_target::Architecture::Aarch64 => (
                omega_isa_aarch64::place_string_write_register_write_ceiling(),
                omega_isa_aarch64::place_string_write_additional_machine_state(),
            ),
        };
        registers.extend_from_slice(writes.as_slice());
        additional_state = additional_state.union(state);
    }
    let evidence = StateFootprintEvidence::new(RegisterSet::new(registers), additional_state);
    validate_state_footprint(boundary, &evidence)?;
    Ok(evidence)
}

/// Derive the target-encoder footprint for compiler-body runtime-text
/// assembly. This is ordinary lowering evidence: Psi has already established
/// the text operation, while Omega retains the exact buffer/place recipe that
/// the final artifact validator can replay.
pub fn derive_boundary_compiler_body_text_assembly_write_footprint<'instruction>(
    boundary: &ValidatedBoundaryEntryPlan,
    instructions: impl IntoIterator<Item = &'instruction SelectedInstructionKind>,
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    let architecture = boundary.plan().call.policy.architecture();
    let mut registers = Vec::new();
    let mut additional_state = MachineStateSet::empty();
    for instruction in instructions {
        if matches!(
            instruction,
            SelectedInstructionKind::AppendRuntimeTextStoredSuffix { .. }
        ) {
            let (writes, state) = match architecture {
                omega_target::Architecture::X86_64 => (
                    omega_isa_x86_64::runtime_text_stored_suffix_append_register_writes(),
                    omega_isa_x86_64::runtime_text_stored_suffix_append_additional_machine_state(),
                ),
                omega_target::Architecture::Aarch64 => (
                    omega_isa_aarch64::runtime_text_stored_suffix_append_register_writes(),
                    omega_isa_aarch64::runtime_text_stored_suffix_append_additional_machine_state(),
                ),
            };
            registers.extend_from_slice(writes.as_slice());
            additional_state = additional_state.union(state);
            continue;
        }
        let segmented_literal = match instruction {
            SelectedInstructionKind::WriteRuntimeTextLiteral { .. } => {
                architecture == omega_target::Architecture::Aarch64
            }
            SelectedInstructionKind::WriteRuntimeTextLiteralSegment { .. } => true,
            _ => false,
        };
        if segmented_literal {
            let (writes, state) = match architecture {
                omega_target::Architecture::X86_64 => (
                    omega_isa_x86_64::runtime_text_literal_segment_write_register_writes(),
                    omega_isa_x86_64::runtime_text_literal_segment_write_additional_machine_state(),
                ),
                omega_target::Architecture::Aarch64 => (
                    omega_isa_aarch64::runtime_text_literal_segment_write_register_writes(),
                    omega_isa_aarch64::runtime_text_literal_segment_write_additional_machine_state(
                    ),
                ),
            };
            registers.extend_from_slice(writes.as_slice());
            additional_state = additional_state.union(state);
            continue;
        }
        let (target, write_kind) = match instruction {
            SelectedInstructionKind::MaterializeTextBufferToPlace { target, .. } => (target, 0u8),
            SelectedInstructionKind::AppendTextLiteralToPlace { target, .. } => (target, 1u8),
            SelectedInstructionKind::AppendTextStoredToPlace { target, .. } => (target, 2u8),
            _ => continue,
        };
        let shape = crate::classify_write_place_shape(target);
        if architecture == omega_target::Architecture::Aarch64
            && crate::classify_frame_base_double_indexed_text_assembly_shape(target).is_some()
        {
            let (writes, state) = match write_kind {
                0 => (
                    omega_isa_aarch64::runtime_text_buffer_materialize_to_runtime_frame_base_double_indexed_register_writes(),
                    omega_isa_aarch64::runtime_text_buffer_materialize_additional_machine_state(),
                ),
                1 => (
                    omega_isa_aarch64::runtime_text_literal_append_to_runtime_frame_base_double_indexed_register_writes(),
                    omega_isa_aarch64::runtime_text_literal_append_additional_machine_state(),
                ),
                2 => (
                    omega_isa_aarch64::runtime_text_stored_place_append_to_runtime_frame_base_double_indexed_register_writes(),
                    omega_isa_aarch64::runtime_text_stored_place_append_additional_machine_state(),
                ),
                _ => unreachable!("closed text-assembly write kind"),
            };
            registers.extend_from_slice(writes.as_slice());
            additional_state = additional_state.union(state);
            continue;
        }
        let (writes, state) = match (architecture, shape, write_kind) {
            (
                omega_target::Architecture::X86_64,
                crate::WritePlaceShape::Direct { .. } | crate::WritePlaceShape::Pointee { .. },
                2,
            ) => (
                omega_isa_x86_64::runtime_text_stored_place_append_register_writes(),
                omega_isa_x86_64::runtime_text_stored_place_append_additional_machine_state(),
            ),
            (
                omega_target::Architecture::X86_64,
                crate::WritePlaceShape::FrameIndexed { .. },
                2,
            ) => (
                omega_isa_x86_64::runtime_text_stored_place_append_to_runtime_frame_indexed_register_writes(),
                omega_isa_x86_64::runtime_text_stored_place_append_additional_machine_state(),
            ),
            (
                omega_target::Architecture::X86_64,
                crate::WritePlaceShape::Direct { .. } | crate::WritePlaceShape::Pointee { .. },
                1,
            ) => (
                omega_isa_x86_64::runtime_text_literal_append_register_writes(),
                omega_isa_x86_64::runtime_text_literal_append_additional_machine_state(),
            ),
            (
                omega_target::Architecture::X86_64,
                crate::WritePlaceShape::FrameIndexed { .. },
                1,
            ) => (
                omega_isa_x86_64::runtime_text_literal_append_to_runtime_frame_indexed_register_writes(),
                omega_isa_x86_64::runtime_text_literal_append_additional_machine_state(),
            ),
            (
                omega_target::Architecture::X86_64,
                crate::WritePlaceShape::Direct { .. },
                0,
            ) => (
                omega_isa_x86_64::runtime_text_buffer_materialize_register_writes(),
                omega_isa_x86_64::runtime_text_buffer_materialize_additional_machine_state(),
            ),
            (
                omega_target::Architecture::X86_64,
                crate::WritePlaceShape::Pointee { .. },
                0,
            ) => (
                omega_isa_x86_64::runtime_text_buffer_materialize_to_runtime_pointee_register_writes(),
                omega_isa_x86_64::runtime_text_buffer_materialize_additional_machine_state(),
            ),
            (
                omega_target::Architecture::X86_64,
                crate::WritePlaceShape::FrameIndexed { .. },
                0,
            ) => (
                omega_isa_x86_64::runtime_text_buffer_materialize_to_runtime_frame_indexed_register_writes(),
                omega_isa_x86_64::runtime_text_buffer_materialize_additional_machine_state(),
            ),
            (omega_target::Architecture::X86_64, _, 2) => (
                omega_isa_x86_64::place_text_stored_append_register_writes(),
                omega_isa_x86_64::runtime_text_stored_place_append_additional_machine_state(),
            ),
            (omega_target::Architecture::X86_64, _, 1) => (
                omega_isa_x86_64::place_text_literal_append_register_writes(target),
                omega_isa_x86_64::runtime_text_literal_append_additional_machine_state(),
            ),
            (omega_target::Architecture::X86_64, _, 0) => (
                omega_isa_x86_64::place_text_buffer_materialize_register_writes(),
                omega_isa_x86_64::place_text_buffer_materialize_additional_machine_state(target),
            ),
            (
                omega_target::Architecture::Aarch64,
                crate::WritePlaceShape::Direct { .. } | crate::WritePlaceShape::Pointee { .. },
                2,
            ) => (
                omega_isa_aarch64::runtime_text_stored_place_append_register_writes(),
                omega_isa_aarch64::runtime_text_stored_place_append_additional_machine_state(),
            ),
            (
                omega_target::Architecture::Aarch64,
                crate::WritePlaceShape::FrameIndexed { .. }
                | crate::WritePlaceShape::FrameBaseIndexed { .. },
                2,
            ) => (
                omega_isa_aarch64::runtime_text_stored_place_append_to_runtime_frame_indexed_register_writes(),
                omega_isa_aarch64::runtime_text_stored_place_append_additional_machine_state(),
            ),
            (
                omega_target::Architecture::Aarch64,
                crate::WritePlaceShape::FrameBaseIndexed { .. },
                1,
            ) => (
                omega_isa_aarch64::runtime_text_literal_append_to_runtime_frame_base_indexed_register_writes(),
                omega_isa_aarch64::runtime_text_literal_append_additional_machine_state(),
            ),
            (
                omega_target::Architecture::Aarch64,
                crate::WritePlaceShape::Direct { .. }
                | crate::WritePlaceShape::Pointee { .. }
                | crate::WritePlaceShape::FrameIndexed { .. },
                1,
            ) => (
                omega_isa_aarch64::runtime_text_literal_append_register_writes(),
                omega_isa_aarch64::runtime_text_literal_append_additional_machine_state(),
            ),
            (
                omega_target::Architecture::Aarch64,
                crate::WritePlaceShape::Direct { .. },
                0,
            ) => (
                omega_isa_aarch64::runtime_text_buffer_materialize_register_writes(),
                omega_isa_aarch64::runtime_text_buffer_materialize_additional_machine_state(),
            ),
            (
                omega_target::Architecture::Aarch64,
                crate::WritePlaceShape::Pointee { .. },
                0,
            ) => (
                omega_isa_aarch64::runtime_text_buffer_materialize_to_runtime_pointee_register_writes(),
                omega_isa_aarch64::runtime_text_buffer_materialize_additional_machine_state(),
            ),
            (
                omega_target::Architecture::Aarch64,
                crate::WritePlaceShape::FrameIndexed { .. }
                | crate::WritePlaceShape::FrameIndexedByRegion { .. }
                | crate::WritePlaceShape::FrameBaseIndexed { .. },
                0,
            ) => (
                omega_isa_aarch64::runtime_text_buffer_materialize_to_runtime_frame_indexed_register_writes(),
                omega_isa_aarch64::runtime_text_buffer_materialize_additional_machine_state(),
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
