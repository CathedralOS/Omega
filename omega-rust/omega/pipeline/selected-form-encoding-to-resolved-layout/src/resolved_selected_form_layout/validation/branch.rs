use std::collections::BTreeMap;

use isa_aarch64::{
    validate_aarch64_selected_i64_less_than_branch_form,
    validate_aarch64_selected_nonzero_branch_form,
    validate_aarch64_selected_u64_less_than_branch_form,
};
use isa_x86_64::{
    validate_x86_64_selected_i64_less_than_branch_form,
    validate_x86_64_selected_nonzero_branch_form,
    validate_x86_64_selected_u64_less_than_branch_form,
};
use physical_instructions::PostAllocationMachineInstruction;
use register_model::ValidatedPhysicalRegisterModel;
use selected_instructions::{
    MachineEncodedEffects, MachineSizeKnowledge, SelectedBlock, SelectedBlockId,
    SelectedInstruction, SelectedTerminator,
};
use target::Architecture;

use super::super::{
    OptimizedResolvedSelectedFormLayoutError, ResolvedBranchEvidence,
    ResolvedConditionalBranchEvidence, ResolvedConditionalBranchPredicate, ResolvedSelectedFormRow,
};

#[allow(clippy::too_many_arguments)]
pub(super) fn validate(
    architecture: Architecture,
    block: &SelectedBlock,
    instruction: &SelectedInstruction,
    instruction_offset: u64,
    block_offsets: &BTreeMap<SelectedBlockId, u64>,
    machine: &PostAllocationMachineInstruction,
    physical: &ValidatedPhysicalRegisterModel,
    candidate: &ResolvedSelectedFormRow,
) -> Result<(), OptimizedResolvedSelectedFormLayoutError> {
    let (predicate, terminator, when_taken, when_fallthrough) = match &block.terminator {
        SelectedTerminator::Jump {
            instruction: jump,
            successor,
        } => {
            if jump.id != instruction.id {
                return Err(OptimizedResolvedSelectedFormLayoutError::ArtifactMismatch);
            }
            let Some(ResolvedBranchEvidence::Jump(actual)) = candidate.branch.as_deref() else {
                return Err(OptimizedResolvedSelectedFormLayoutError::ArtifactMismatch);
            };
            let target = *block_offsets
                .get(&successor.block)
                .ok_or(OptimizedResolvedSelectedFormLayoutError::ArtifactMismatch)?;
            let effects = match architecture {
                Architecture::X86_64 => {
                    let end = instruction_offset
                        .checked_add(5)
                        .ok_or(OptimizedResolvedSelectedFormLayoutError::ArtifactMismatch)?;
                    let displacement = checked_delta(target, end)?;
                    if displacement != actual.byte_displacement {
                        return Err(OptimizedResolvedSelectedFormLayoutError::ArtifactMismatch);
                    }
                    isa_x86_64::validate_x86_64_selected_jump_form(
                        physical,
                        machine.alternative.key,
                        displacement,
                        &candidate.bytes,
                    )
                    .map_err(|_| OptimizedResolvedSelectedFormLayoutError::ArtifactMismatch)?
                    .footprint()
                    .encoded
                    .clone()
                }
                Architecture::Aarch64 => {
                    let displacement = checked_delta(target, instruction_offset)?;
                    if displacement != actual.byte_displacement {
                        return Err(OptimizedResolvedSelectedFormLayoutError::ArtifactMismatch);
                    }
                    isa_aarch64::validate_aarch64_selected_jump_form(
                        physical,
                        machine.alternative.key,
                        displacement,
                        &candidate.bytes,
                    )
                    .map_err(|_| OptimizedResolvedSelectedFormLayoutError::ArtifactMismatch)?
                    .footprint()
                    .encoded
                    .clone()
                }
            };
            if actual.source_block != block.id
                || actual.target_edge != successor.psi_edge
                || actual.target_block != successor.block
                || actual.target_offset != target
                || actual.decoded_effects != effects
                || effects != machine.alternative.encoded
                || !declared_size_matches(machine.alternative.size, candidate.bytes.len() as u64)
            {
                return Err(OptimizedResolvedSelectedFormLayoutError::ArtifactMismatch);
            }
            return Ok(());
        }
        SelectedTerminator::ConditionalBranch {
            instruction,
            when_nonzero,
            when_zero,
        } => (
            ResolvedConditionalBranchPredicate::NonZeroV1,
            instruction,
            when_nonzero,
            when_zero,
        ),
        SelectedTerminator::ConditionalBranchU64LessThan {
            instruction,
            when_less,
            when_not_less,
        } => (
            ResolvedConditionalBranchPredicate::U64LessThanV1,
            instruction,
            when_less,
            when_not_less,
        ),
        SelectedTerminator::ConditionalBranchI64LessThan {
            instruction,
            when_less,
            when_not_less,
        } => (
            ResolvedConditionalBranchPredicate::I64LessThanV1,
            instruction,
            when_less,
            when_not_less,
        ),
        SelectedTerminator::Return { .. } | SelectedTerminator::HostedExitProcess { .. } => {
            return Err(OptimizedResolvedSelectedFormLayoutError::ArtifactMismatch);
        }
    };
    if terminator.id != instruction.id {
        return Err(OptimizedResolvedSelectedFormLayoutError::ArtifactMismatch);
    }
    let taken_offset = *block_offsets
        .get(&when_taken.block)
        .ok_or(OptimizedResolvedSelectedFormLayoutError::ArtifactMismatch)?;
    let fallthrough_offset = *block_offsets
        .get(&when_fallthrough.block)
        .ok_or(OptimizedResolvedSelectedFormLayoutError::ArtifactMismatch)?;
    let branch_size = branch_size(architecture);
    let branch_end = instruction_offset
        .checked_add(branch_size)
        .ok_or(OptimizedResolvedSelectedFormLayoutError::ArtifactMismatch)?;
    if fallthrough_offset != branch_end {
        return Err(OptimizedResolvedSelectedFormLayoutError::ArtifactMismatch);
    }
    let displacement = match architecture {
        Architecture::X86_64 => checked_delta(taken_offset, branch_end)?,
        Architecture::Aarch64 => checked_delta(taken_offset, instruction_offset)?,
    };
    let (register_reads, effects) = decode(
        architecture,
        physical,
        machine,
        predicate,
        displacement,
        &candidate.bytes,
    )?;
    if effects != machine.alternative.encoded {
        return Err(OptimizedResolvedSelectedFormLayoutError::ArtifactMismatch);
    }
    if u64::try_from(candidate.bytes.len()).ok() != Some(branch_size)
        || !declared_size_matches(machine.alternative.size, branch_size)
    {
        return Err(OptimizedResolvedSelectedFormLayoutError::ArtifactMismatch);
    }
    let evidence = ResolvedConditionalBranchEvidence {
        predicate,
        source_block: block.id,
        when_taken_edge: when_taken.psi_edge,
        when_taken_block: when_taken.block,
        when_taken_offset: taken_offset,
        when_fallthrough_edge: when_fallthrough.psi_edge,
        when_fallthrough_block: when_fallthrough.block,
        when_fallthrough_offset: fallthrough_offset,
        byte_displacement: displacement,
        decoded_register_reads: register_reads,
        decoded_effects: effects,
    };
    if candidate.branch.as_deref() != Some(&ResolvedBranchEvidence::Conditional(evidence)) {
        return Err(OptimizedResolvedSelectedFormLayoutError::ArtifactMismatch);
    }
    Ok(())
}

fn decode(
    architecture: Architecture,
    physical: &ValidatedPhysicalRegisterModel,
    machine: &PostAllocationMachineInstruction,
    predicate: ResolvedConditionalBranchPredicate,
    displacement: i64,
    bytes: &[u8],
) -> Result<
    (Vec<register_model::RegisterViewId>, MachineEncodedEffects),
    OptimizedResolvedSelectedFormLayoutError,
> {
    let footprint = match (architecture, predicate) {
        (Architecture::X86_64, ResolvedConditionalBranchPredicate::NonZeroV1) => {
            let decoded = validate_x86_64_selected_nonzero_branch_form(
                physical,
                machine.alternative.key,
                displacement,
                bytes,
            )
            .map_err(|_| OptimizedResolvedSelectedFormLayoutError::ArtifactMismatch)?;
            return Ok((
                decoded.footprint().register_reads.clone(),
                decoded.footprint().encoded.clone(),
            ));
        }
        (Architecture::Aarch64, ResolvedConditionalBranchPredicate::NonZeroV1) => {
            validate_aarch64_selected_nonzero_branch_form(
                physical,
                machine.alternative.key,
                displacement,
                bytes,
            )
            .map_err(|_| OptimizedResolvedSelectedFormLayoutError::ArtifactMismatch)?
            .footprint()
            .clone()
        }
        (Architecture::X86_64, ResolvedConditionalBranchPredicate::U64LessThanV1) => {
            let decoded = validate_x86_64_selected_u64_less_than_branch_form(
                physical,
                machine.alternative.key,
                displacement,
                bytes,
            )
            .map_err(|_| OptimizedResolvedSelectedFormLayoutError::ArtifactMismatch)?;
            return Ok((
                decoded.footprint().register_reads.clone(),
                decoded.footprint().encoded.clone(),
            ));
        }
        (Architecture::Aarch64, ResolvedConditionalBranchPredicate::U64LessThanV1) => {
            validate_aarch64_selected_u64_less_than_branch_form(
                physical,
                machine.alternative.key,
                displacement,
                bytes,
            )
            .map_err(|_| OptimizedResolvedSelectedFormLayoutError::ArtifactMismatch)?
            .footprint()
            .clone()
        }
        (Architecture::X86_64, ResolvedConditionalBranchPredicate::I64LessThanV1) => {
            let decoded = validate_x86_64_selected_i64_less_than_branch_form(
                physical,
                machine.alternative.key,
                displacement,
                bytes,
            )
            .map_err(|_| OptimizedResolvedSelectedFormLayoutError::ArtifactMismatch)?;
            return Ok((
                decoded.footprint().register_reads.clone(),
                decoded.footprint().encoded.clone(),
            ));
        }
        (Architecture::Aarch64, ResolvedConditionalBranchPredicate::I64LessThanV1) => {
            validate_aarch64_selected_i64_less_than_branch_form(
                physical,
                machine.alternative.key,
                displacement,
                bytes,
            )
            .map_err(|_| OptimizedResolvedSelectedFormLayoutError::ArtifactMismatch)?
            .footprint()
            .clone()
        }
    };
    Ok((footprint.register_reads, footprint.encoded))
}

fn declared_size_matches(knowledge: MachineSizeKnowledge, actual: u64) -> bool {
    match knowledge {
        MachineSizeKnowledge::ExactBytes(expected) => u64::from(expected) == actual,
        MachineSizeKnowledge::EncoderResolved {
            minimum_bytes,
            maximum_bytes,
        } => {
            actual >= u64::from(minimum_bytes)
                && maximum_bytes.is_none_or(|maximum| actual <= u64::from(maximum))
        }
    }
}

fn branch_size(architecture: Architecture) -> u64 {
    match architecture {
        Architecture::X86_64 => 6,
        Architecture::Aarch64 => 4,
    }
}

fn checked_delta(target: u64, base: u64) -> Result<i64, OptimizedResolvedSelectedFormLayoutError> {
    i64::try_from(i128::from(target) - i128::from(base))
        .map_err(|_| OptimizedResolvedSelectedFormLayoutError::ArtifactMismatch)
}
