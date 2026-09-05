use std::collections::BTreeMap;

use omega_machine_code::{
    FunctionFragment, FunctionFragmentControlProvenance, FunctionFragmentEmissionPlan,
    FunctionFragmentInternalMachineFixup,
};
use omega_register_model::ValidatedPhysicalRegisterModel;

use crate::{TargetFrameProtocolEncodingPlan, validate_optimized_function_fragment_emission};

use super::{
    FunctionAppliedFrameEpilogue, FunctionAppliedFrameProtocol,
    FunctionFragmentFrameApplicationError, FunctionFragmentFrameApplicationReceipt,
    StagedFunctionFragmentFrameApplication,
};

pub(super) fn validate(
    staged: &StagedFunctionFragmentFrameApplication,
) -> Result<FunctionFragmentFrameApplicationReceipt, FunctionFragmentFrameApplicationError> {
    validate_optimized_function_fragment_emission(&staged.source)
        .map_err(FunctionFragmentFrameApplicationError::Source)?;
    let admitted_protocol = staged
        .source
        .source()
        .frame_protocol()
        .ok_or(FunctionFragmentFrameApplicationError::SourceKindMismatch)?;
    let source = staged.source.fragments();
    let protocol = admitted_protocol.plan();
    let candidate = &staged.application;
    if candidate.source_fragment_manifest != staged.source.manifest().record().identity
        || candidate.source_fragments != source.identity
        || candidate.frame_protocol != admitted_protocol.receipt().identity()
        || candidate.functions.len() != source.functions.len()
        || candidate.fragments.identity != candidate.fragments.recomputed_identity()
        || candidate.identity != candidate.recomputed_identity()
        || !same_plan_roots(source, &candidate.fragments)
    {
        return Err(FunctionFragmentFrameApplicationError::ArtifactMismatch);
    }
    let physical = staged.source.source().register_environment().physical();
    for ((source_function, candidate_function), application) in source
        .functions
        .iter()
        .zip(&candidate.fragments.functions)
        .zip(&candidate.functions)
    {
        let protocol_row = unique_protocol(protocol, source_function.machine)?;
        let prologue = protocol_row.prologue.bytes(&protocol.bytes).ok_or(
            FunctionFragmentFrameApplicationError::InvalidProtocolSpan(source_function.machine),
        )?;
        let epilogue = protocol_row.epilogue.bytes(&protocol.bytes).ok_or(
            FunctionFragmentFrameApplicationError::InvalidProtocolSpan(source_function.machine),
        )?;
        validate_function(
            source_function,
            candidate_function,
            application,
            prologue,
            epilogue,
            source.target.architecture,
            physical,
        )?;
    }
    if protocol.functions.len() != source.functions.len() {
        return Err(FunctionFragmentFrameApplicationError::FunctionRosterMismatch);
    }
    let receipt = super::model::seal(candidate);
    if staged.receipt != receipt {
        return Err(FunctionFragmentFrameApplicationError::ReceiptMismatch);
    }
    Ok(receipt)
}

fn same_plan_roots(
    source: &FunctionFragmentEmissionPlan,
    candidate: &FunctionFragmentEmissionPlan,
) -> bool {
    source.psi == candidate.psi
        && source.fuel_schedule == candidate.fuel_schedule
        && source.selected == candidate.selected
        && source.target == candidate.target
        && source.entry == candidate.entry
        && source.structural_unit_functions == candidate.structural_unit_functions
        && source.functions.len() == candidate.functions.len()
}

fn unique_protocol(
    protocol: &TargetFrameProtocolEncodingPlan,
    machine: psi_core::MachineId,
) -> Result<&crate::FunctionTargetFrameProtocolEncoding, FunctionFragmentFrameApplicationError> {
    let mut rows = protocol
        .functions
        .iter()
        .filter(|row| row.machine == machine);
    let row = rows
        .next()
        .ok_or(FunctionFragmentFrameApplicationError::MissingFunction(
            machine,
        ))?;
    if rows.next().is_some() {
        return Err(FunctionFragmentFrameApplicationError::FunctionRosterMismatch);
    }
    Ok(row)
}

#[allow(clippy::too_many_arguments)]
fn validate_function(
    source: &FunctionFragment,
    candidate: &FunctionFragment,
    application: &FunctionAppliedFrameProtocol,
    prologue: &[u8],
    epilogue: &[u8],
    architecture: omega_target::Architecture,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<(), FunctionFragmentFrameApplicationError> {
    if prologue.is_empty() != epilogue.is_empty()
        || source.machine != candidate.machine
        || source.machine != application.machine
        || source.attachment != candidate.attachment
        || source.provenance != candidate.provenance
        || source.blocks.len() != candidate.blocks.len()
        || application.prologue_function_offset != 0
        || application.prologue_byte_count != prologue.len() as u64
        || candidate.byte_count != candidate.bytes.len() as u64
        || candidate.bytes.get(..prologue.len()) != Some(prologue)
    {
        return Err(FunctionFragmentFrameApplicationError::ArtifactMismatch);
    }
    let block_offsets = candidate
        .blocks
        .iter()
        .map(|block| (block.block, block.offset))
        .collect::<BTreeMap<_, _>>();
    let mut expected_epilogues = Vec::new();
    let mut cursor = prologue.len() as u64;
    for (source_block, candidate_block) in source.blocks.iter().zip(&candidate.blocks) {
        if source_block.block != candidate_block.block
            || source_block.instructions.len() != candidate_block.instructions.len()
            || candidate_block.offset != cursor
        {
            return Err(FunctionFragmentFrameApplicationError::ArtifactMismatch);
        }
        let block_start = cursor;
        for (index, (source_row, candidate_row)) in source_block
            .instructions
            .iter()
            .zip(&candidate_block.instructions)
            .enumerate()
        {
            if source_row.instruction != candidate_row.instruction
                || source_row.alternative != candidate_row.alternative
                || source_row.provenance != candidate_row.provenance
                || source_row.control != candidate_row.control
            {
                return Err(FunctionFragmentFrameApplicationError::ArtifactMismatch);
            }
            if let FunctionFragmentControlProvenance::Return { psi_return_edge } =
                source_row.control
            {
                if index + 1 != source_block.instructions.len() {
                    return Err(FunctionFragmentFrameApplicationError::MissingFinalReturn(
                        source.machine,
                    ));
                }
                check_bytes(&candidate.bytes, cursor, epilogue)?;
                expected_epilogues.push(FunctionAppliedFrameEpilogue {
                    block: source_block.block,
                    return_instruction: source_row.instruction,
                    psi_return_edge,
                    function_offset: cursor,
                    byte_count: epilogue.len() as u64,
                });
                cursor = advance(cursor, epilogue.len())?;
            }
            if candidate_row.offset != cursor {
                return Err(FunctionFragmentFrameApplicationError::ArtifactMismatch);
            }
            check_bytes(&candidate.bytes, cursor, &candidate_row.bytes)?;
            if source_row.branch.is_some() {
                super::validation_branch::validate(
                    source_row,
                    candidate_row,
                    architecture,
                    physical,
                    &block_offsets,
                )?;
            } else if candidate_row.branch.is_some() || candidate_row.bytes != source_row.bytes {
                return Err(FunctionFragmentFrameApplicationError::ArtifactMismatch);
            }
            let shift = candidate_row
                .offset
                .checked_sub(source_row.offset)
                .ok_or(FunctionFragmentFrameApplicationError::ArtifactMismatch)?;
            validate_fixup(
                source_row.internal_machine_fixup,
                candidate_row.internal_machine_fixup,
                shift,
            )?;
            cursor = advance(cursor, candidate_row.bytes.len())?;
        }
        if candidate_block.byte_count != cursor - block_start {
            return Err(FunctionFragmentFrameApplicationError::ArtifactMismatch);
        }
    }
    if expected_epilogues.is_empty()
        || application.epilogues != expected_epilogues
        || cursor != candidate.byte_count
    {
        return Err(FunctionFragmentFrameApplicationError::ArtifactMismatch);
    }
    Ok(())
}

fn validate_fixup(
    source: Option<FunctionFragmentInternalMachineFixup>,
    candidate: Option<FunctionFragmentInternalMachineFixup>,
    shift: u64,
) -> Result<(), FunctionFragmentFrameApplicationError> {
    let Some(mut expected) = source else {
        return if candidate.is_none() {
            Ok(())
        } else {
            Err(FunctionFragmentFrameApplicationError::ArtifactMismatch)
        };
    };
    expected.opcode_function_offset = expected
        .opcode_function_offset
        .checked_add(shift)
        .ok_or(FunctionFragmentFrameApplicationError::OffsetOverflow)?;
    expected.patch_function_offset = expected
        .patch_function_offset
        .checked_add(shift)
        .ok_or(FunctionFragmentFrameApplicationError::OffsetOverflow)?;
    expected.reference_function_offset = expected
        .reference_function_offset
        .checked_add(shift)
        .ok_or(FunctionFragmentFrameApplicationError::OffsetOverflow)?;
    if candidate != Some(expected) {
        return Err(FunctionFragmentFrameApplicationError::ArtifactMismatch);
    }
    Ok(())
}

fn check_bytes(
    bytes: &[u8],
    offset: u64,
    expected: &[u8],
) -> Result<(), FunctionFragmentFrameApplicationError> {
    let start = usize::try_from(offset)
        .map_err(|_| FunctionFragmentFrameApplicationError::OffsetOverflow)?;
    let end = start
        .checked_add(expected.len())
        .ok_or(FunctionFragmentFrameApplicationError::OffsetOverflow)?;
    if bytes.get(start..end) != Some(expected) {
        return Err(FunctionFragmentFrameApplicationError::ArtifactMismatch);
    }
    Ok(())
}

fn advance(offset: u64, length: usize) -> Result<u64, FunctionFragmentFrameApplicationError> {
    offset
        .checked_add(
            u64::try_from(length)
                .map_err(|_| FunctionFragmentFrameApplicationError::OffsetOverflow)?,
        )
        .ok_or(FunctionFragmentFrameApplicationError::OffsetOverflow)
}
