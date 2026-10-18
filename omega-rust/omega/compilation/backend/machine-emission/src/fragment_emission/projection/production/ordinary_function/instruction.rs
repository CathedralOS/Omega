use machine_code::{
    FunctionFragmentConditionalBranchEvidence, FunctionFragmentInstructionSpan,
    FunctionFragmentInternalMachineFixup, FunctionFragmentInternalMachineFixupState,
    FunctionFragmentNormalizedForeignCallFixup, FunctionFragmentNormalizedForeignCallFixupKind,
    FunctionFragmentNormalizedForeignCallFixupState,
};
use selected_instructions::{SelectedBlock, SelectedInstruction, SelectedInstructionKind};

use machine_code::{
    ResolvedSelectedFormRow, SelectedFormInternalMachineFixupState,
    SelectedFormNormalizedForeignCallFixupKind, SelectedFormNormalizedForeignCallFixupState,
};

use super::control;
use crate::fragment_emission::projection::ResolvedFragmentEmissionError;

pub(super) fn emit(
    function: &selected_instructions::SelectedFunction,
    block: &SelectedBlock,
    instruction: &SelectedInstruction,
    row: &ResolvedSelectedFormRow,
    bytes: &mut Vec<u8>,
) -> Result<FunctionFragmentInstructionSpan, ResolvedFragmentEmissionError> {
    let row_offset =
        u64::try_from(bytes.len()).map_err(|_| ResolvedFragmentEmissionError::OffsetOverflow)?;
    if row_offset != row.offset {
        return Err(ResolvedFragmentEmissionError::RootMismatch);
    }
    if instruction.id != row.instruction {
        return Err(ResolvedFragmentEmissionError::MissingInstruction(
            row.instruction,
        ));
    }
    let control = control::provenance(function, block, instruction)?;
    let internal_machine_fixup = translate_fixup(row, instruction)?;
    let import_free = if let SelectedInstructionKind::NormalizedForeignCall { boundary, ordinal } =
        instruction.kind
    {
        let call = function
            .normalized_foreign_calls
            .get(ordinal as usize)
            .filter(|call| call.instruction == instruction.id && call.call.boundary == boundary)
            .ok_or(ResolvedFragmentEmissionError::RootMismatch)?;
        call.call.binding.import().is_none()
    } else {
        false
    };
    let normalized_foreign_call_fixup = translate_foreign_fixup(row, instruction, import_free)?;
    let image_address_fixup = translate_byte_literal_fixup(row, instruction)?;
    bytes.extend_from_slice(&row.bytes);
    Ok(FunctionFragmentInstructionSpan {
        instruction: row.instruction,
        alternative: row.alternative,
        offset: row.offset,
        bytes: row.bytes.clone(),
        branch: row.branch.as_deref().map(|branch| {
            let branch = match branch {
                machine_code::ResolvedBranchEvidence::Conditional(branch) => branch,
                machine_code::ResolvedBranchEvidence::Jump(jump) => {
                    return Box::new(machine_code::FunctionFragmentBranchEvidence::Jump(
                        jump.clone(),
                    ));
                }
            };
            Box::new(machine_code::FunctionFragmentBranchEvidence::Conditional(
                FunctionFragmentConditionalBranchEvidence {
                    predicate: branch.predicate,
                    source_block: branch.source_block,
                    when_taken_edge: branch.when_taken_edge,
                    when_taken_block: branch.when_taken_block,
                    when_taken_offset: branch.when_taken_offset,
                    when_fallthrough_edge: branch.when_fallthrough_edge,
                    when_fallthrough_block: branch.when_fallthrough_block,
                    when_fallthrough_offset: branch.when_fallthrough_offset,
                    byte_displacement: branch.byte_displacement,
                    decoded_register_reads: branch.decoded_register_reads.clone(),
                    decoded_effects: branch.decoded_effects.clone(),
                },
            ))
        }),
        internal_machine_fixup,
        normalized_foreign_call_fixup,
        image_address_fixup,
        provenance: instruction.provenance.clone(),
        control,
    })
}

/// A row-relative literal address field becomes function-relative; the
/// field itself stays zero for object-level symbol binding.
fn translate_byte_literal_fixup(
    row: &ResolvedSelectedFormRow,
    instruction: &SelectedInstruction,
) -> Result<Option<machine_code::FunctionFragmentImageAddressFixup>, ResolvedFragmentEmissionError>
{
    let reference = match instruction.kind {
        SelectedInstructionKind::ImmutableImageAddress { reference } => Some(reference),
        _ => None,
    };
    match (row.image_address_fixup, reference) {
        (None, None) => Ok(None),
        (Some(fixup), Some(reference)) if fixup.reference == reference && row.branch.is_none() => {
            Ok(Some(machine_code::FunctionFragmentImageAddressFixup {
                reference,
                fields: fixup
                    .fields
                    .shifted(row.offset)
                    .ok_or(ResolvedFragmentEmissionError::OffsetOverflow)?,
            }))
        }
        _ => Err(ResolvedFragmentEmissionError::RootMismatch),
    }
}

fn translate_fixup(
    row: &ResolvedSelectedFormRow,
    instruction: &SelectedInstruction,
) -> Result<Option<FunctionFragmentInternalMachineFixup>, ResolvedFragmentEmissionError> {
    let Some(fixup) = row.internal_machine_fixup else {
        if instruction.kind.machine_callee().is_some() {
            return Err(ResolvedFragmentEmissionError::RootMismatch);
        }
        return Ok(None);
    };
    let callee = instruction
        .kind
        .machine_callee()
        .ok_or(ResolvedFragmentEmissionError::RootMismatch)?;
    if fixup.state != SelectedFormInternalMachineFixupState::UnresolvedZeroFieldV1
        || fixup.callee != callee
        || row.branch.is_some()
    {
        return Err(ResolvedFragmentEmissionError::RootMismatch);
    }
    let translate = |offset: u16| {
        row.offset
            .checked_add(u64::from(offset))
            .ok_or(ResolvedFragmentEmissionError::OffsetOverflow)
    };
    Ok(Some(FunctionFragmentInternalMachineFixup {
        kind: fixup.kind,
        state: FunctionFragmentInternalMachineFixupState::UnresolvedZeroFieldV1,
        callee,
        opcode_function_offset: translate(fixup.opcode_row_offset)?,
        patch_function_offset: translate(fixup.patch_row_offset)?,
        reference_function_offset: translate(fixup.reference_row_offset)?,
        patch_byte_width: fixup.patch_byte_width,
        addend: fixup.addend,
    }))
}

/// Row-relative foreign fixups become function-relative custody; the field
/// itself stays unresolved for object-level import binding.
fn translate_foreign_fixup(
    row: &ResolvedSelectedFormRow,
    instruction: &SelectedInstruction,
    import_free: bool,
) -> Result<Option<FunctionFragmentNormalizedForeignCallFixup>, ResolvedFragmentEmissionError> {
    // The retained mechanism, not the common call opcode, owns relocation.
    // Kernel and table calls are fully encoded before layout; only imports
    // introduce a linker field.
    if import_free {
        if row.normalized_foreign_call_fixup.is_some()
            || row.internal_machine_fixup.is_some()
            || row.branch.is_some()
        {
            return Err(ResolvedFragmentEmissionError::RootMismatch);
        }
        return Ok(None);
    }
    let Some(fixup) = row.normalized_foreign_call_fixup.clone() else {
        if matches!(
            instruction.kind,
            SelectedInstructionKind::NormalizedForeignCall { .. }
        ) {
            return Err(ResolvedFragmentEmissionError::RootMismatch);
        }
        return Ok(None);
    };
    let SelectedInstructionKind::NormalizedForeignCall { boundary, ordinal } = instruction.kind
    else {
        return Err(ResolvedFragmentEmissionError::RootMismatch);
    };
    if fixup.state != SelectedFormNormalizedForeignCallFixupState::UnresolvedImportFieldV1
        || fixup.boundary != boundary
        || fixup.ordinal != ordinal
        || row.branch.is_some()
        || row.internal_machine_fixup.is_some()
    {
        return Err(ResolvedFragmentEmissionError::RootMismatch);
    }
    let translate = |offset: u16| {
        row.offset
            .checked_add(u64::from(offset))
            .ok_or(ResolvedFragmentEmissionError::OffsetOverflow)
    };
    Ok(Some(FunctionFragmentNormalizedForeignCallFixup {
        kind: match fixup.kind {
            SelectedFormNormalizedForeignCallFixupKind::X86Relative32FromNextInstructionToNormalizedForeignImportV1 => FunctionFragmentNormalizedForeignCallFixupKind::X86Relative32FromNextInstructionToNormalizedForeignImportV1,
            SelectedFormNormalizedForeignCallFixupKind::Aarch64BranchLinkImmediate26FromInstructionToNormalizedForeignImportV1 => FunctionFragmentNormalizedForeignCallFixupKind::Aarch64BranchLinkImmediate26FromInstructionToNormalizedForeignImportV1,
        },
        state: FunctionFragmentNormalizedForeignCallFixupState::UnresolvedImportFieldV1,
        boundary,
        ordinal,
        opcode_function_offset: translate(fixup.opcode_row_offset)?,
        patch_function_offset: translate(fixup.patch_row_offset)?,
        reference_function_offset: translate(fixup.reference_row_offset)?,
        patch_byte_width: fixup.patch_byte_width,
        addend: fixup.addend,
        callbacks: fixup
            .callbacks
            .iter()
            .map(|callback| {
                Ok(machine_code::FunctionFragmentCallbackMaterialization {
                    destination: callback.destination,
                    code_function_offset: translate(callback.code_row_offset)?,
                    code_byte_count: u64::from(callback.code_byte_count),
                    encoding: match callback.encoding {
                        machine_code::SelectedFormCallbackMaterializationEncoding::X86_64Relative32 {
                            patch_row_offset,
                        } => machine_code::FunctionFragmentCallbackMaterializationEncoding::X86_64Relative32 {
                            patch_function_offset: translate(patch_row_offset)?,
                        },
                        machine_code::SelectedFormCallbackMaterializationEncoding::Aarch64PageAddress {
                            page_patch_row_offset,
                            page_offset_patch_row_offset,
                        } => machine_code::FunctionFragmentCallbackMaterializationEncoding::Aarch64PageAddress {
                            page_patch_function_offset: translate(page_patch_row_offset)?,
                            page_offset_patch_function_offset: translate(
                                page_offset_patch_row_offset,
                            )?,
                        },
                    },
                })
            })
            .collect::<Result<Vec<_>, _>>()?,
    }))
}

#[cfg(test)]
mod tests {
    use super::{
        FunctionFragmentInternalMachineFixupState, ResolvedSelectedFormRow,
        SelectedFormInternalMachineFixupState, SelectedInstruction, SelectedInstructionKind,
        translate_fixup,
    };
    use machine_code::{SelectedFormInternalMachineFixup, SelectedFormInternalMachineFixupKind};
    use register_model::{RegisterConstraintFamily, RegisterConstraintKey};
    use selected_instructions::{
        MachineAlternativeFamily, MachineAlternativeKey, SelectedInstructionId,
        SelectedInstructionProvenance,
    };
    use semantic_vocabulary::MachineId;

    fn selected_call(callee: MachineId) -> SelectedInstruction {
        SelectedInstruction {
            id: SelectedInstructionId(17),
            kind: SelectedInstructionKind::CallScalar {
                target: selected_instructions::SelectedCallTarget::Machine(callee),
            },
            constraint: RegisterConstraintKey {
                family: RegisterConstraintFamily::Call,
                variant: 0,
            },
            operands: Vec::new(),
            implicit_uses: Vec::new(),
            implicit_defs: Vec::new(),
            clobbers: Vec::new(),
            provenance: SelectedInstructionProvenance::default(),
        }
    }

    #[test]
    fn system_call_projection_has_no_import_fixup_but_imports_still_require_one() {
        let mut instruction = selected_call(MachineId::new(29).unwrap());
        instruction.kind = SelectedInstructionKind::NormalizedForeignCall {
            boundary: semantic_vocabulary::BoundaryMachineId::new(3).unwrap(),
            ordinal: 0,
        };
        instruction.constraint = isa_x86_64::x86_64_linux_system_call_keys()[0];
        let row = ResolvedSelectedFormRow {
            instruction: instruction.id,
            alternative: MachineAlternativeKey {
                family: MachineAlternativeFamily::NormalizedForeignCall,
                variant: 0,
            },
            offset: 37,
            bytes: Vec::new(),
            branch: None,
            internal_machine_fixup: None,
            normalized_foreign_call_fixup: None,
            image_address_fixup: None,
        };
        assert_eq!(
            super::translate_foreign_fixup(&row, &instruction, true).unwrap(),
            None
        );
        instruction.constraint.family = RegisterConstraintFamily::Call;
        assert!(super::translate_foreign_fixup(&row, &instruction, false).is_err());
    }

    fn row(bytes: Vec<u8>, fixup: SelectedFormInternalMachineFixup) -> ResolvedSelectedFormRow {
        ResolvedSelectedFormRow {
            instruction: SelectedInstructionId(17),
            alternative: MachineAlternativeKey {
                family: MachineAlternativeFamily::CallScalar,
                variant: 0,
            },
            offset: 37,
            bytes,
            branch: None,
            internal_machine_fixup: Some(fixup),
            normalized_foreign_call_fixup: None,
            image_address_fixup: None,
        }
    }

    #[test]
    fn row_relative_call_fixups_become_function_relative_without_resolution() {
        let callee = MachineId::new(29).unwrap();
        for (kind, bytes, opcode, patch, reference) in [
            (
                SelectedFormInternalMachineFixupKind::X86Relative32FromNextInstructionToInternalMachineV1,
                vec![0xe8, 0, 0, 0, 0],
                0,
                1,
                5,
            ),
            (
                SelectedFormInternalMachineFixupKind::Aarch64BranchLinkImmediate26FromInstructionToInternalMachineV1,
                0x9400_0000_u32.to_le_bytes().to_vec(),
                0,
                0,
                0,
            ),
        ] {
            let selected = selected_call(callee);
            let translated = translate_fixup(
                &row(
                    bytes,
                    SelectedFormInternalMachineFixup {
                        kind,
                        state: SelectedFormInternalMachineFixupState::UnresolvedZeroFieldV1,
                        callee,
                        opcode_row_offset: opcode,
                        patch_row_offset: patch,
                        reference_row_offset: reference,
                        patch_byte_width: 4,
                        addend: 0,
                    },
                ),
                &selected,
            )
            .unwrap()
            .unwrap();
            assert_eq!(translated.callee, callee);
            assert_eq!(translated.opcode_function_offset, 37 + u64::from(opcode));
            assert_eq!(translated.patch_function_offset, 37 + u64::from(patch));
            assert_eq!(
                translated.reference_function_offset,
                37 + u64::from(reference)
            );
            assert_eq!(
                translated.state,
                FunctionFragmentInternalMachineFixupState::UnresolvedZeroFieldV1
            );
        }
    }

    #[test]
    fn call_fixup_translation_rejects_detached_callee_and_missing_custody() {
        let callee = MachineId::new(29).unwrap();
        let selected = selected_call(callee);
        let detached = row(
            vec![0xe8, 0, 0, 0, 0],
            SelectedFormInternalMachineFixup {
                kind: SelectedFormInternalMachineFixupKind::X86Relative32FromNextInstructionToInternalMachineV1,
                state: SelectedFormInternalMachineFixupState::UnresolvedZeroFieldV1,
                callee: MachineId::new(30).unwrap(),
                opcode_row_offset: 0,
                patch_row_offset: 1,
                reference_row_offset: 5,
                patch_byte_width: 4,
                addend: 0,
            },
        );
        assert!(translate_fixup(&detached, &selected).is_err());
        let mut missing = detached;
        missing.internal_machine_fixup = None;
        assert!(translate_fixup(&missing, &selected).is_err());
    }
}
