use std::collections::BTreeMap;

use physical_instructions::PostAllocationMachineInstruction;
use register_model::ValidatedPhysicalRegisterModel;
use selected_instructions::{
    MachineAlternativeFamily, SelectedBlock, SelectedBlockId, SelectedInstruction,
    SelectedSuccessor, SelectedTerminator,
};
use target::Architecture;

use super::super::{
    OptimizedResolvedSelectedFormLayoutError, ResolvedBranchEvidence,
    ResolvedConditionalBranchEvidence, ResolvedConditionalBranchPredicate, ResolvedJumpEvidence,
    declared_size_matches,
};
use super::plan::{branch_size, unexpected};

/// One selected conditional-branch terminator read as the branch it encodes:
/// the resolved predicate plus the instruction and the two successor rows the
/// terminator carries. The resolver, the layout plan's widening check, both
/// order walks and the independent validator read a block's terminator through
/// this one owner; `None` names a terminator that is not a conditional branch.
pub(crate) struct ConditionalBranchDescriptor<'a> {
    pub(crate) predicate: ResolvedConditionalBranchPredicate,
    pub(crate) terminator: &'a SelectedInstruction,
    pub(crate) when_taken: &'a SelectedSuccessor,
    pub(crate) when_fallthrough: &'a SelectedSuccessor,
}

pub(crate) fn conditional_branch_descriptor(
    block: &SelectedBlock,
) -> Option<ConditionalBranchDescriptor<'_>> {
    match &block.terminator {
        SelectedTerminator::ConditionalBranch {
            instruction,
            when_nonzero,
            when_zero,
        } => Some(ConditionalBranchDescriptor {
            predicate: ResolvedConditionalBranchPredicate::NonZeroV1,
            terminator: instruction,
            when_taken: when_nonzero,
            when_fallthrough: when_zero,
        }),
        SelectedTerminator::ConditionalBranchU64LessThan {
            instruction,
            when_less,
            when_not_less,
        } => Some(ConditionalBranchDescriptor {
            predicate: ResolvedConditionalBranchPredicate::U64LessThanV1,
            terminator: instruction,
            when_taken: when_less,
            when_fallthrough: when_not_less,
        }),
        SelectedTerminator::ConditionalBranchI64LessThan {
            instruction,
            when_less,
            when_not_less,
        } => Some(ConditionalBranchDescriptor {
            predicate: ResolvedConditionalBranchPredicate::I64LessThanV1,
            terminator: instruction,
            when_taken: when_less,
            when_fallthrough: when_not_less,
        }),
        SelectedTerminator::Jump { .. }
        | selected_instructions::terminator_no_successor_pattern!(..) => None,
    }
}

/// The machine alternative family one resolved conditional-branch predicate
/// names: the stage's own guard on the recorded alternative, and the family
/// the AArch64 encoder and decoder requests assert.
pub(crate) const fn conditional_branch_family(
    predicate: ResolvedConditionalBranchPredicate,
) -> MachineAlternativeFamily {
    match predicate {
        ResolvedConditionalBranchPredicate::NonZeroV1 => {
            MachineAlternativeFamily::ConditionalBranchNonZero
        }
        ResolvedConditionalBranchPredicate::U64LessThanV1 => {
            MachineAlternativeFamily::ConditionalBranchU64LessThan
        }
        ResolvedConditionalBranchPredicate::I64LessThanV1 => {
            MachineAlternativeFamily::ConditionalBranchI64LessThan
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn resolve(
    architecture: Architecture,
    block: &SelectedBlock,
    instruction: &SelectedInstruction,
    instruction_offset: u64,
    block_offsets: &BTreeMap<SelectedBlockId, u64>,
    machine: &PostAllocationMachineInstruction,
    physical: &ValidatedPhysicalRegisterModel,
    widened: bool,
) -> Result<(Vec<u8>, Option<Box<ResolvedBranchEvidence>>), OptimizedResolvedSelectedFormLayoutError>
{
    if let SelectedTerminator::Jump {
        instruction: jump,
        successor,
    } = &block.terminator
    {
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
                let displacement = super::plan::byte_displacement(target_offset, end)?;
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
                let displacement =
                    super::plan::byte_displacement(target_offset, instruction_offset)?;
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
    let Some(descriptor) = conditional_branch_descriptor(block) else {
        return unexpected(instruction.id);
    };
    let ConditionalBranchDescriptor {
        predicate,
        terminator,
        when_taken,
        when_fallthrough,
    } = descriptor;
    if terminator.id != instruction.id {
        return unexpected(instruction.id);
    }
    if machine.alternative.key.family != conditional_branch_family(predicate) {
        return Err(OptimizedResolvedSelectedFormLayoutError::AlternativeMismatch(instruction.id));
    }
    let taken_offset = *block_offsets.get(&when_taken.block).ok_or(
        OptimizedResolvedSelectedFormLayoutError::BranchFallthroughMismatch(instruction.id),
    )?;
    let fallthrough_offset = *block_offsets.get(&when_fallthrough.block).ok_or(
        OptimizedResolvedSelectedFormLayoutError::BranchFallthroughMismatch(instruction.id),
    )?;
    // The plan sizes widened AArch64 rows at eight bytes; the unconditional
    // `B` that carries the far displacement occupies the row's second word.
    let byte_count = if widened {
        8
    } else {
        branch_size(architecture)
    };
    let branch_end = instruction_offset
        .checked_add(byte_count)
        .ok_or(OptimizedResolvedSelectedFormLayoutError::OffsetOverflow)?;
    if fallthrough_offset != branch_end {
        return Err(
            OptimizedResolvedSelectedFormLayoutError::BranchFallthroughMismatch(instruction.id),
        );
    }
    let displacement = match architecture {
        Architecture::X86_64 => super::plan::byte_displacement(taken_offset, branch_end)?,
        Architecture::Aarch64 => super::plan::byte_displacement(taken_offset, instruction_offset)?,
    };
    let (bytes, register_reads, effects) = match architecture {
        Architecture::X86_64 => {
            let realized = match predicate {
                ResolvedConditionalBranchPredicate::NonZeroV1 => {
                    isa_x86_64::encode_x86_64_selected_nonzero_branch_form(
                        physical,
                        machine.alternative.key,
                        displacement,
                    )
                }
                ResolvedConditionalBranchPredicate::U64LessThanV1 => {
                    isa_x86_64::encode_x86_64_selected_u64_less_than_branch_form(
                        physical,
                        machine.alternative.key,
                        displacement,
                    )
                }
                ResolvedConditionalBranchPredicate::I64LessThanV1 => {
                    isa_x86_64::encode_x86_64_selected_i64_less_than_branch_form(
                        physical,
                        machine.alternative.key,
                        displacement,
                    )
                }
            }
            .map_err(OptimizedResolvedSelectedFormLayoutError::X86_64)?;
            (
                realized.bytes().to_vec(),
                realized.footprint().register_reads.clone(),
                realized.footprint().encoded.clone(),
            )
        }
        Architecture::Aarch64 => {
            let form = if widened {
                isa_aarch64::Aarch64ConditionalBranchForm::WidenedPairV1
            } else {
                isa_aarch64::Aarch64ConditionalBranchForm::CanonicalWordV1
            };
            let realized = isa_aarch64::encode_aarch64_selected_conditional_branch_form(
                physical,
                machine.alternative.key,
                conditional_branch_family(predicate),
                form,
                displacement,
            )
            .map_err(OptimizedResolvedSelectedFormLayoutError::Aarch64)?;
            (
                realized.bytes().to_vec(),
                realized.footprint().register_reads.clone(),
                realized.footprint().encoded.clone(),
            )
        }
    };
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
