use isa_aarch64::{
    Aarch64CallbackMaterializationDestination, Aarch64NormalizedForeignCallFixupKind,
    Aarch64NormalizedForeignCallFixupState,
    encode_aarch64_selected_normalized_foreign_call_template,
};
use isa_x86_64::{
    X86_64CallbackMaterializationDestination, X86_64NormalizedForeignCallFixupKind,
    X86_64NormalizedForeignCallFixupState, encode_x86_64_selected_normalized_foreign_call_template,
};
use physical_instructions::PostAllocationMachineInstruction;
use register_model::ValidatedPhysicalRegisterModel;
use selected_instructions::{SelectedFunction, SelectedInstructionId, SelectedInstructionKind};
use target::{Architecture, NativeTarget};

use super::{validate_operand_footprint, validate_size};
use crate::OptimizedSelectedFormEncodingError;
use machine_code::{
    SelectedFormDecodedFootprint, SelectedFormEncodingState,
    SelectedFormNormalizedForeignCallFixup, SelectedFormNormalizedForeignCallFixupKind,
    SelectedFormNormalizedForeignCallFixupState,
};

/// Encode one normalized foreign call row into the canonical unresolved
/// import-field template. The fixup carries the selected `{boundary,
/// ordinal}` roster binding; the byte field stays a canonical zero/immediate
/// placeholder until object construction names the import symbol. The
/// evaluated callback roster on the selected function supplies each private
/// materialization's destination, and the emitted template carries one
/// materialization group per roster entry, in roster order.
pub(super) fn encode(
    target: NativeTarget,
    instruction: SelectedInstructionId,
    kind: SelectedInstructionKind,
    machine: &PostAllocationMachineInstruction,
    physical: &ValidatedPhysicalRegisterModel,
    selected_function: &SelectedFunction,
) -> Result<SelectedFormEncodingState, OptimizedSelectedFormEncodingError> {
    let views = machine
        .operands
        .iter()
        .map(|operand| operand.view)
        .collect::<Vec<_>>();
    let accesses = machine
        .operands
        .iter()
        .map(|operand| operand.access)
        .collect::<Vec<_>>();
    let destinations = callback_destinations(selected_function, instruction, kind)?;
    let (bytes, effects, fixup) = match target.architecture {
        Architecture::X86_64 => {
            let destinations = destinations
                .iter()
                .map(|destination| match destination {
                    machine_code::CallbackAddressDestination::Register(register) => Ok(
                        X86_64CallbackMaterializationDestination::Register(*register),
                    ),
                    machine_code::CallbackAddressDestination::RecordField {
                        root_register,
                        byte_offset,
                    } => Ok(X86_64CallbackMaterializationDestination::RecordField {
                        root_register: *root_register,
                        byte_offset: *byte_offset,
                    }),
                    machine_code::CallbackAddressDestination::OutgoingStack { .. } => {
                        Err(OptimizedSelectedFormEncodingError::ArtifactMismatch)
                    }
                })
                .collect::<Result<Vec<_>, _>>()?;
            let encoded = encode_x86_64_selected_normalized_foreign_call_template(
                target,
                physical,
                kind,
                machine.alternative.key,
                &views,
                &accesses,
                &machine.alternative.encoded,
                &destinations,
            )
            .map_err(OptimizedSelectedFormEncodingError::X86_64NormalizedForeignCall)?;
            let target_fixup = encoded.fixup();
            let kind = match target_fixup.kind {
                X86_64NormalizedForeignCallFixupKind::Relative32FromNextInstructionToNormalizedForeignImportV1 => {
                    SelectedFormNormalizedForeignCallFixupKind::X86Relative32FromNextInstructionToNormalizedForeignImportV1
                }
            };
            let state = match target_fixup.state {
                X86_64NormalizedForeignCallFixupState::UnresolvedImportFieldV1 => {
                    SelectedFormNormalizedForeignCallFixupState::UnresolvedImportFieldV1
                }
            };
            let callbacks = target_fixup
                .callbacks
                .iter()
                .map(|callback| machine_code::SelectedFormCallbackMaterialization {
                    destination: match callback.destination {
                        X86_64CallbackMaterializationDestination::Register(register) => {
                            machine_code::CallbackAddressDestination::Register(register)
                        }
                        X86_64CallbackMaterializationDestination::RecordField {
                            root_register,
                            byte_offset,
                        } => machine_code::CallbackAddressDestination::RecordField {
                            root_register,
                            byte_offset,
                        },
                    },
                    code_row_offset: callback.materialization_byte_offset,
                    code_byte_count: callback.materialization_byte_count,
                    encoding:
                        machine_code::SelectedFormCallbackMaterializationEncoding::X86_64Relative32 {
                            patch_row_offset: callback.relocation_byte_offset,
                        },
                })
                .collect();
            (
                encoded.bytes().to_vec(),
                encoded.effects().clone(),
                SelectedFormNormalizedForeignCallFixup {
                    kind,
                    state,
                    boundary: target_fixup.boundary,
                    ordinal: target_fixup.ordinal,
                    opcode_row_offset: target_fixup.opcode_byte_offset,
                    patch_row_offset: target_fixup.patch_byte_offset,
                    reference_row_offset: target_fixup.reference_byte_offset,
                    patch_byte_width: target_fixup.patch_byte_width,
                    addend: 0,
                    callbacks,
                },
            )
        }
        Architecture::Aarch64 => {
            let destinations = destinations
                .iter()
                .map(|destination| match destination {
                    machine_code::CallbackAddressDestination::Register(register) => Ok(
                        Aarch64CallbackMaterializationDestination::Register(*register),
                    ),
                    machine_code::CallbackAddressDestination::RecordField {
                        root_register,
                        byte_offset,
                    } => Ok(Aarch64CallbackMaterializationDestination::RecordField {
                        root_register: *root_register,
                        byte_offset: *byte_offset,
                    }),
                    machine_code::CallbackAddressDestination::OutgoingStack { .. } => {
                        Err(OptimizedSelectedFormEncodingError::ArtifactMismatch)
                    }
                })
                .collect::<Result<Vec<_>, _>>()?;
            let encoded = encode_aarch64_selected_normalized_foreign_call_template(
                target,
                physical,
                kind,
                machine.alternative.key,
                &views,
                &accesses,
                &machine.alternative.encoded,
                &destinations,
            )
            .map_err(OptimizedSelectedFormEncodingError::Aarch64NormalizedForeignCall)?;
            let target_fixup = encoded.fixup();
            let kind = match target_fixup.kind {
                Aarch64NormalizedForeignCallFixupKind::BranchLinkImmediate26FromInstructionToNormalizedForeignImportV1 => {
                    SelectedFormNormalizedForeignCallFixupKind::Aarch64BranchLinkImmediate26FromInstructionToNormalizedForeignImportV1
                }
            };
            let state = match target_fixup.state {
                Aarch64NormalizedForeignCallFixupState::UnresolvedImportFieldV1 => {
                    SelectedFormNormalizedForeignCallFixupState::UnresolvedImportFieldV1
                }
            };
            let callbacks = target_fixup
                .callbacks
                .iter()
                .map(|callback| machine_code::SelectedFormCallbackMaterialization {
                    destination: match callback.destination {
                        Aarch64CallbackMaterializationDestination::Register(register) => {
                            machine_code::CallbackAddressDestination::Register(register)
                        }
                        Aarch64CallbackMaterializationDestination::RecordField {
                            root_register,
                            byte_offset,
                        } => machine_code::CallbackAddressDestination::RecordField {
                            root_register,
                            byte_offset,
                        },
                    },
                    code_row_offset: callback.materialization_byte_offset,
                    code_byte_count: callback.materialization_byte_count,
                    encoding:
                        machine_code::SelectedFormCallbackMaterializationEncoding::Aarch64PageAddress {
                            page_patch_row_offset: callback.page_relocation_byte_offset,
                            page_offset_patch_row_offset: callback
                                .page_offset_relocation_byte_offset,
                        },
                })
                .collect();
            (
                encoded.bytes().to_vec(),
                encoded.effects().clone(),
                SelectedFormNormalizedForeignCallFixup {
                    kind,
                    state,
                    boundary: target_fixup.boundary,
                    ordinal: target_fixup.ordinal,
                    opcode_row_offset: target_fixup.opcode_byte_offset,
                    patch_row_offset: target_fixup.patch_byte_offset,
                    reference_row_offset: target_fixup.reference_byte_offset,
                    patch_byte_width: target_fixup.patch_byte_width,
                    addend: 0,
                    callbacks,
                },
            )
        }
    };
    let footprint = decoded_footprint(machine, &effects)?;
    validate_operand_footprint(
        instruction,
        machine,
        &effects,
        &footprint.register_reads,
        &footprint.register_writes,
    )?;
    validate_size(instruction, machine.alternative.size, bytes.len())?;
    Ok(SelectedFormEncodingState::UnresolvedNormalizedForeignCall {
        bytes,
        footprint: Box::new(footprint),
        fixup,
    })
}

/// Resolve the instruction's `{boundary, ordinal}` to its custody roster row
/// and derive each private callback's materialization destination from the
/// evaluated `NativePlace` the roster row retains.
fn callback_destinations(
    selected_function: &SelectedFunction,
    instruction: SelectedInstructionId,
    kind: SelectedInstructionKind,
) -> Result<Vec<machine_code::CallbackAddressDestination>, OptimizedSelectedFormEncodingError> {
    let SelectedInstructionKind::NormalizedForeignCall { boundary, ordinal } = kind else {
        return Err(OptimizedSelectedFormEncodingError::ArtifactMismatch);
    };
    let Some(record) = selected_function
        .normalized_foreign_calls
        .get(ordinal as usize)
    else {
        return Err(OptimizedSelectedFormEncodingError::ArtifactMismatch);
    };
    if record.instruction != instruction || record.call.boundary != boundary {
        return Err(OptimizedSelectedFormEncodingError::ArtifactMismatch);
    }
    record
        .call
        .callbacks
        .iter()
        .map(|callback| {
            let root = callback
                .application
                .placement
                .locations
                .first()
                .and_then(|location| match location {
                    calling_conventions::ValueLocation::Register { register, .. } => {
                        Some(*register)
                    }
                    _ => None,
                })
                .ok_or(OptimizedSelectedFormEncodingError::ArtifactMismatch)?;
            match callback.destination {
                calling_conventions::NativePlace::Parameter(_) => {
                    Ok(machine_code::CallbackAddressDestination::Register(root))
                }
                calling_conventions::NativePlace::Field { .. } => {
                    let byte_offset = u32::try_from(
                        callback
                            .field_byte_offset
                            .ok_or(OptimizedSelectedFormEncodingError::ArtifactMismatch)?,
                    )
                    .map_err(|_| OptimizedSelectedFormEncodingError::ArtifactMismatch)?;
                    Ok(machine_code::CallbackAddressDestination::RecordField {
                        root_register: root,
                        byte_offset,
                    })
                }
            }
        })
        .collect()
}

fn decoded_footprint(
    machine: &PostAllocationMachineInstruction,
    effects: &selected_instructions::MachineEncodedEffects,
) -> Result<SelectedFormDecodedFootprint, OptimizedSelectedFormEncodingError> {
    let resolve = |operand: u16| {
        machine
            .operands
            .iter()
            .find(|row| row.operand == operand)
            .map(|row| row.view)
    };
    let register_reads = effects
        .external_operand_reads
        .iter()
        .map(|operand| resolve(*operand))
        .collect::<Option<Vec<_>>>()
        .ok_or(OptimizedSelectedFormEncodingError::OperandFootprintMismatch(machine.instruction))?;
    let register_writes = effects
        .external_operand_writes
        .iter()
        .map(|operand| resolve(*operand))
        .collect::<Option<Vec<_>>>()
        .ok_or(OptimizedSelectedFormEncodingError::OperandFootprintMismatch(machine.instruction))?;
    Ok(SelectedFormDecodedFootprint {
        register_reads,
        register_writes,
        implicit_defs: effects.implicit_unit_defs.clone(),
        implicit_clobbers: effects.implicit_unit_clobbers.clone(),
        encoded: effects.clone(),
    })
}
