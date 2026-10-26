use std::collections::BTreeMap;

use physical_instructions::PostAllocationMachineInstruction;
use register_model::ValidatedPhysicalRegisterModel;
use selected_instructions::{
    SelectedBlock, SelectedBlockId, SelectedInstruction, SelectedTerminator,
};
use target::Architecture;

use super::super::ordinary::{
    ConditionalBranchDescriptor, branch_size, conditional_branch_descriptor,
    conditional_branch_family,
};
use super::super::{
    OptimizedResolvedSelectedFormLayoutError, ResolvedBranchEvidence,
    ResolvedConditionalBranchEvidence, ResolvedConditionalBranchPredicate, ResolvedSelectedFormRow,
    declared_size_matches,
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
    widened: bool,
) -> Result<(), OptimizedResolvedSelectedFormLayoutError> {
    if let SelectedTerminator::Jump {
        instruction: jump,
        successor,
    } = &block.terminator
    {
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
    let Some(descriptor) = conditional_branch_descriptor(block) else {
        return Err(OptimizedResolvedSelectedFormLayoutError::ArtifactMismatch);
    };
    let ConditionalBranchDescriptor {
        predicate,
        terminator,
        when_taken,
        when_fallthrough,
    } = descriptor;
    if terminator.id != instruction.id {
        return Err(OptimizedResolvedSelectedFormLayoutError::ArtifactMismatch);
    }
    if machine.alternative.key.family != conditional_branch_family(predicate) {
        return Err(OptimizedResolvedSelectedFormLayoutError::ArtifactMismatch);
    }
    let taken_offset = *block_offsets
        .get(&when_taken.block)
        .ok_or(OptimizedResolvedSelectedFormLayoutError::ArtifactMismatch)?;
    let fallthrough_offset = *block_offsets
        .get(&when_fallthrough.block)
        .ok_or(OptimizedResolvedSelectedFormLayoutError::ArtifactMismatch)?;
    // The plan independently re-derives which AArch64 rows widen to
    // `B.<invcond> +8; B target`; a candidate row must match that size exactly.
    let branch_size = if widened {
        8
    } else {
        branch_size(architecture)
    };
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
    let (register_reads, effects) = match architecture {
        Architecture::X86_64 => {
            let decoded = match predicate {
                ResolvedConditionalBranchPredicate::NonZeroV1 => {
                    isa_x86_64::validate_x86_64_selected_nonzero_branch_form(
                        physical,
                        machine.alternative.key,
                        displacement,
                        &candidate.bytes,
                    )
                }
                ResolvedConditionalBranchPredicate::U64LessThanV1 => {
                    isa_x86_64::validate_x86_64_selected_u64_less_than_branch_form(
                        physical,
                        machine.alternative.key,
                        displacement,
                        &candidate.bytes,
                    )
                }
                ResolvedConditionalBranchPredicate::I64LessThanV1 => {
                    isa_x86_64::validate_x86_64_selected_i64_less_than_branch_form(
                        physical,
                        machine.alternative.key,
                        displacement,
                        &candidate.bytes,
                    )
                }
            }
            .map_err(|_| OptimizedResolvedSelectedFormLayoutError::ArtifactMismatch)?;
            (
                decoded.footprint().register_reads.clone(),
                decoded.footprint().encoded.clone(),
            )
        }
        Architecture::Aarch64 => {
            let form = if widened {
                isa_aarch64::Aarch64ConditionalBranchForm::WidenedPairV1
            } else {
                isa_aarch64::Aarch64ConditionalBranchForm::CanonicalWordV1
            };
            let decoded = isa_aarch64::validate_aarch64_selected_conditional_branch_form(
                physical,
                machine.alternative.key,
                conditional_branch_family(predicate),
                form,
                displacement,
                &candidate.bytes,
            )
            .map_err(|_| OptimizedResolvedSelectedFormLayoutError::ArtifactMismatch)?;
            (
                decoded.footprint().register_reads.clone(),
                decoded.footprint().encoded.clone(),
            )
        }
    };
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

fn checked_delta(target: u64, base: u64) -> Result<i64, OptimizedResolvedSelectedFormLayoutError> {
    i64::try_from(i128::from(target) - i128::from(base))
        .map_err(|_| OptimizedResolvedSelectedFormLayoutError::ArtifactMismatch)
}
