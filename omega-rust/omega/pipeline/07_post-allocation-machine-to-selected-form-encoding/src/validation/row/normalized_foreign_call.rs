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

use super::{decoded_footprint, operand_views, validate_machine_footprint, validate_size};
use crate::OptimizedSelectedFormEncodingError;
use machine_code::{
    SelectedFormEncodingState, SelectedFormNormalizedForeignCallFixupKind,
    SelectedFormNormalizedForeignCallFixupState,
};

/// Independently validate one unresolved normalized-foreign-call row. The
/// validator re-derives the canonical template from the machine row and the
/// kind's `{boundary, ordinal}` binding; a substituted boundary, ordinal,
/// byte, fixup, or effect cannot self-certify. Each callback entry in the
/// fixup replays into the destination roster the ISA validator cross-checks
/// against the constraint row's declared shape.
pub(super) fn validate(
    target: NativeTarget,
    instruction: SelectedInstructionId,
    kind: SelectedInstructionKind,
    machine: &PostAllocationMachineInstruction,
    physical: &ValidatedPhysicalRegisterModel,
    state: &SelectedFormEncodingState,
) -> Result<(), OptimizedSelectedFormEncodingError> {
    let SelectedFormEncodingState::UnresolvedNormalizedForeignCall {
        bytes,
        footprint,
        fixup,
    } = state
    else {
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
            )
            .map_err(|_| OptimizedSelectedFormEncodingError::ArtifactMismatch)?;
            decoded_footprint(
                &views_for_effects(machine, validated.effects(), true)?,
                &views_for_effects(machine, validated.effects(), false)?,
                validated.effects(),
            )
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
            )
            .map_err(|_| OptimizedSelectedFormEncodingError::ArtifactMismatch)?;
            decoded_footprint(
                &views_for_effects(machine, validated.effects(), true)?,
                &views_for_effects(machine, validated.effects(), false)?,
                validated.effects(),
            )
        }
    };
    validate_machine_footprint(instruction, machine, &decoded)?;
    validate_size(instruction, machine.alternative.size, bytes.len())?;
    if footprint.as_ref() != &decoded {
        return Err(OptimizedSelectedFormEncodingError::ArtifactMismatch);
    }
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
            machine_code::CallbackAddressDestination::OutgoingStack { .. } => {
                return Err(OptimizedSelectedFormEncodingError::ArtifactMismatch);
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

fn views_for_effects(
    machine: &PostAllocationMachineInstruction,
    effects: &selected_instructions::MachineEncodedEffects,
    reads: bool,
) -> Result<Vec<register_model::RegisterViewId>, OptimizedSelectedFormEncodingError> {
    let resolve = |operand: u16| {
        machine
            .operands
            .iter()
            .find(|row| row.operand == operand)
            .map(|row| row.view)
    };
    let operands = if reads {
        &effects.external_operand_reads
    } else {
        &effects.external_operand_writes
    };
    operands
        .iter()
        .map(|operand| resolve(*operand))
        .collect::<Option<Vec<_>>>()
        .ok_or(OptimizedSelectedFormEncodingError::OperandFootprintMismatch(machine.instruction))
}
