//! Exact per-function rosters and unchanged bytes for both source forms.
use super::{TextPlacementError, TextPlacementInput, add, bytes, calls, spans, unchanged_bytes};
use omega_machine_code::{FunctionFragmentControlProvenance, RelocationFreeTextSectionPlacement};
use omega_target::Architecture;
use psi_core::MachineId;
use std::collections::BTreeMap;

pub(super) fn source_function(
    offsets: &mut BTreeMap<MachineId, u64>,
    extent: &mut u64,
    machine: MachineId,
    count: u64,
    bytes: &[u8],
    alignment: u64,
) -> Result<(), TextPlacementError> {
    if offsets.insert(machine, *extent).is_some() {
        return Err(TextPlacementError::DuplicateFunction(machine));
    }
    if count != bytes.len() as u64 {
        return Err(TextPlacementError::SourceShapeMismatch);
    }
    if !extent.is_multiple_of(alignment) || !count.is_multiple_of(alignment) {
        return Err(TextPlacementError::MisalignedAarch64Span);
    }
    *extent = add(*extent, count)?;
    Ok(())
}

pub(super) fn check(
    input: TextPlacementInput<'_>,
    section: &RelocationFreeTextSectionPlacement,
    offsets: &BTreeMap<MachineId, u64>,
    alignment: u64,
) -> Result<(), TextPlacementError> {
    let fragments = input.fragments();
    let structural = matches!(input, TextPlacementInput::Structural { .. });
    let mut resolutions = section.resolved_internal_machine_calls.iter();
    if structural {
        for (index, (source, placed)) in fragments
            .structural_unit_functions
            .iter()
            .zip(&section.functions)
            .enumerate()
        {
            let offset = offsets[&source.machine];
            spans::function(placed, index, source.machine, offset, source.byte_count)?;
            let [block] = placed.blocks.as_slice() else {
                return Err(TextPlacementError::ArtifactMismatch);
            };
            spans::block(
                block,
                source.block.block,
                source.block.offset,
                offset,
                source.block.byte_count,
                source.byte_count,
                alignment,
            )?;
            let [returned] = block.instructions.as_slice() else {
                return Err(TextPlacementError::ArtifactMismatch);
            };
            let source_return = &source.block.return_instruction;
            spans::instruction(
                returned,
                source_return.instruction,
                source_return.alternative,
                source_return.offset,
                offset,
                source_return.bytes.len() as u64,
                source.byte_count,
                alignment,
            )?;
            let candidate_bytes = bytes(&section.bytes, offset, source.byte_count)?;
            let mut patches = Vec::new();
            if let Some(call) = &source.block.call {
                let candidate = resolutions
                    .next()
                    .ok_or(TextPlacementError::ArtifactMismatch)?;
                patches.push(calls::check(
                    calls::Call {
                        caller: source.machine,
                        block: source.block.block,
                        instruction: call.instruction,
                        operation: call.operation,
                        callee: call.callee,
                        offset: call.offset,
                        bytes: &call.bytes,
                        fixup: call.fixup,
                    },
                    fragments.target.architecture,
                    offset,
                    offsets,
                    candidate_bytes,
                    candidate,
                )?);
            }
            unchanged_bytes(&source.bytes, candidate_bytes, patches)?;
        }
    } else {
        for (index, (source, placed)) in fragments
            .functions
            .iter()
            .zip(&section.functions)
            .enumerate()
        {
            let offset = offsets[&source.machine];
            spans::function(placed, index, source.machine, offset, source.byte_count)?;
            if placed.blocks.len() != source.blocks.len() {
                return Err(TextPlacementError::ArtifactMismatch);
            }
            let candidate_bytes = bytes(&section.bytes, offset, source.byte_count)?;
            let mut patches = Vec::new();
            for (source_block, placed_block) in source.blocks.iter().zip(&placed.blocks) {
                spans::block(
                    placed_block,
                    source_block.block,
                    source_block.offset,
                    offset,
                    source_block.byte_count,
                    source.byte_count,
                    alignment,
                )?;
                if placed_block.instructions.len() != source_block.instructions.len() {
                    return Err(TextPlacementError::ArtifactMismatch);
                }
                for (source_row, placed_row) in source_block
                    .instructions
                    .iter()
                    .zip(&placed_block.instructions)
                {
                    spans::instruction(
                        placed_row,
                        source_row.instruction,
                        source_row.alternative,
                        source_row.offset,
                        offset,
                        source_row.bytes.len() as u64,
                        source.byte_count,
                        alignment,
                    )?;
                    if let Some(fixup) = source_row.internal_machine_fixup {
                        // Ordinary call rows are a single instruction; structural calls
                        // have their own checked multi-instruction template instead.
                        if fixup.opcode_function_offset != source_row.offset
                            || bytes(
                                &source.bytes,
                                source_row.offset,
                                source_row.bytes.len() as u64,
                            )? != source_row.bytes
                            || (fragments.target.architecture == Architecture::X86_64
                                && source_row.bytes != [0xe8, 0, 0, 0, 0])
                        {
                            return Err(TextPlacementError::SourceShapeMismatch);
                        }
                        if !matches!(input, TextPlacementInput::InternalCalls(_)) {
                            return Err(TextPlacementError::UnsupportedRelocationShape);
                        }
                        let FunctionFragmentControlProvenance::DirectInternalCall { callee } =
                            source_row.control
                        else {
                            return Err(TextPlacementError::SourceShapeMismatch);
                        };
                        let [operation] = source_row.provenance.operations.as_slice() else {
                            return Err(TextPlacementError::SourceShapeMismatch);
                        };
                        let candidate = resolutions
                            .next()
                            .ok_or(TextPlacementError::ArtifactMismatch)?;
                        patches.push(calls::check(
                            calls::Call {
                                caller: source.machine,
                                block: source_block.block,
                                instruction: source_row.instruction,
                                operation: *operation,
                                callee,
                                offset: source_row.offset,
                                bytes: &source_row.bytes,
                                fixup,
                            },
                            fragments.target.architecture,
                            offset,
                            offsets,
                            candidate_bytes,
                            candidate,
                        )?);
                    }
                }
            }
            unchanged_bytes(&source.bytes, candidate_bytes, patches)?;
        }
    }
    if resolutions.next().is_some() {
        return Err(TextPlacementError::ArtifactMismatch);
    }
    Ok(())
}
