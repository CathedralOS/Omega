use isa_aarch64::{
    validate_aarch64_fused_compare_i64_zero_branch_nonzero_to_cbnz_form,
    validate_aarch64_selected_i64_less_than_branch_form,
    validate_aarch64_selected_nonzero_branch_form,
    validate_aarch64_selected_u64_less_than_branch_form,
};
use isa_x86_64::{
    validate_x86_64_selected_i64_less_than_branch_form,
    validate_x86_64_selected_nonzero_branch_form,
    validate_x86_64_selected_short_nonzero_branch_form,
    validate_x86_64_selected_u64_less_than_branch_form,
};
use machine_code::{
    FunctionFragmentBranchEvidence as Branch, FunctionFragmentConditionalBranchPredicate,
    FunctionFragmentInstructionSpan,
};
use register_model::ValidatedPhysicalRegisterModel;
use selected_instructions::SelectedBlockId;
use target::Architecture;

use super::FrameApplicationError;

pub(super) fn validate(
    source: &FunctionFragmentInstructionSpan,
    candidate: &FunctionFragmentInstructionSpan,
    architecture: Architecture,
    physical: &ValidatedPhysicalRegisterModel,
    block_offsets: &std::collections::BTreeMap<SelectedBlockId, u64>,
) -> Result<(), FrameApplicationError> {
    let (Some(source_branch), Some(branch)) =
        (source.branch.as_deref(), candidate.branch.as_deref())
    else {
        return Err(FrameApplicationError::ArtifactMismatch);
    };
    if let (Branch::Jump(source_branch), Branch::Jump(branch)) = (source_branch, branch) {
        if source_branch.source_block != branch.source_block
            || source_branch.target_edge != branch.target_edge
            || source_branch.target_block != branch.target_block
            || source_branch.decoded_effects != branch.decoded_effects
        {
            return Err(FrameApplicationError::ArtifactMismatch);
        }
        let target = block_offsets.get(&branch.target_block).copied().ok_or(
            FrameApplicationError::MissingTargetBlock(branch.target_block),
        )?;
        let reference = match architecture {
            Architecture::X86_64 => candidate
                .offset
                .checked_add(candidate.bytes.len() as u64)
                .ok_or(FrameApplicationError::OffsetOverflow)?,
            Architecture::Aarch64 => candidate.offset,
        };
        let displacement = i64::try_from(i128::from(target) - i128::from(reference))
            .map_err(|_| FrameApplicationError::OffsetOverflow)?;
        if branch.target_offset != target || branch.byte_displacement != displacement {
            return Err(FrameApplicationError::ArtifactMismatch);
        }
        let effects = match architecture {
            Architecture::X86_64 => isa_x86_64::validate_x86_64_selected_jump_form(
                physical,
                candidate.alternative,
                displacement,
                &candidate.bytes,
            )
            .map_err(|error| FrameApplicationError::X86_64Branch(candidate.instruction, error))?
            .footprint()
            .encoded
            .clone(),
            Architecture::Aarch64 => isa_aarch64::validate_aarch64_selected_jump_form(
                physical,
                candidate.alternative,
                displacement,
                &candidate.bytes,
            )
            .map_err(|error| FrameApplicationError::Aarch64Branch(candidate.instruction, error))?
            .footprint()
            .encoded
            .clone(),
        };
        return if effects == branch.decoded_effects {
            Ok(())
        } else {
            Err(FrameApplicationError::BranchEffectsMismatch(
                candidate.instruction,
            ))
        };
    }
    let (Branch::Conditional(source_branch), Branch::Conditional(branch)) = (source_branch, branch)
    else {
        return Err(FrameApplicationError::ArtifactMismatch);
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
        return Err(FrameApplicationError::ArtifactMismatch);
    }
    let taken = block_offsets.get(&branch.when_taken_block).copied().ok_or(
        FrameApplicationError::MissingTargetBlock(branch.when_taken_block),
    )?;
    let fallthrough = block_offsets
        .get(&branch.when_fallthrough_block)
        .copied()
        .ok_or(FrameApplicationError::MissingTargetBlock(
            branch.when_fallthrough_block,
        ))?;
    let end = candidate
        .offset
        .checked_add(
            u64::try_from(candidate.bytes.len())
                .map_err(|_| FrameApplicationError::OffsetOverflow)?,
        )
        .ok_or(FrameApplicationError::OffsetOverflow)?;
    if fallthrough != end {
        return Err(FrameApplicationError::BranchFallthroughMismatch(
            candidate.instruction,
        ));
    }
    let reference = match architecture {
        Architecture::X86_64 => end,
        Architecture::Aarch64 => candidate.offset,
    };
    let displacement = i64::try_from(i128::from(taken) - i128::from(reference))
        .map_err(|_| FrameApplicationError::OffsetOverflow)?;
    if branch.when_taken_offset != taken
        || branch.when_fallthrough_offset != fallthrough
        || branch.byte_displacement != displacement
    {
        return Err(FrameApplicationError::ArtifactMismatch);
    }
    let (reads, effects) = decode(candidate, branch.predicate, architecture, physical)?;
    if reads != branch.decoded_register_reads || effects != branch.decoded_effects {
        return Err(FrameApplicationError::BranchEffectsMismatch(
            candidate.instruction,
        ));
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
        Vec<register_model::RegisterViewId>,
        selected_instructions::MachineEncodedEffects,
    ),
    FrameApplicationError,
> {
    let instruction = row.instruction;
    let Some(Branch::Conditional(branch)) = row.branch.as_deref() else {
        return Err(FrameApplicationError::ArtifactMismatch);
    };
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
            .map_err(|error| FrameApplicationError::X86_64Branch(instruction, error))?;
            footprint!(decoded)
        }
        (Architecture::X86_64, FunctionFragmentConditionalBranchPredicate::U64LessThanV1) => {
            let decoded = validate_x86_64_selected_u64_less_than_branch_form(
                physical,
                row.alternative,
                branch.byte_displacement,
                &row.bytes,
            )
            .map_err(|error| FrameApplicationError::X86_64Branch(instruction, error))?;
            footprint!(decoded)
        }
        (Architecture::X86_64, FunctionFragmentConditionalBranchPredicate::I64LessThanV1) => {
            let decoded = validate_x86_64_selected_i64_less_than_branch_form(
                physical,
                row.alternative,
                branch.byte_displacement,
                &row.bytes,
            )
            .map_err(|error| FrameApplicationError::X86_64Branch(instruction, error))?;
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
            .map_err(|error| FrameApplicationError::Aarch64Branch(instruction, error))?;
            footprint!(decoded)
        }
        (Architecture::Aarch64, FunctionFragmentConditionalBranchPredicate::NonZeroV1) => {
            let decoded = validate_aarch64_selected_nonzero_branch_form(
                physical,
                row.alternative,
                branch.byte_displacement,
                &row.bytes,
            )
            .map_err(|error| FrameApplicationError::Aarch64Branch(instruction, error))?;
            footprint!(decoded)
        }
        (Architecture::Aarch64, FunctionFragmentConditionalBranchPredicate::U64LessThanV1) => {
            let decoded = validate_aarch64_selected_u64_less_than_branch_form(
                physical,
                row.alternative,
                branch.byte_displacement,
                &row.bytes,
            )
            .map_err(|error| FrameApplicationError::Aarch64Branch(instruction, error))?;
            footprint!(decoded)
        }
        (Architecture::Aarch64, FunctionFragmentConditionalBranchPredicate::I64LessThanV1) => {
            let decoded = validate_aarch64_selected_i64_less_than_branch_form(
                physical,
                row.alternative,
                branch.byte_displacement,
                &row.bytes,
            )
            .map_err(|error| FrameApplicationError::Aarch64Branch(instruction, error))?;
            footprint!(decoded)
        }
    };
    Ok(decoded)
}
