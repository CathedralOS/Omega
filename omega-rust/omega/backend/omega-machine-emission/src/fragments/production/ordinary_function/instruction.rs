use omega_machine_code::{
    FunctionFragmentConditionalBranchEvidence, FunctionFragmentConditionalBranchPredicate,
    FunctionFragmentInstructionSpan, FunctionFragmentInternalMachineFixup,
    FunctionFragmentInternalMachineFixupKind, FunctionFragmentInternalMachineFixupState,
};
use omega_selected_instructions::{
    SelectedBlock, SelectedInstruction, SelectedInstructionKind, SelectedTerminator,
};

use omega_machine_code::{
    ResolvedSelectedFormRow, SelectedFormInternalMachineFixupKind,
    SelectedFormInternalMachineFixupState,
};

use super::control;
use crate::fragments::ResolvedFragmentEmissionError;

pub(super) fn emit(
    block: &SelectedBlock,
    row: &ResolvedSelectedFormRow,
    bytes: &mut Vec<u8>,
) -> Result<FunctionFragmentInstructionSpan, ResolvedFragmentEmissionError> {
    let row_offset =
        u64::try_from(bytes.len()).map_err(|_| ResolvedFragmentEmissionError::OffsetOverflow)?;
    if row_offset != row.offset {
        return Err(ResolvedFragmentEmissionError::RootMismatch);
    }
    let instruction = selected(block, row)?;
    let control = control::provenance(block, instruction);
    let internal_machine_fixup = translate_fixup(row, instruction)?;
    bytes.extend_from_slice(&row.bytes);
    Ok(FunctionFragmentInstructionSpan {
        instruction: row.instruction,
        alternative: row.alternative,
        offset: row.offset,
        bytes: row.bytes.clone(),
        branch: row.branch.as_deref().map(|branch| {
            Box::new(FunctionFragmentConditionalBranchEvidence {
                predicate: match branch.predicate {
                    omega_machine_code::ResolvedConditionalBranchPredicate::NonZeroV1 => {
                        FunctionFragmentConditionalBranchPredicate::NonZeroV1
                    }
                    omega_machine_code::ResolvedConditionalBranchPredicate::U64LessThanV1 => {
                        FunctionFragmentConditionalBranchPredicate::U64LessThanV1
                    }
                    omega_machine_code::ResolvedConditionalBranchPredicate::I64LessThanV1 => {
                        FunctionFragmentConditionalBranchPredicate::I64LessThanV1
                    }
                },
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
            })
        }),
        internal_machine_fixup,
        provenance: instruction.provenance.clone(),
        control,
    })
}

fn translate_fixup(
    row: &ResolvedSelectedFormRow,
    instruction: &SelectedInstruction,
) -> Result<Option<FunctionFragmentInternalMachineFixup>, ResolvedFragmentEmissionError> {
    let Some(fixup) = row.internal_machine_fixup else {
        if matches!(instruction.kind, SelectedInstructionKind::CallI64 { .. }) {
            return Err(ResolvedFragmentEmissionError::RootMismatch);
        }
        return Ok(None);
    };
    let SelectedInstructionKind::CallI64 { callee } = instruction.kind else {
        return Err(ResolvedFragmentEmissionError::RootMismatch);
    };
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
        kind: match fixup.kind {
            SelectedFormInternalMachineFixupKind::X86Relative32FromNextInstructionToInternalMachineV1 => FunctionFragmentInternalMachineFixupKind::X86Relative32FromNextInstructionToInternalMachineV1,
            SelectedFormInternalMachineFixupKind::Aarch64BranchLinkImmediate26FromInstructionToInternalMachineV1 => FunctionFragmentInternalMachineFixupKind::Aarch64BranchLinkImmediate26FromInstructionToInternalMachineV1,
        },
        state: FunctionFragmentInternalMachineFixupState::UnresolvedZeroFieldV1,
        callee,
        opcode_function_offset: translate(fixup.opcode_row_offset)?,
        patch_function_offset: translate(fixup.patch_row_offset)?,
        reference_function_offset: translate(fixup.reference_row_offset)?,
        patch_byte_width: fixup.patch_byte_width,
        addend: fixup.addend,
    }))
}

fn selected<'a>(
    block: &'a SelectedBlock,
    row: &ResolvedSelectedFormRow,
) -> Result<&'a SelectedInstruction, ResolvedFragmentEmissionError> {
    block
        .instructions
        .iter()
        .chain(std::iter::once(match &block.terminator {
            SelectedTerminator::ConditionalBranch { instruction, .. }
            | SelectedTerminator::ConditionalBranchU64LessThan { instruction, .. }
            | SelectedTerminator::ConditionalBranchI64LessThan { instruction, .. }
            | SelectedTerminator::Return { instruction, .. } => instruction,
        }))
        .find(|instruction| instruction.id == row.instruction)
        .ok_or(ResolvedFragmentEmissionError::MissingInstruction(
            row.instruction,
        ))
}

#[cfg(test)]
mod tests {
    use omega_register_model::{RegisterConstraintFamily, RegisterConstraintKey};
    use omega_selected_instructions::{
        MachineAlternativeFamily, MachineAlternativeKey, SelectedInstructionId,
        SelectedInstructionProvenance,
    };
    use psi_core::MachineId;

    use super::*;
    use omega_machine_code::{
        SelectedFormInternalMachineFixup, SelectedFormInternalMachineFixupKind,
    };

    fn selected_call(callee: MachineId) -> SelectedInstruction {
        SelectedInstruction {
            id: SelectedInstructionId(17),
            kind: SelectedInstructionKind::CallI64 { callee },
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

    fn row(bytes: Vec<u8>, fixup: SelectedFormInternalMachineFixup) -> ResolvedSelectedFormRow {
        ResolvedSelectedFormRow {
            instruction: SelectedInstructionId(17),
            alternative: MachineAlternativeKey {
                family: MachineAlternativeFamily::CallI64,
                variant: 0,
            },
            offset: 37,
            bytes,
            branch: None,
            internal_machine_fixup: Some(fixup),
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
