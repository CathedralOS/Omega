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

use super::SelectedFormEncodedRowDraft;
use crate::OptimizedSelectedFormEncodingError;
use machine_code::{
    SelectedFormEncodingState, SelectedFormNormalizedForeignCallFixup,
    SelectedFormNormalizedForeignCallFixupKind, SelectedFormNormalizedForeignCallFixupState,
};

/// Encode the selected entry mechanism. Kernel entries materialize the exact
/// retained number and are fully encoded here; imports remain unresolved.
/// The import fixup carries the selected `{boundary,
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
) -> Result<SelectedFormEncodedRowDraft, OptimizedSelectedFormEncodingError> {
    let SelectedInstructionKind::NormalizedForeignCall { boundary, ordinal } = kind else {
        return Err(OptimizedSelectedFormEncodingError::ArtifactMismatch);
    };
    let record = selected_function
        .normalized_foreign_calls
        .get(ordinal as usize)
        .ok_or(OptimizedSelectedFormEncodingError::ArtifactMismatch)?;
    if record.instruction != instruction
        || record.call.boundary != boundary
        || record.call.binding.native_target() != Some(target)
        || !record.call.binding.has_valid_entry_mechanism()
    {
        return Err(OptimizedSelectedFormEncodingError::ArtifactMismatch);
    }
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
    if record.call.binding.table_dispatch().is_some() {
        if target.architecture != Architecture::X86_64 {
            return Err(OptimizedSelectedFormEncodingError::ArtifactMismatch);
        }
        let encoded = isa_x86_64::encode_x86_64_selected_table_call_form(
            record,
            machine.alternative.key,
            &views,
            &accesses,
        )
        .map_err(|_| OptimizedSelectedFormEncodingError::ArtifactMismatch)?;
        return super::validated_encoded_draft(
            instruction,
            machine,
            &encoded.footprint().encoded,
            encoded.bytes().to_vec(),
            SelectedFormEncodingState::Encoded,
        );
    }
    if let Some(binding) = record.call.binding.system_call() {
        if !record.call.callbacks.is_empty() {
            return Err(OptimizedSelectedFormEncodingError::ArtifactMismatch);
        }
        let (bytes, effects) = match target.architecture {
            Architecture::X86_64 => {
                let encoded = isa_x86_64::encode_x86_64_selected_system_call_form(
                    target,
                    physical,
                    kind,
                    machine.alternative.key,
                    &views,
                    &accesses,
                    binding.number,
                )
                .map_err(|_| OptimizedSelectedFormEncodingError::ArtifactMismatch)?;
                (
                    encoded.bytes().to_vec(),
                    encoded.footprint().encoded.clone(),
                )
            }
            Architecture::Aarch64 => {
                let encoded = isa_aarch64::encode_aarch64_selected_system_call_form(
                    target,
                    physical,
                    kind,
                    machine.alternative.key,
                    &views,
                    &accesses,
                    binding.number,
                )
                .map_err(|_| OptimizedSelectedFormEncodingError::ArtifactMismatch)?;
                (
                    encoded.bytes().to_vec(),
                    encoded.footprint().encoded.clone(),
                )
            }
        };
        return super::validated_encoded_draft(
            instruction,
            machine,
            &effects,
            bytes,
            SelectedFormEncodingState::Encoded,
        );
    }
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
                    machine_code::CallbackAddressDestination::OutgoingStack { byte_offset } => {
                        Ok(X86_64CallbackMaterializationDestination::OutgoingStack {
                            byte_offset: *byte_offset,
                        })
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
                record,
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
                        X86_64CallbackMaterializationDestination::OutgoingStack { byte_offset } =>
                            machine_code::CallbackAddressDestination::OutgoingStack { byte_offset },
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
                record,
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
    super::validated_encoded_draft(
        instruction,
        machine,
        &effects,
        bytes,
        SelectedFormEncodingState::UnresolvedNormalizedForeignCall { fixup },
    )
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
            match (
                &callback.destination,
                callback.application.placement.locations.as_slice(),
            ) {
                (
                    calling_conventions::NativePlace::Parameter(_),
                    [calling_conventions::ValueLocation::Register { register, .. }],
                ) => Ok(machine_code::CallbackAddressDestination::Register(
                    *register,
                )),
                (
                    calling_conventions::NativePlace::Parameter(_),
                    [
                        calling_conventions::ValueLocation::Stack {
                            stack_byte_offset, ..
                        },
                    ],
                ) => Ok(machine_code::CallbackAddressDestination::OutgoingStack {
                    byte_offset: *stack_byte_offset,
                }),
                (
                    calling_conventions::NativePlace::Field { .. },
                    [calling_conventions::ValueLocation::Register { register, .. }],
                ) => {
                    let byte_offset = u32::try_from(
                        callback
                            .field_byte_offset
                            .ok_or(OptimizedSelectedFormEncodingError::ArtifactMismatch)?,
                    )
                    .map_err(|_| OptimizedSelectedFormEncodingError::ArtifactMismatch)?;
                    Ok(machine_code::CallbackAddressDestination::RecordField {
                        root_register: *register,
                        byte_offset,
                    })
                }
                _ => Err(OptimizedSelectedFormEncodingError::ArtifactMismatch),
            }
        })
        .collect()
}
