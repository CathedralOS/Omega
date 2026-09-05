use std::collections::BTreeMap;

use omega_isa_aarch64::{
    encode_aarch64_fused_compare_i64_zero_branch_nonzero_to_cbnz_form,
    encode_aarch64_selected_i64_less_than_branch_form, encode_aarch64_selected_nonzero_branch_form,
    encode_aarch64_selected_u64_less_than_branch_form,
};
use omega_isa_x86_64::{
    encode_x86_64_selected_i64_less_than_branch_form, encode_x86_64_selected_nonzero_branch_form,
    encode_x86_64_selected_u64_less_than_branch_form,
};
use omega_physical_instructions::PostAllocationMachineInstruction;
use omega_post_allocation_machine_to_optimized_machine::{
    Aarch64CbnzFusionAction, QualifiedPhysicalRead,
};
use omega_register_model::ValidatedPhysicalRegisterModel;
use omega_selected_instructions::{
    MachineEncodedEffects, MachineSizeKnowledge, SelectedBlock, SelectedBlockId,
    SelectedInstruction, SelectedInstructionId, SelectedTerminator,
};
use omega_target::Architecture;

use super::super::{
    OptimizedResolvedSelectedFormLayoutError, ResolvedConditionalBranchEvidence,
    ResolvedConditionalBranchPredicate,
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
    fused: Option<(&QualifiedPhysicalRead, &Aarch64CbnzFusionAction)>,
) -> Result<
    (Vec<u8>, Option<Box<ResolvedConditionalBranchEvidence>>),
    OptimizedResolvedSelectedFormLayoutError,
> {
    let (predicate, terminator, when_taken, when_fallthrough) = match &block.terminator {
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
    let (bytes, register_reads, effects) = encode(
        architecture,
        physical,
        machine,
        fused,
        predicate,
        displacement,
        instruction.id,
    )?;
    if let Some((source_read, action)) = fused {
        validate_fused_footprint(
            instruction.id,
            block,
            source_read,
            action,
            physical,
            &register_reads,
            &effects,
            &machine.alternative.encoded,
        )?;
    } else if effects != machine.alternative.encoded {
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
        Some(Box::new(ResolvedConditionalBranchEvidence {
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
        })),
    ))
}

fn encode(
    architecture: Architecture,
    physical: &ValidatedPhysicalRegisterModel,
    machine: &PostAllocationMachineInstruction,
    fused: Option<(&QualifiedPhysicalRead, &Aarch64CbnzFusionAction)>,
    predicate: ResolvedConditionalBranchPredicate,
    displacement: i64,
    instruction: SelectedInstructionId,
) -> Result<
    (
        Vec<u8>,
        Vec<omega_register_model::RegisterViewId>,
        MachineEncodedEffects,
    ),
    OptimizedResolvedSelectedFormLayoutError,
> {
    match (architecture, fused, predicate) {
        (
            Architecture::Aarch64,
            Some((source_read, _)),
            ResolvedConditionalBranchPredicate::NonZeroV1,
        ) => {
            let encoded = encode_aarch64_fused_compare_i64_zero_branch_nonzero_to_cbnz_form(
                physical,
                source_read.view,
                displacement,
            )
            .map_err(OptimizedResolvedSelectedFormLayoutError::Aarch64)?;
            Ok((
                encoded.bytes().to_vec(),
                encoded.footprint().register_reads.clone(),
                encoded.footprint().encoded.clone(),
            ))
        }
        (
            _,
            Some(_),
            ResolvedConditionalBranchPredicate::U64LessThanV1
            | ResolvedConditionalBranchPredicate::I64LessThanV1,
        )
        | (Architecture::X86_64, Some(_), _) => unexpected(instruction),
        (Architecture::X86_64, None, ResolvedConditionalBranchPredicate::NonZeroV1) => {
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
        (Architecture::Aarch64, None, ResolvedConditionalBranchPredicate::NonZeroV1) => {
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
        (Architecture::X86_64, None, ResolvedConditionalBranchPredicate::U64LessThanV1) => {
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
        (Architecture::Aarch64, None, ResolvedConditionalBranchPredicate::U64LessThanV1) => {
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
        (Architecture::X86_64, None, ResolvedConditionalBranchPredicate::I64LessThanV1) => {
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
        (Architecture::Aarch64, None, ResolvedConditionalBranchPredicate::I64LessThanV1) => {
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

#[allow(clippy::too_many_arguments)]
fn validate_fused_footprint(
    instruction: SelectedInstructionId,
    block: &SelectedBlock,
    source_read: &QualifiedPhysicalRead,
    action: &Aarch64CbnzFusionAction,
    physical: &ValidatedPhysicalRegisterModel,
    register_reads: &[omega_register_model::RegisterViewId],
    effects: &MachineEncodedEffects,
    original: &MachineEncodedEffects,
) -> Result<(), OptimizedResolvedSelectedFormLayoutError> {
    let view = physical
        .model()
        .views
        .iter()
        .find(|view| view.id == source_read.view)
        .ok_or(OptimizedResolvedSelectedFormLayoutError::BranchEffectsMismatch(instruction))?;
    let SelectedTerminator::ConditionalBranch {
        when_nonzero,
        when_zero,
        ..
    } = &block.terminator
    else {
        return unexpected(instruction);
    };
    if register_reads != [source_read.view]
        || source_read.units != view.units
        || &action.source_read != source_read
        || action.when_nonzero_edge != when_nonzero.psi_edge
        || action.when_nonzero_block != when_nonzero.block
        || action.when_zero_edge != when_zero.psi_edge
        || action.when_zero_block != when_zero.block
        || !effects.external_operand_reads.is_empty()
        || !effects.external_operand_writes.is_empty()
        || effects.implicit_unit_uses != action.pc_units
        || effects.implicit_unit_defs != action.pc_units
        || !effects.implicit_unit_clobbers.is_empty()
        || effects
            .implicit_unit_uses
            .iter()
            .any(|unit| action.nzcv_units.contains(unit))
        || effects.memory != original.memory
        || effects.stack != original.stack
        || effects.trap != original.trap
        || effects.control != original.control
    {
        return Err(OptimizedResolvedSelectedFormLayoutError::BranchEffectsMismatch(instruction));
    }
    Ok(())
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
