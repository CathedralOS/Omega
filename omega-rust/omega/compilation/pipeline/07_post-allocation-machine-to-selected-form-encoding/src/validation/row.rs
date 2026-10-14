use isa_aarch64::validate_aarch64_selected_form_encoding;
use isa_x86_64::validate_x86_64_selected_form_encoding;
use physical_instructions::PostAllocationMachineInstruction;
use register_model::{RegisterViewId, ValidatedPhysicalRegisterModel};
use selected_instructions::{
    MachineEncodedEffects, MachineSizeKnowledge, SelectedInstruction, SelectedInstructionId,
    SelectedInstructionKind,
};
use target::{Architecture, NativeTarget};

use super::super::OptimizedSelectedFormEncodingError;
use machine_code::{
    DeferredControlEncodingReason, SelectedFormDecodedFootprint, SelectedFormEncodedRowStore,
    SelectedFormEncodingRow, SelectedFormEncodingState, SelectedFormMachineDisposition,
};

mod normalized_foreign_call;
mod scalar_call;

pub(crate) fn validate(
    target: NativeTarget,
    selected_function: &selected_instructions::SelectedFunction,
    selected: &SelectedInstruction,
    machine: &PostAllocationMachineInstruction,
    physical: &ValidatedPhysicalRegisterModel,
    row: &SelectedFormEncodingRow,
    encoded: &SelectedFormEncodedRowStore,
) -> Result<(), OptimizedSelectedFormEncodingError> {
    let architecture = target.architecture;
    if row.instruction != selected.id
        || row.alternative != machine.alternative.key
        || row.machine_disposition != SelectedFormMachineDisposition::RetainedV1
    {
        return Err(OptimizedSelectedFormEncodingError::ArtifactMismatch);
    }
    match selected.kind {
        kind @ (SelectedInstructionKind::PlacedScalarAccess { .. }
        | SelectedInstructionKind::Store { .. }
        | SelectedInstructionKind::AddressOffset { .. }
        | SelectedInstructionKind::Load64 { .. }
        | SelectedInstructionKind::LoadPacked { .. }
        | SelectedInstructionKind::StorePacked { .. }
        | selected_instructions::load_width_triple_pattern!(..)
        | SelectedInstructionKind::HostedWriteByteI32 { .. }
        | SelectedInstructionKind::HostedReadByte { .. }
        | SelectedInstructionKind::Load8Indexed
        | SelectedInstructionKind::SaveFloatingControl { .. }
        | SelectedInstructionKind::RestoreFloatingControl { .. }
        | SelectedInstructionKind::Store64 { .. }
        | SelectedInstructionKind::FrameAddress { .. }) => {
            let address = row
                .address
                .ok_or(OptimizedSelectedFormEncodingError::ArtifactMismatch)?;
            // The symbolic operation must be the address family this kind
            // owns; the producer's route declaration is not trusted here.
            if !admits_address_operation(kind, address.symbolic) {
                return Err(OptimizedSelectedFormEncodingError::ArtifactMismatch);
            }
            if !matches!(row.state, SelectedFormEncodingState::Encoded) {
                return Err(OptimizedSelectedFormEncodingError::ArtifactMismatch);
            }
            let Some((bytes, footprint)) = encoded.encoded(row) else {
                return Err(OptimizedSelectedFormEncodingError::ArtifactMismatch);
            };
            let decoded = if architecture == Architecture::Aarch64 {
                let encoded = if matches!(kind, SelectedInstructionKind::HostedWriteByteI32 { .. })
                {
                    isa_aarch64::validate_aarch64_selected_hosted_write_byte_form(
                        target,
                        physical,
                        kind,
                        machine.alternative.key,
                        &operand_views(machine),
                        address.displacement,
                        bytes,
                    )
                } else if matches!(kind, SelectedInstructionKind::HostedReadByte { .. }) {
                    isa_aarch64::validate_aarch64_selected_hosted_read_byte_form(
                        target,
                        physical,
                        kind,
                        machine.alternative.key,
                        &operand_views(machine),
                        address.displacement,
                        bytes,
                    )
                } else if matches!(
                    kind,
                    SelectedInstructionKind::FrameAddress { .. }
                        | SelectedInstructionKind::Store64 { .. }
                ) {
                    isa_aarch64::validate_aarch64_selected_frame_form(
                        physical,
                        kind,
                        machine.alternative.key,
                        &operand_views(machine),
                        u32::try_from(address.displacement)
                            .map_err(|_| OptimizedSelectedFormEncodingError::ArtifactMismatch)?,
                        address.frame_alignment_bytes,
                        bytes,
                    )
                } else {
                    isa_aarch64::validate_aarch64_selected_memory_form(
                        physical,
                        kind,
                        machine.alternative.key,
                        &operand_views(machine),
                        address.displacement,
                        bytes,
                    )
                }
                .map_err(OptimizedSelectedFormEncodingError::Aarch64)?;
                decoded_footprint(
                    &encoded.footprint().register_reads,
                    &encoded.footprint().register_writes,
                    &encoded.footprint().encoded,
                )
            } else {
                let encoded = if matches!(kind, SelectedInstructionKind::HostedWriteByteI32 { .. })
                {
                    isa_x86_64::validate_x86_64_selected_hosted_write_byte_form(
                        target,
                        physical,
                        kind,
                        machine.alternative.key,
                        &operand_views(machine),
                        address.displacement,
                        bytes,
                    )
                } else if matches!(kind, SelectedInstructionKind::HostedReadByte { .. }) {
                    isa_x86_64::validate_x86_64_selected_hosted_read_byte_form(
                        target,
                        physical,
                        kind,
                        machine.alternative.key,
                        &operand_views(machine),
                        address.displacement,
                        bytes,
                    )
                } else if matches!(
                    kind,
                    SelectedInstructionKind::FrameAddress { .. }
                        | SelectedInstructionKind::Store64 { .. }
                ) {
                    isa_x86_64::validate_x86_64_selected_frame_form(
                        physical,
                        kind,
                        machine.alternative.key,
                        &operand_views(machine),
                        address.displacement,
                        address.frame_alignment_bytes,
                        bytes,
                    )
                } else {
                    isa_x86_64::validate_x86_64_selected_memory_form(
                        physical,
                        kind,
                        machine.alternative.key,
                        &operand_views(machine),
                        address.displacement,
                        bytes,
                    )
                }
                .map_err(OptimizedSelectedFormEncodingError::X86_64)?;
                decoded_footprint(
                    &encoded.footprint().register_reads,
                    &encoded.footprint().register_writes,
                    &encoded.footprint().encoded,
                )
            };
            validate_machine_footprint(selected.id, machine, &decoded)?;
            validate_size(selected.id, machine.alternative.size, bytes.len())?;
            if footprint != &decoded {
                return Err(OptimizedSelectedFormEncodingError::ArtifactMismatch);
            }
            Ok(())
        }
        kind @ (selected_instructions::selected_call_triple_pattern!(..)) => {
            if row.address.is_some() {
                return Err(OptimizedSelectedFormEncodingError::ArtifactMismatch);
            }
            scalar_call::validate(target, selected.id, kind, machine, physical, row, encoded)
        }
        kind @ SelectedInstructionKind::NormalizedForeignCall { .. } => {
            if row.address.is_some() {
                return Err(OptimizedSelectedFormEncodingError::ArtifactMismatch);
            }
            normalized_foreign_call::validate(
                target,
                selected_function,
                selected.id,
                kind,
                machine,
                physical,
                row,
                encoded,
            )
        }
        SelectedInstructionKind::ConditionalBranchNonZero
        | SelectedInstructionKind::ConditionalBranchU64LessThan
        | SelectedInstructionKind::ConditionalBranchI64LessThan
        | SelectedInstructionKind::Jump => {
            if row.address.is_some()
                || row.state
                    != (SelectedFormEncodingState::DeferredControl {
                        reason: DeferredControlEncodingReason::RequiresResolvedBranchLayout,
                    })
            {
                return Err(OptimizedSelectedFormEncodingError::ArtifactMismatch);
            }
            Ok(())
        }
        kind @ SelectedInstructionKind::ImmutableImageAddress { reference } => {
            // The field coordinates come from the target's own declaration,
            // never from the producer's fixup.
            let fields = match architecture {
                Architecture::X86_64 => machine_code::ByteLiteralAddressFields::X86_64Relative32 {
                    patch_offset: u64::from(isa_x86_64::X86_64_BYTE_LITERAL_ADDRESS_FIELD_OFFSET),
                },
                Architecture::Aarch64 => {
                    machine_code::ByteLiteralAddressFields::Aarch64PageAddress {
                        page_patch_offset: u64::from(
                            isa_aarch64::AARCH64_BYTE_LITERAL_ADDRESS_PAGE_FIELD_OFFSET,
                        ),
                        page_offset_patch_offset: u64::from(
                            isa_aarch64::AARCH64_BYTE_LITERAL_ADDRESS_PAGE_OFFSET_FIELD_OFFSET,
                        ),
                    }
                }
            };
            let matched = match (reference, &row.state) {
                (
                    selected_instructions::SelectedImageAddressReference::ByteLiteral {
                        operation,
                        place,
                    },
                    SelectedFormEncodingState::UnresolvedByteLiteralAddress { fixup },
                ) => {
                    fixup.literal == machine_code::ByteLiteralReference { operation, place }
                        && fixup.fields == fields
                }
                (
                    selected_instructions::SelectedImageAddressReference::ImplementationTable {
                        table,
                    },
                    SelectedFormEncodingState::UnresolvedImplementationTableAddress { fixup },
                ) => fixup.reference.table == table && fixup.fields == fields,
                _ => false,
            };
            if !matched {
                return Err(OptimizedSelectedFormEncodingError::ArtifactMismatch);
            }
            // The baseline route admits the same bytes and footprint the
            // literal row already interned; only its unresolved field differs.
            let retagged = SelectedFormEncodingRow {
                state: SelectedFormEncodingState::Encoded,
                ..row.clone()
            };
            validate_baseline(
                target,
                selected.id,
                kind,
                machine,
                physical,
                &retagged,
                encoded,
            )
        }
        kind => validate_baseline(target, selected.id, kind, machine, physical, row, encoded),
    }
}

/// The validator's own kind→address-operation binding. The producer's route
/// descriptors are not trusted here: a resolved address naming a different
/// operation family contradicts the selected kind no matter how consistent
/// its displacement is.
fn admits_address_operation(
    kind: SelectedInstructionKind,
    operation: physical_instructions::PhysicalAddressOperation,
) -> bool {
    use SelectedInstructionKind as Kind;
    use physical_instructions::PhysicalAddressOperation as Operation;
    if let (
        Kind::PlacedScalarAccess {
            form,
            contract,
            byte_offset,
        },
        Operation::PlacedScalarAccess {
            base_operand: 0,
            form: actual_form,
            contract: actual_contract,
            byte_offset: actual_offset,
        },
    ) = (kind, operation)
    {
        return form == actual_form
            && contract == actual_contract
            && byte_offset == actual_offset
            && form.ordinary_semantic().is_some();
    }
    matches!(
        (kind, operation),
        (Kind::Store { .. }, Operation::Store { .. })
            | (Kind::AddressOffset { .. }, Operation::AddressOffset { .. })
            | (Kind::Load64 { .. }, Operation::Load64 { .. })
            | (Kind::LoadPacked { .. }, Operation::LoadPacked { .. })
            | (Kind::StorePacked { .. }, Operation::StorePacked { .. })
            | (Kind::Load8 { .. }, Operation::Load8 { .. })
            | (Kind::Load16 { .. }, Operation::Load16 { .. })
            | (Kind::Load32 { .. }, Operation::Load32 { .. })
            | (Kind::Load8Indexed, Operation::Load8Indexed { .. })
            | (
                Kind::HostedReadByte { .. },
                Operation::HostedReadByte { .. }
            )
            | (
                Kind::HostedWriteByteI32 { .. },
                Operation::HostedWriteByteI32 { .. }
            )
            | (
                Kind::SaveFloatingControl { .. },
                Operation::SaveFloatingControl { .. }
            )
            | (
                Kind::RestoreFloatingControl { .. },
                Operation::RestoreFloatingControl { .. }
            )
            | (Kind::Store64 { .. }, Operation::Store64 { .. })
            | (Kind::FrameAddress { .. }, Operation::FrameAddress { .. })
    )
}

fn validate_baseline(
    target: NativeTarget,
    instruction: SelectedInstructionId,
    kind: SelectedInstructionKind,
    machine: &PostAllocationMachineInstruction,
    physical: &ValidatedPhysicalRegisterModel,
    row: &SelectedFormEncodingRow,
    encoded: &SelectedFormEncodedRowStore,
) -> Result<(), OptimizedSelectedFormEncodingError> {
    // Ordinary routes declare no address operation; an attached resolved
    // address is a contradiction the frame-equation parity check cannot see.
    if row.address.is_some() {
        return Err(OptimizedSelectedFormEncodingError::ArtifactMismatch);
    }
    if !matches!(row.state, SelectedFormEncodingState::Encoded) {
        return Err(OptimizedSelectedFormEncodingError::ArtifactMismatch);
    }
    let Some((bytes, footprint)) = encoded.encoded(row) else {
        return Err(OptimizedSelectedFormEncodingError::ArtifactMismatch);
    };
    let views = operand_views(machine);
    let decoded = match target.architecture {
        Architecture::X86_64 => {
            let decoded = if kind == SelectedInstructionKind::HostedExitProcessI32 {
                isa_x86_64::validate_x86_64_selected_hosted_exit_process_form(
                    target,
                    physical,
                    kind,
                    machine.alternative.key,
                    &views,
                    bytes,
                )
            } else {
                validate_x86_64_selected_form_encoding(
                    physical,
                    kind,
                    machine.alternative.key,
                    &views,
                    bytes,
                )
            }
            .map_err(|_| OptimizedSelectedFormEncodingError::ArtifactMismatch)?;
            decoded_footprint(
                &decoded.footprint().register_reads,
                &decoded.footprint().register_writes,
                &decoded.footprint().encoded,
            )
        }
        Architecture::Aarch64 => {
            let decoded = if kind == SelectedInstructionKind::HostedExitProcessI32 {
                isa_aarch64::validate_aarch64_selected_hosted_exit_process_form(
                    target,
                    physical,
                    kind,
                    machine.alternative.key,
                    &views,
                    bytes,
                )
            } else {
                validate_aarch64_selected_form_encoding(
                    physical,
                    kind,
                    machine.alternative.key,
                    &views,
                    bytes,
                )
            }
            .map_err(|_| OptimizedSelectedFormEncodingError::ArtifactMismatch)?;
            decoded_footprint(
                &decoded.footprint().register_reads,
                &decoded.footprint().register_writes,
                &decoded.footprint().encoded,
            )
        }
    };
    validate_machine_footprint(instruction, machine, &decoded)?;
    validate_size(instruction, machine.alternative.size, bytes.len())?;
    if footprint != &decoded {
        return Err(OptimizedSelectedFormEncodingError::ArtifactMismatch);
    }
    Ok(())
}

fn decoded_footprint(
    reads: &[RegisterViewId],
    writes: &[RegisterViewId],
    encoded: &MachineEncodedEffects,
) -> SelectedFormDecodedFootprint {
    SelectedFormDecodedFootprint {
        register_reads: reads.to_vec(),
        register_writes: writes.to_vec(),
        implicit_defs: encoded.implicit_unit_defs.clone(),
        implicit_clobbers: encoded.implicit_unit_clobbers.clone(),
        encoded: encoded.clone(),
    }
}

fn validate_machine_footprint(
    instruction: SelectedInstructionId,
    machine: &PostAllocationMachineInstruction,
    decoded: &SelectedFormDecodedFootprint,
) -> Result<(), OptimizedSelectedFormEncodingError> {
    validate_external_operands(instruction, machine, decoded)?;
    if decoded.encoded != machine.alternative.encoded {
        return Err(OptimizedSelectedFormEncodingError::ImplicitFootprintMismatch(instruction));
    }
    Ok(())
}

fn validate_external_operands(
    instruction: SelectedInstructionId,
    machine: &PostAllocationMachineInstruction,
    decoded: &SelectedFormDecodedFootprint,
) -> Result<(), OptimizedSelectedFormEncodingError> {
    let resolve = |operand: u16| machine.operand_footprint(operand).map(|row| row.view);
    let expected_reads = decoded
        .encoded
        .external_operand_reads
        .iter()
        .map(|operand| resolve(*operand))
        .collect::<Option<Vec<_>>>()
        .ok_or(OptimizedSelectedFormEncodingError::OperandFootprintMismatch(instruction))?;
    let expected_writes = decoded
        .encoded
        .external_operand_writes
        .iter()
        .map(|operand| resolve(*operand))
        .collect::<Option<Vec<_>>>()
        .ok_or(OptimizedSelectedFormEncodingError::OperandFootprintMismatch(instruction))?;
    if decoded.register_reads != expected_reads || decoded.register_writes != expected_writes {
        return Err(OptimizedSelectedFormEncodingError::OperandFootprintMismatch(instruction));
    }
    Ok(())
}

pub(crate) fn validate_size(
    instruction: SelectedInstructionId,
    knowledge: MachineSizeKnowledge,
    actual: usize,
) -> Result<(), OptimizedSelectedFormEncodingError> {
    let actual = u16::try_from(actual)
        .map_err(|_| OptimizedSelectedFormEncodingError::SizeDeclarationMismatch(instruction))?;
    let matches = match knowledge {
        MachineSizeKnowledge::ExactBytes(expected) => actual == expected,
        MachineSizeKnowledge::EncoderResolved {
            minimum_bytes,
            maximum_bytes,
        } => actual >= minimum_bytes && maximum_bytes.is_none_or(|maximum| actual <= maximum),
    };
    if !matches {
        return Err(OptimizedSelectedFormEncodingError::SizeDeclarationMismatch(
            instruction,
        ));
    }
    Ok(())
}

fn operand_views(machine: &PostAllocationMachineInstruction) -> Vec<RegisterViewId> {
    machine
        .operands
        .iter()
        .map(|operand| operand.view)
        .collect()
}
