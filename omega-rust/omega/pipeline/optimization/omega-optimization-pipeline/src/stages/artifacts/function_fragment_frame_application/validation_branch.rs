use omega_isa_aarch64::{
    validate_aarch64_fused_compare_i64_zero_branch_nonzero_to_cbnz_form,
    validate_aarch64_selected_i64_less_than_branch_form,
    validate_aarch64_selected_nonzero_branch_form,
    validate_aarch64_selected_u64_less_than_branch_form,
};
use omega_isa_x86_64::{
    validate_x86_64_selected_i64_less_than_branch_form,
    validate_x86_64_selected_nonzero_branch_form,
    validate_x86_64_selected_short_nonzero_branch_form,
    validate_x86_64_selected_u64_less_than_branch_form,
};
use omega_machine_code::{
    FunctionFragmentConditionalBranchPredicate, FunctionFragmentInstructionSpan,
};
use omega_register_model::ValidatedPhysicalRegisterModel;
use omega_selected_instructions::SelectedBlockId;
use omega_target::Architecture;

use super::FunctionFragmentFrameApplicationError;

pub(super) fn validate(
    source: &FunctionFragmentInstructionSpan,
    candidate: &FunctionFragmentInstructionSpan,
    architecture: Architecture,
    physical: &ValidatedPhysicalRegisterModel,
    block_offsets: &std::collections::BTreeMap<SelectedBlockId, u64>,
) -> Result<(), FunctionFragmentFrameApplicationError> {
    let (Some(source_branch), Some(branch)) =
        (source.branch.as_deref(), candidate.branch.as_deref())
    else {
        return Err(FunctionFragmentFrameApplicationError::ArtifactMismatch);
    };
    if source_branch.predicate != branch.predicate
        || source_branch.source_block != branch.source_block
        || source_branch.when_taken_edge != branch.when_taken_edge
        || source_branch.when_taken_block != branch.when_taken_block
        || source_branch.when_fallthrough_edge != branch.when_fallthrough_edge
        || source_branch.when_fallthrough_block != branch.when_fallthrough_block
        || source_branch.decoded_register_reads != branch.decoded_register_reads
        || source_branch.decoded_effects != branch.decoded_effects
    {
        return Err(FunctionFragmentFrameApplicationError::ArtifactMismatch);
    }
    let taken = block_offsets.get(&branch.when_taken_block).copied().ok_or(
        FunctionFragmentFrameApplicationError::MissingTargetBlock(branch.when_taken_block),
    )?;
    let fallthrough = block_offsets
        .get(&branch.when_fallthrough_block)
        .copied()
        .ok_or(FunctionFragmentFrameApplicationError::MissingTargetBlock(
            branch.when_fallthrough_block,
        ))?;
    let end = candidate
        .offset
        .checked_add(
            u64::try_from(candidate.bytes.len())
                .map_err(|_| FunctionFragmentFrameApplicationError::OffsetOverflow)?,
        )
        .ok_or(FunctionFragmentFrameApplicationError::OffsetOverflow)?;
    if fallthrough != end {
        return Err(
            FunctionFragmentFrameApplicationError::BranchFallthroughMismatch(candidate.instruction),
        );
    }
    let reference = match architecture {
        Architecture::X86_64 => end,
        Architecture::Aarch64 => candidate.offset,
    };
    let displacement = i64::try_from(i128::from(taken) - i128::from(reference))
        .map_err(|_| FunctionFragmentFrameApplicationError::OffsetOverflow)?;
    if branch.when_taken_offset != taken
        || branch.when_fallthrough_offset != fallthrough
        || branch.byte_displacement != displacement
    {
        return Err(FunctionFragmentFrameApplicationError::ArtifactMismatch);
    }
    let (reads, effects) = decode(candidate, branch.predicate, architecture, physical)?;
    if reads != branch.decoded_register_reads || effects != branch.decoded_effects {
        return Err(
            FunctionFragmentFrameApplicationError::BranchEffectsMismatch(candidate.instruction),
        );
    }
    Ok(())
}

fn decode(
    row: &FunctionFragmentInstructionSpan,
    predicate: FunctionFragmentConditionalBranchPredicate,
    architecture: Architecture,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<
    (
        Vec<omega_register_model::RegisterViewId>,
        omega_selected_instructions::MachineEncodedEffects,
    ),
    FunctionFragmentFrameApplicationError,
> {
    let instruction = row.instruction;
    let branch = row.branch.as_deref().unwrap();
    macro_rules! footprint {
        ($encoded:expr) => {{
            let encoded = $encoded;
            (
                encoded.footprint().register_reads.clone(),
                encoded.footprint().encoded.clone(),
            )
        }};
    }
    let decoded = match (architecture, predicate) {
        (Architecture::X86_64, FunctionFragmentConditionalBranchPredicate::NonZeroV1) => {
            let decoded = match row.bytes.len() {
                2 => validate_x86_64_selected_short_nonzero_branch_form(
                    physical,
                    row.alternative,
                    branch.byte_displacement,
                    &row.bytes,
                ),
                _ => validate_x86_64_selected_nonzero_branch_form(
                    physical,
                    row.alternative,
                    branch.byte_displacement,
                    &row.bytes,
                ),
            }
            .map_err(|error| {
                FunctionFragmentFrameApplicationError::X86_64Branch(instruction, error)
            })?;
            footprint!(decoded)
        }
        (Architecture::X86_64, FunctionFragmentConditionalBranchPredicate::U64LessThanV1) => {
            let decoded = validate_x86_64_selected_u64_less_than_branch_form(
                physical,
                row.alternative,
                branch.byte_displacement,
                &row.bytes,
            )
            .map_err(|error| {
                FunctionFragmentFrameApplicationError::X86_64Branch(instruction, error)
            })?;
            footprint!(decoded)
        }
        (Architecture::X86_64, FunctionFragmentConditionalBranchPredicate::I64LessThanV1) => {
            let decoded = validate_x86_64_selected_i64_less_than_branch_form(
                physical,
                row.alternative,
                branch.byte_displacement,
                &row.bytes,
            )
            .map_err(|error| {
                FunctionFragmentFrameApplicationError::X86_64Branch(instruction, error)
            })?;
            footprint!(decoded)
        }
        (Architecture::Aarch64, FunctionFragmentConditionalBranchPredicate::NonZeroV1)
            if branch.decoded_register_reads.len() == 1 =>
        {
            let decoded = validate_aarch64_fused_compare_i64_zero_branch_nonzero_to_cbnz_form(
                physical,
                branch.decoded_register_reads[0],
                branch.byte_displacement,
                &row.bytes,
            )
            .map_err(|error| {
                FunctionFragmentFrameApplicationError::Aarch64Branch(instruction, error)
            })?;
            footprint!(decoded)
        }
        (Architecture::Aarch64, FunctionFragmentConditionalBranchPredicate::NonZeroV1) => {
            let decoded = validate_aarch64_selected_nonzero_branch_form(
                physical,
                row.alternative,
                branch.byte_displacement,
                &row.bytes,
            )
            .map_err(|error| {
                FunctionFragmentFrameApplicationError::Aarch64Branch(instruction, error)
            })?;
            footprint!(decoded)
        }
        (Architecture::Aarch64, FunctionFragmentConditionalBranchPredicate::U64LessThanV1) => {
            let decoded = validate_aarch64_selected_u64_less_than_branch_form(
                physical,
                row.alternative,
                branch.byte_displacement,
                &row.bytes,
            )
            .map_err(|error| {
                FunctionFragmentFrameApplicationError::Aarch64Branch(instruction, error)
            })?;
            footprint!(decoded)
        }
        (Architecture::Aarch64, FunctionFragmentConditionalBranchPredicate::I64LessThanV1) => {
            let decoded = validate_aarch64_selected_i64_less_than_branch_form(
                physical,
                row.alternative,
                branch.byte_displacement,
                &row.bytes,
            )
            .map_err(|error| {
                FunctionFragmentFrameApplicationError::Aarch64Branch(instruction, error)
            })?;
            footprint!(decoded)
        }
    };
    Ok(decoded)
}
