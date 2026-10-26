use isa_aarch64::{
    Aarch64CallbackAddressMaterialization, Aarch64CallbackMaterializationDestination,
    Aarch64NormalizedForeignCallFixup, Aarch64NormalizedForeignCallFixupKind,
    Aarch64NormalizedForeignCallFixupState,
    validate_aarch64_selected_normalized_foreign_call_template,
};
use isa_x86_64::{
    X86_64CallbackAddressMaterialization, X86_64CallbackMaterializationDestination,
    X86_64NormalizedForeignCallFixup, X86_64NormalizedForeignCallFixupKind,
    X86_64NormalizedForeignCallFixupState,
    validate_x86_64_selected_normalized_foreign_call_template,
};
use physical_instructions::PostAllocationMachineInstruction;
use register_model::ValidatedPhysicalRegisterModel;
use selected_instructions::{SelectedInstructionId, SelectedInstructionKind};
use target::{Architecture, NativeTarget};

use super::{admit_decoded_footprint, decoded_effects_footprint, decoded_footprint, operand_views};
use crate::OptimizedSelectedFormEncodingError;
use crate::row_encoding::normalized_foreign_call_record;
use machine_code::{
    SelectedFormEncodedRowStore, SelectedFormEncodingRow, SelectedFormEncodingState,
    SelectedFormNormalizedForeignCallFixupKind, SelectedFormNormalizedForeignCallFixupState,
};

/// Independently validate the retained entry mechanism and its bytes. Kernel
/// entries must decode the selected roster's number without any import fixup.
/// For an unresolved normalized-foreign-call row, the
/// validator re-derives the canonical template from the machine row and the
/// kind's `{boundary, ordinal}` binding; a substituted boundary, ordinal,
/// byte, fixup, or effect cannot self-certify. Each callback entry in the
/// fixup replays into the destination roster the ISA validator cross-checks
/// against the constraint row's declared shape.
pub(super) fn validate(
    target: NativeTarget,
    selected_function: &selected_instructions::SelectedFunction,
    instruction: SelectedInstructionId,
    kind: SelectedInstructionKind,
    machine: &PostAllocationMachineInstruction,
    physical: &ValidatedPhysicalRegisterModel,
    row: &SelectedFormEncodingRow,
    encoded: &SelectedFormEncodedRowStore,
) -> Result<(), OptimizedSelectedFormEncodingError> {
    let record = normalized_foreign_call_record(target, instruction, kind, selected_function)?;
    if record.call.binding.table_dispatch().is_some() {
        if target.architecture != Architecture::X86_64
            || !matches!(row.state, SelectedFormEncodingState::Encoded)
        {
            return Err(OptimizedSelectedFormEncodingError::ArtifactMismatch);
        }
        let (bytes, retained) = encoded
            .encoded(row)
            .ok_or(OptimizedSelectedFormEncodingError::ArtifactMismatch)?;
        let accesses = machine
            .operands
            .iter()
            .map(|operand| operand.access)
            .collect::<Vec<_>>();
        let validated = isa_x86_64::validate_x86_64_selected_table_call_form(
            record,
            machine.alternative.key,
            &operand_views(machine),
            &accesses,
            bytes,
        )
        .map_err(|_| OptimizedSelectedFormEncodingError::ArtifactMismatch)?;
        let footprint = validated.footprint();
        let decoded = decoded_footprint(
            &footprint.register_reads,
            &footprint.register_writes,
            &footprint.encoded,
        );
        admit_decoded_footprint(instruction, machine, bytes.len(), retained, &decoded)?;
        return Ok(());
    }
    if let Some(binding) = record.call.binding.system_call() {
        if !matches!(row.state, SelectedFormEncodingState::Encoded) {
            return Err(OptimizedSelectedFormEncodingError::ArtifactMismatch);
        }
        let Some((bytes, footprint)) = encoded.encoded(row) else {
            return Err(OptimizedSelectedFormEncodingError::ArtifactMismatch);
        };
        if !record.call.callbacks.is_empty() {
            return Err(OptimizedSelectedFormEncodingError::ArtifactMismatch);
        }
        let views = operand_views(machine);
        let accesses = machine
            .operands
            .iter()
            .map(|operand| operand.access)
            .collect::<Vec<_>>();
        let decoded = match target.architecture {
            Architecture::X86_64 => {
                let validated = isa_x86_64::validate_x86_64_selected_system_call_form(
                    target,
                    physical,
                    kind,
                    machine.alternative.key,
                    &views,
                    &accesses,
                    binding.number,
                    bytes,
                )
                .map_err(|_| OptimizedSelectedFormEncodingError::ArtifactMismatch)?;
                let footprint = validated.footprint();
                decoded_footprint(
                    &footprint.register_reads,
                    &footprint.register_writes,
                    &footprint.encoded,
                )
            }
            Architecture::Aarch64 => {
                let validated = isa_aarch64::validate_aarch64_selected_system_call_form(
                    target,
                    physical,
                    kind,
                    machine.alternative.key,
                    &views,
                    &accesses,
                    binding.number,
                    bytes,
                )
                .map_err(|_| OptimizedSelectedFormEncodingError::ArtifactMismatch)?;
                let footprint = validated.footprint();
                decoded_footprint(
                    &footprint.register_reads,
                    &footprint.register_writes,
                    &footprint.encoded,
                )
            }
        };
        admit_decoded_footprint(instruction, machine, bytes.len(), footprint, &decoded)?;
        return Ok(());
    }
    let SelectedFormEncodingState::UnresolvedNormalizedForeignCall { fixup } = &row.state else {
        return Err(OptimizedSelectedFormEncodingError::ArtifactMismatch);
    };
    let Some((bytes, footprint)) = encoded.encoded(row) else {
        return Err(OptimizedSelectedFormEncodingError::ArtifactMismatch);
    };
    if fixup.state != SelectedFormNormalizedForeignCallFixupState::UnresolvedImportFieldV1
        || fixup.addend != 0
    {
        return Err(OptimizedSelectedFormEncodingError::ArtifactMismatch);
    }
    let (boundary, ordinal) = match kind {
        SelectedInstructionKind::NormalizedForeignCall { boundary, ordinal } => (boundary, ordinal),
        _ => return Err(OptimizedSelectedFormEncodingError::ArtifactMismatch),
    };
    if fixup.boundary != boundary || fixup.ordinal != ordinal {
        return Err(OptimizedSelectedFormEncodingError::ArtifactMismatch);
    }
    let views = operand_views(machine);
    let accesses = machine
        .operands
        .iter()
        .map(|operand| operand.access)
        .collect::<Vec<_>>();
    let decoded = match target.architecture {
        Architecture::X86_64 => {
            let target_kind = match fixup.kind {
                SelectedFormNormalizedForeignCallFixupKind::X86Relative32FromNextInstructionToNormalizedForeignImportV1 => {
                    X86_64NormalizedForeignCallFixupKind::Relative32FromNextInstructionToNormalizedForeignImportV1
                }
                _ => return Err(OptimizedSelectedFormEncodingError::ArtifactMismatch),
            };
            let (destinations, callbacks) = x86_64_callback_replay(fixup)?;
            let validated = validate_x86_64_selected_normalized_foreign_call_template(
                target,
                physical,
                kind,
                machine.alternative.key,
                &views,
                &accesses,
                &machine.alternative.encoded,
                bytes,
                X86_64NormalizedForeignCallFixup {
                    kind: target_kind,
                    state: X86_64NormalizedForeignCallFixupState::UnresolvedImportFieldV1,
                    boundary: fixup.boundary,
                    ordinal: fixup.ordinal,
                    opcode_byte_offset: fixup.opcode_row_offset,
                    patch_byte_offset: fixup.patch_row_offset,
                    reference_byte_offset: fixup.reference_row_offset,
                    patch_byte_width: fixup.patch_byte_width,
                    callbacks,
                },
                &destinations,
                record,
            )
            .map_err(|_| OptimizedSelectedFormEncodingError::ArtifactMismatch)?;
            decoded_effects_footprint(machine, validated.effects())?
        }
        Architecture::Aarch64 => {
            let target_kind = match fixup.kind {
                SelectedFormNormalizedForeignCallFixupKind::Aarch64BranchLinkImmediate26FromInstructionToNormalizedForeignImportV1 => {
                    Aarch64NormalizedForeignCallFixupKind::BranchLinkImmediate26FromInstructionToNormalizedForeignImportV1
                }
                _ => return Err(OptimizedSelectedFormEncodingError::ArtifactMismatch),
            };
            let (destinations, callbacks) = aarch64_callback_replay(fixup)?;
            let validated = validate_aarch64_selected_normalized_foreign_call_template(
                target,
                physical,
                kind,
                machine.alternative.key,
                &views,
                &accesses,
                &machine.alternative.encoded,
                bytes,
                Aarch64NormalizedForeignCallFixup {
                    kind: target_kind,
                    state: Aarch64NormalizedForeignCallFixupState::UnresolvedImportFieldV1,
                    boundary: fixup.boundary,
                    ordinal: fixup.ordinal,
                    opcode_byte_offset: fixup.opcode_row_offset,
                    patch_byte_offset: fixup.patch_row_offset,
                    reference_byte_offset: fixup.reference_row_offset,
                    patch_byte_width: fixup.patch_byte_width,
                    callbacks,
                },
                &destinations,
                record,
            )
            .map_err(|_| OptimizedSelectedFormEncodingError::ArtifactMismatch)?;
            decoded_effects_footprint(machine, validated.effects())?
        }
    };
    admit_decoded_footprint(instruction, machine, bytes.len(), footprint, &decoded)?;
    Ok(())
}

/// Replay every fixup callback entry into the ISA destination roster and
/// the materialization record list the template validator expects.
fn x86_64_callback_replay(
    fixup: &machine_code::SelectedFormNormalizedForeignCallFixup,
) -> Result<
    (
        Vec<X86_64CallbackMaterializationDestination>,
        Vec<X86_64CallbackAddressMaterialization>,
    ),
    OptimizedSelectedFormEncodingError,
> {
    let mut destinations = Vec::with_capacity(fixup.callbacks.len());
    let mut callbacks = Vec::with_capacity(fixup.callbacks.len());
    for callback in &fixup.callbacks {
        let machine_code::SelectedFormCallbackMaterializationEncoding::X86_64Relative32 {
            patch_row_offset,
        } = callback.encoding
        else {
            return Err(OptimizedSelectedFormEncodingError::ArtifactMismatch);
        };
        let destination = match callback.destination {
            machine_code::CallbackAddressDestination::Register(register) => {
                X86_64CallbackMaterializationDestination::Register(register)
            }
            machine_code::CallbackAddressDestination::RecordField {
                root_register,
                byte_offset,
            } => X86_64CallbackMaterializationDestination::RecordField {
                root_register,
                byte_offset,
            },
            machine_code::CallbackAddressDestination::OutgoingStack { byte_offset } => {
                X86_64CallbackMaterializationDestination::OutgoingStack { byte_offset }
            }
        };
        destinations.push(destination);
        callbacks.push(X86_64CallbackAddressMaterialization {
            destination,
            materialization_byte_offset: callback.code_row_offset,
            materialization_byte_count: callback.code_byte_count,
            relocation_byte_offset: patch_row_offset,
        });
    }
    Ok((destinations, callbacks))
}

fn aarch64_callback_replay(
    fixup: &machine_code::SelectedFormNormalizedForeignCallFixup,
) -> Result<
    (
        Vec<Aarch64CallbackMaterializationDestination>,
        Vec<Aarch64CallbackAddressMaterialization>,
    ),
    OptimizedSelectedFormEncodingError,
> {
    let mut destinations = Vec::with_capacity(fixup.callbacks.len());
    let mut callbacks = Vec::with_capacity(fixup.callbacks.len());
    for callback in &fixup.callbacks {
        let machine_code::SelectedFormCallbackMaterializationEncoding::Aarch64PageAddress {
            page_patch_row_offset,
            page_offset_patch_row_offset,
        } = callback.encoding
        else {
            return Err(OptimizedSelectedFormEncodingError::ArtifactMismatch);
        };
        let destination = match callback.destination {
            machine_code::CallbackAddressDestination::Register(register) => {
                Aarch64CallbackMaterializationDestination::Register(register)
            }
            machine_code::CallbackAddressDestination::RecordField {
                root_register,
                byte_offset,
            } => Aarch64CallbackMaterializationDestination::RecordField {
                root_register,
                byte_offset,
            },
            machine_code::CallbackAddressDestination::OutgoingStack { .. } => {
                return Err(OptimizedSelectedFormEncodingError::ArtifactMismatch);
            }
        };
        destinations.push(destination);
        callbacks.push(Aarch64CallbackAddressMaterialization {
            destination,
            materialization_byte_offset: callback.code_row_offset,
            materialization_byte_count: callback.code_byte_count,
            page_relocation_byte_offset: page_patch_row_offset,
            page_offset_relocation_byte_offset: page_offset_patch_row_offset,
        });
    }
    Ok((destinations, callbacks))
}
