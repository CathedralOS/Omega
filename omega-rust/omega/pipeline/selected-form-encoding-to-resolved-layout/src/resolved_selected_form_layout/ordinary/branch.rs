use std::collections::BTreeMap;

use isa_aarch64::{
    encode_aarch64_selected_i64_less_than_branch_form, encode_aarch64_selected_nonzero_branch_form,
    encode_aarch64_selected_u64_less_than_branch_form,
};
use isa_x86_64::{
    encode_x86_64_selected_i64_less_than_branch_form, encode_x86_64_selected_nonzero_branch_form,
    encode_x86_64_selected_u64_less_than_branch_form,
};
use physical_instructions::PostAllocationMachineInstruction;
use register_model::ValidatedPhysicalRegisterModel;
use selected_instructions::{
    MachineEncodedEffects, MachineSizeKnowledge, SelectedBlock, SelectedBlockId,
    SelectedInstruction, SelectedInstructionId, SelectedTerminator,
};
use target::Architecture;

use super::super::{
    OptimizedResolvedSelectedFormLayoutError, ResolvedBranchEvidence,
    ResolvedConditionalBranchEvidence, ResolvedConditionalBranchPredicate, ResolvedJumpEvidence,
};

#[allow(clippy::too_many_arguments)]
pub(super) fn resolve(
    architecture: Architecture,
    block: &SelectedBlock,
    instruction: &SelectedInstruction,
    instruction_offset: u64,
    block_offsets: &BTreeMap<SelectedBlockId, u64>,
    machine: &PostAllocationMachineInstruction,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<(Vec<u8>, Option<Box<ResolvedBranchEvidence>>), OptimizedResolvedSelectedFormLayoutError>
{
    let (predicate, terminator, when_taken, when_fallthrough) = match &block.terminator {
        SelectedTerminator::Jump {
            instruction: jump,
            successor,
        } => {
            if jump.id != instruction.id {
                return unexpected(instruction.id);
            }
            let target_offset = *block_offsets.get(&successor.block).ok_or(
                OptimizedResolvedSelectedFormLayoutError::BranchFallthroughMismatch(instruction.id),
            )?;
            let (bytes, displacement, effects) = match architecture {
                Architecture::X86_64 => {
                    let end = instruction_offset
                        .checked_add(5)
                        .ok_or(OptimizedResolvedSelectedFormLayoutError::OffsetOverflow)?;
                    let displacement = checked_delta(target_offset, end)?;
                    let result = isa_x86_64::encode_x86_64_selected_jump_form(
                        physical,
                        machine.alternative.key,
                        displacement,
                    )
                    .map_err(OptimizedResolvedSelectedFormLayoutError::X86_64)?;
                    (
                        result.bytes().to_vec(),
                        displacement,
                        result.footprint().encoded.clone(),
                    )
                }
                Architecture::Aarch64 => {
                    let displacement = checked_delta(target_offset, instruction_offset)?;
                    let result = isa_aarch64::encode_aarch64_selected_jump_form(
                        physical,
                        machine.alternative.key,
                        displacement,
                    )
                    .map_err(OptimizedResolvedSelectedFormLayoutError::Aarch64)?;
                    (
                        result.bytes().to_vec(),
                        displacement,
                        result.footprint().encoded.clone(),
                    )
                }
            };
            if effects != machine.alternative.encoded
                || !declared_size_matches(machine.alternative.size, bytes.len() as u64)
            {
                return unexpected(instruction.id);
            }
            return Ok((
                bytes,
                Some(Box::new(ResolvedBranchEvidence::Jump(
                    ResolvedJumpEvidence {
                        source_block: block.id,
                        target_edge: successor.psi_edge,
                        target_block: successor.block,
                        target_offset,
                        byte_displacement: displacement,
                        decoded_effects: effects,
                    },
                ))),
            ));
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
        SelectedTerminator::Return { .. } => return unexpected(instruction.id),
    };
    if terminator.id != instruction.id {
        return unexpected(instruction.id);
    }
    let taken_offset = *block_offsets.get(&when_taken.block).ok_or(
        OptimizedResolvedSelectedFormLayoutError::BranchFallthroughMismatch(instruction.id),
    )?;
    let fallthrough_offset = *block_offsets.get(&when_fallthrough.block).ok_or(
        OptimizedResolvedSelectedFormLayoutError::BranchFallthroughMismatch(instruction.id),
    )?;
    let byte_count = branch_size(architecture);
    let branch_end = instruction_offset
        .checked_add(byte_count)
        .ok_or(OptimizedResolvedSelectedFormLayoutError::OffsetOverflow)?;
    if fallthrough_offset != branch_end {
        return Err(
            OptimizedResolvedSelectedFormLayoutError::BranchFallthroughMismatch(instruction.id),
        );
    }
    let displacement = match architecture {
        Architecture::X86_64 => checked_delta(taken_offset, branch_end)?,
        Architecture::Aarch64 => checked_delta(taken_offset, instruction_offset)?,
    };
    let (bytes, register_reads, effects) =
        encode(architecture, physical, machine, predicate, displacement)?;
    if effects != machine.alternative.encoded {
        return Err(
            OptimizedResolvedSelectedFormLayoutError::BranchEffectsMismatch(instruction.id),
        );
    }
    if u64::try_from(bytes.len()).ok() != Some(byte_count)
        || !declared_size_matches(machine.alternative.size, byte_count)
    {
        return Err(OptimizedResolvedSelectedFormLayoutError::BranchSizeMismatch(instruction.id));
    }
    Ok((
        bytes,
        Some(Box::new(ResolvedBranchEvidence::Conditional(
            ResolvedConditionalBranchEvidence {
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
            },
        ))),
    ))
}

fn encode(
    architecture: Architecture,
    physical: &ValidatedPhysicalRegisterModel,
    machine: &PostAllocationMachineInstruction,
    predicate: ResolvedConditionalBranchPredicate,
    displacement: i64,
) -> Result<
    (
        Vec<u8>,
        Vec<register_model::RegisterViewId>,
        MachineEncodedEffects,
    ),
    OptimizedResolvedSelectedFormLayoutError,
> {
    match (architecture, predicate) {
        (Architecture::X86_64, ResolvedConditionalBranchPredicate::NonZeroV1) => {
            let encoded = encode_x86_64_selected_nonzero_branch_form(
                physical,
                machine.alternative.key,
                displacement,
            )
            .map_err(OptimizedResolvedSelectedFormLayoutError::X86_64)?;
            Ok((
                encoded.bytes().to_vec(),
                encoded.footprint().register_reads.clone(),
                encoded.footprint().encoded.clone(),
            ))
        }
        (Architecture::Aarch64, ResolvedConditionalBranchPredicate::NonZeroV1) => {
            let encoded = encode_aarch64_selected_nonzero_branch_form(
                physical,
                machine.alternative.key,
                displacement,
            )
            .map_err(OptimizedResolvedSelectedFormLayoutError::Aarch64)?;
            Ok((
                encoded.bytes().to_vec(),
                encoded.footprint().register_reads.clone(),
                encoded.footprint().encoded.clone(),
            ))
        }
        (Architecture::X86_64, ResolvedConditionalBranchPredicate::U64LessThanV1) => {
            let encoded = encode_x86_64_selected_u64_less_than_branch_form(
                physical,
                machine.alternative.key,
                displacement,
            )
            .map_err(OptimizedResolvedSelectedFormLayoutError::X86_64)?;
            Ok((
                encoded.bytes().to_vec(),
                encoded.footprint().register_reads.clone(),
                encoded.footprint().encoded.clone(),
            ))
        }
        (Architecture::Aarch64, ResolvedConditionalBranchPredicate::U64LessThanV1) => {
            let encoded = encode_aarch64_selected_u64_less_than_branch_form(
                physical,
                machine.alternative.key,
                displacement,
            )
            .map_err(OptimizedResolvedSelectedFormLayoutError::Aarch64)?;
            Ok((
                encoded.bytes().to_vec(),
                encoded.footprint().register_reads.clone(),
                encoded.footprint().encoded.clone(),
            ))
        }
        (Architecture::X86_64, ResolvedConditionalBranchPredicate::I64LessThanV1) => {
            let encoded = encode_x86_64_selected_i64_less_than_branch_form(
                physical,
                machine.alternative.key,
                displacement,
            )
            .map_err(OptimizedResolvedSelectedFormLayoutError::X86_64)?;
            Ok((
                encoded.bytes().to_vec(),
                encoded.footprint().register_reads.clone(),
                encoded.footprint().encoded.clone(),
            ))
        }
        (Architecture::Aarch64, ResolvedConditionalBranchPredicate::I64LessThanV1) => {
            let encoded = encode_aarch64_selected_i64_less_than_branch_form(
                physical,
                machine.alternative.key,
                displacement,
            )
            .map_err(OptimizedResolvedSelectedFormLayoutError::Aarch64)?;
            Ok((
                encoded.bytes().to_vec(),
                encoded.footprint().register_reads.clone(),
                encoded.footprint().encoded.clone(),
            ))
        }
    }
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

const fn branch_size(architecture: Architecture) -> u64 {
    match architecture {
        Architecture::X86_64 => 6,
        Architecture::Aarch64 => 4,
    }
}

fn checked_delta(target: u64, base: u64) -> Result<i64, OptimizedResolvedSelectedFormLayoutError> {
    i64::try_from(i128::from(target) - i128::from(base))
        .map_err(|_| OptimizedResolvedSelectedFormLayoutError::OffsetOverflow)
}

fn unexpected<T>(
    instruction: SelectedInstructionId,
) -> Result<T, OptimizedResolvedSelectedFormLayoutError> {
    Err(OptimizedResolvedSelectedFormLayoutError::UnexpectedEncodingState(instruction))
}
