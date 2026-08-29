use std::collections::BTreeMap;

use omega_isa_aarch64::{
    encode_aarch64_fused_compare_i64_zero_branch_nonzero_to_cbnz_form,
    encode_aarch64_selected_nonzero_branch_form,
};
use omega_isa_x86_64::encode_x86_64_selected_nonzero_branch_form;
use omega_machine_optimizer::{
    Aarch64CbnzFusionAction, Aarch64CbnzInstructionDisposition, PostAllocationMachineInstruction,
    QualifiedPhysicalRead,
};
use omega_register_model::ValidatedPhysicalRegisterModel;
use omega_selected_instructions::{
    MachineEncodedEffects, MachineSizeKnowledge, SelectedBlock, SelectedBlockId, SelectedFunction,
    SelectedInstruction, SelectedInstructionId, SelectedTerminator,
};
use omega_target::Architecture;
use psi_core::MachineId;

use crate::{
    DeferredControlEncodingReason, SelectedFormEncodingRow, SelectedFormEncodingState,
    StagedOptimizedAarch64CbnzFusion,
};

use super::error::OptimizedResolvedSelectedFormLayoutError;
use super::model::{
    ResolvedConditionalBranchEvidence, ResolvedSelectedBlockLayout, ResolvedSelectedFormRow,
    ResolvedSelectedFunctionLayout, SelectedFunctionLayoutPolicy,
};

pub(super) fn layout_function(
    architecture: Architecture,
    function: &SelectedFunction,
    pre_rows: &BTreeMap<SelectedInstructionId, &SelectedFormEncodingRow>,
    machine_rows: &BTreeMap<SelectedInstructionId, &PostAllocationMachineInstruction>,
    physical: &ValidatedPhysicalRegisterModel,
    fusion: Option<&StagedOptimizedAarch64CbnzFusion>,
) -> Result<ResolvedSelectedFunctionLayout, OptimizedResolvedSelectedFormLayoutError> {
    if function.blocks.len() == 1 {
        return layout_single_block(
            architecture,
            function,
            pre_rows,
            machine_rows,
            physical,
            fusion,
        );
    }
    if function.blocks.len() != 3 {
        return Err(
            OptimizedResolvedSelectedFormLayoutError::UnsupportedFunctionShape(function.machine),
        );
    }
    let entry = find_block(function, function.entry_block)?;
    let SelectedTerminator::ConditionalBranch {
        when_nonzero,
        when_zero,
        ..
    } = &entry.terminator
    else {
        return Err(
            OptimizedResolvedSelectedFormLayoutError::UnsupportedFunctionShape(function.machine),
        );
    };
    if when_nonzero.block == when_zero.block
        || entry.id == when_nonzero.block
        || entry.id == when_zero.block
    {
        return Err(
            OptimizedResolvedSelectedFormLayoutError::UnsupportedFunctionShape(function.machine),
        );
    }
    let zero = find_block(function, when_zero.block)?;
    let nonzero = find_block(function, when_nonzero.block)?;
    if !matches!(zero.terminator, SelectedTerminator::Return { .. })
        || !matches!(nonzero.terminator, SelectedTerminator::Return { .. })
    {
        return Err(
            OptimizedResolvedSelectedFormLayoutError::UnsupportedFunctionShape(function.machine),
        );
    }
    let ordered = [entry, zero, nonzero];
    let mut block_offsets = BTreeMap::new();
    let mut block_sizes = BTreeMap::new();
    let mut offset = 0_u64;
    for block in ordered {
        block_offsets.insert(block.id, offset);
        let start = offset;
        for instruction in block_instructions(block) {
            let pre = pre_rows.get(&instruction.id).ok_or(
                OptimizedResolvedSelectedFormLayoutError::MissingInstruction(instruction.id),
            )?;
            offset = offset
                .checked_add(planned_size(architecture, instruction, pre)?)
                .ok_or(OptimizedResolvedSelectedFormLayoutError::OffsetOverflow)?;
        }
        block_sizes.insert(block.id, offset - start);
    }
    let function_size = offset;

    let mut blocks = Vec::with_capacity(3);
    for block in ordered {
        let block_offset = block_offsets[&block.id];
        let mut instruction_offset = block_offset;
        let mut instructions = Vec::new();
        for instruction in block_instructions(block) {
            let pre = pre_rows[&instruction.id];
            let machine = machine_rows.get(&instruction.id).ok_or(
                OptimizedResolvedSelectedFormLayoutError::MissingInstruction(instruction.id),
            )?;
            if machine.alternative.key != pre.alternative {
                return Err(
                    OptimizedResolvedSelectedFormLayoutError::AlternativeMismatch(instruction.id),
                );
            }
            let (bytes, branch) = resolve_instruction(
                architecture,
                function.machine,
                block,
                instruction,
                instruction_offset,
                &block_offsets,
                machine,
                pre,
                physical,
                fusion,
            )?;
            let byte_count = u64::try_from(bytes.len())
                .map_err(|_| OptimizedResolvedSelectedFormLayoutError::OffsetOverflow)?;
            instructions.push(ResolvedSelectedFormRow {
                instruction: instruction.id,
                alternative: pre.alternative,
                offset: instruction_offset,
                bytes,
                branch,
            });
            instruction_offset = instruction_offset
                .checked_add(byte_count)
                .ok_or(OptimizedResolvedSelectedFormLayoutError::OffsetOverflow)?;
        }
        let byte_count = block_sizes[&block.id];
        if instruction_offset != block_offset + byte_count {
            return Err(OptimizedResolvedSelectedFormLayoutError::OffsetOverflow);
        }
        blocks.push(ResolvedSelectedBlockLayout {
            block: block.id,
            offset: block_offset,
            byte_count,
            instructions,
        });
    }
    Ok(ResolvedSelectedFunctionLayout {
        machine: function.machine,
        byte_count: function_size,
        blocks,
    })
}

pub(super) fn selected_layout_policy(
    selected: &omega_selected_instructions::SelectedInstructionPlan,
) -> Result<SelectedFunctionLayoutPolicy, OptimizedResolvedSelectedFormLayoutError> {
    let is_single_entry = |function: &SelectedFunction| {
        let [block] = function.blocks.as_slice() else {
            return false;
        };
        function.entry_block == block.id
            && matches!(block.terminator, SelectedTerminator::Return { .. })
    };
    let single_entry_count = selected
        .functions
        .iter()
        .filter(|function| is_single_entry(function))
        .count();
    if single_entry_count == selected.functions.len() {
        Ok(SelectedFunctionLayoutPolicy::SingleEntryBlockV1)
    } else if single_entry_count == 0 {
        Ok(SelectedFunctionLayoutPolicy::EntryThenZeroFallthroughThenNonzeroV1)
    } else {
        Err(
            OptimizedResolvedSelectedFormLayoutError::UnsupportedFunctionShape(
                selected.functions[single_entry_count].machine,
            ),
        )
    }
}

fn layout_single_block(
    architecture: Architecture,
    function: &SelectedFunction,
    pre_rows: &BTreeMap<SelectedInstructionId, &SelectedFormEncodingRow>,
    machine_rows: &BTreeMap<SelectedInstructionId, &PostAllocationMachineInstruction>,
    physical: &ValidatedPhysicalRegisterModel,
    fusion: Option<&StagedOptimizedAarch64CbnzFusion>,
) -> Result<ResolvedSelectedFunctionLayout, OptimizedResolvedSelectedFormLayoutError> {
    let [block] = function.blocks.as_slice() else {
        return Err(
            OptimizedResolvedSelectedFormLayoutError::UnsupportedFunctionShape(function.machine),
        );
    };
    if function.entry_block != block.id
        || !matches!(block.terminator, SelectedTerminator::Return { .. })
        || fusion.is_some()
    {
        return Err(
            OptimizedResolvedSelectedFormLayoutError::UnsupportedFunctionShape(function.machine),
        );
    }
    let mut offset = 0_u64;
    let mut instructions = Vec::new();
    let block_offsets = BTreeMap::from([(block.id, 0)]);
    for instruction in block_instructions(block) {
        let pre = pre_rows
            .get(&instruction.id)
            .ok_or(OptimizedResolvedSelectedFormLayoutError::MissingInstruction(instruction.id))?;
        let machine = machine_rows
            .get(&instruction.id)
            .ok_or(OptimizedResolvedSelectedFormLayoutError::MissingInstruction(instruction.id))?;
        if machine.alternative.key != pre.alternative {
            return Err(
                OptimizedResolvedSelectedFormLayoutError::AlternativeMismatch(instruction.id),
            );
        }
        let (bytes, branch) = resolve_instruction(
            architecture,
            function.machine,
            block,
            instruction,
            offset,
            &block_offsets,
            machine,
            pre,
            physical,
            None,
        )?;
        if branch.is_some() {
            return Err(
                OptimizedResolvedSelectedFormLayoutError::UnsupportedFunctionShape(
                    function.machine,
                ),
            );
        }
        let byte_count = u64::try_from(bytes.len())
            .map_err(|_| OptimizedResolvedSelectedFormLayoutError::OffsetOverflow)?;
        instructions.push(ResolvedSelectedFormRow {
            instruction: instruction.id,
            alternative: pre.alternative,
            offset,
            bytes,
            branch: None,
        });
        offset = offset
            .checked_add(byte_count)
            .ok_or(OptimizedResolvedSelectedFormLayoutError::OffsetOverflow)?;
    }
    Ok(ResolvedSelectedFunctionLayout {
        machine: function.machine,
        byte_count: offset,
        blocks: vec![ResolvedSelectedBlockLayout {
            block: block.id,
            offset: 0,
            byte_count: offset,
            instructions,
        }],
    })
}

#[allow(clippy::too_many_arguments)]
fn resolve_instruction(
    architecture: Architecture,
    function: MachineId,
    block: &SelectedBlock,
    instruction: &SelectedInstruction,
    instruction_offset: u64,
    block_offsets: &BTreeMap<SelectedBlockId, u64>,
    machine: &PostAllocationMachineInstruction,
    pre: &SelectedFormEncodingRow,
    physical: &ValidatedPhysicalRegisterModel,
    fusion: Option<&StagedOptimizedAarch64CbnzFusion>,
) -> Result<
    (Vec<u8>, Option<Box<ResolvedConditionalBranchEvidence>>),
    OptimizedResolvedSelectedFormLayoutError,
> {
    match (&pre.machine_disposition, &pre.state) {
        (
            Aarch64CbnzInstructionDisposition::RetainedV1,
            SelectedFormEncodingState::Encoded { bytes, .. },
        ) => Ok((bytes.clone(), None)),
        (
            Aarch64CbnzInstructionDisposition::RetainedV1,
            SelectedFormEncodingState::DeferredControl {
                reason: DeferredControlEncodingReason::RequiresResolvedBranchLayout,
            },
        ) => resolve_branch(
            architecture,
            block,
            instruction,
            instruction_offset,
            block_offsets,
            machine,
            physical,
            None,
        ),
        (
            Aarch64CbnzInstructionDisposition::ElidedCompareI64ZeroV1 { consumer },
            SelectedFormEncodingState::Encoded { .. },
        ) => {
            let action = fusion_action(fusion, function, block.id, instruction.id, *consumer)?;
            if architecture != Architecture::Aarch64
                || !matches!(
                    instruction.kind,
                    omega_selected_instructions::SelectedInstructionKind::CompareI64Zero
                )
                || action.compare != instruction.id
                || action.branch != *consumer
            {
                return Err(
                    OptimizedResolvedSelectedFormLayoutError::UnexpectedEncodingState(
                        instruction.id,
                    ),
                );
            }
            Ok((Vec::new(), None))
        }
        (
            Aarch64CbnzInstructionDisposition::FusedBranchNonZeroToCbnzV1 {
                compare,
                source_read,
            },
            SelectedFormEncodingState::DeferredControl {
                reason: DeferredControlEncodingReason::RequiresResolvedBranchLayout,
            },
        ) => {
            let action = fusion_action(fusion, function, block.id, *compare, instruction.id)?;
            if architecture != Architecture::Aarch64 || &action.source_read != source_read {
                return Err(
                    OptimizedResolvedSelectedFormLayoutError::UnexpectedEncodingState(
                        instruction.id,
                    ),
                );
            }
            resolve_branch(
                architecture,
                block,
                instruction,
                instruction_offset,
                block_offsets,
                machine,
                physical,
                Some((source_read, action)),
            )
        }
        _ => Err(OptimizedResolvedSelectedFormLayoutError::UnexpectedEncodingState(instruction.id)),
    }
}

fn resolve_branch(
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
    let SelectedTerminator::ConditionalBranch {
        instruction: terminator,
        when_nonzero,
        when_zero,
    } = &block.terminator
    else {
        return Err(
            OptimizedResolvedSelectedFormLayoutError::UnexpectedEncodingState(instruction.id),
        );
    };
    if terminator.id != instruction.id {
        return Err(
            OptimizedResolvedSelectedFormLayoutError::UnexpectedEncodingState(instruction.id),
        );
    }
    let nonzero_offset = *block_offsets.get(&when_nonzero.block).ok_or(
        OptimizedResolvedSelectedFormLayoutError::BranchFallthroughMismatch(instruction.id),
    )?;
    let zero_offset = *block_offsets.get(&when_zero.block).ok_or(
        OptimizedResolvedSelectedFormLayoutError::BranchFallthroughMismatch(instruction.id),
    )?;
    let branch_size = branch_size(architecture);
    let branch_end = instruction_offset
        .checked_add(branch_size)
        .ok_or(OptimizedResolvedSelectedFormLayoutError::OffsetOverflow)?;
    if zero_offset != branch_end {
        return Err(
            OptimizedResolvedSelectedFormLayoutError::BranchFallthroughMismatch(instruction.id),
        );
    }
    let displacement = match architecture {
        Architecture::X86_64 => checked_delta(nonzero_offset, branch_end)?,
        Architecture::Aarch64 => checked_delta(nonzero_offset, instruction_offset)?,
    };
    let (bytes, register_reads, effects) = match (architecture, fused) {
        (Architecture::Aarch64, Some((source_read, _))) => {
            let encoded = encode_aarch64_fused_compare_i64_zero_branch_nonzero_to_cbnz_form(
                physical,
                source_read.view,
                displacement,
            )
            .map_err(OptimizedResolvedSelectedFormLayoutError::Aarch64)?;
            (
                encoded.bytes().to_vec(),
                encoded.footprint().register_reads.clone(),
                encoded.footprint().encoded.clone(),
            )
        }
        (Architecture::X86_64, Some(_)) => {
            return Err(
                OptimizedResolvedSelectedFormLayoutError::UnexpectedEncodingState(instruction.id),
            );
        }
        (Architecture::X86_64, None) => {
            let encoded = encode_x86_64_selected_nonzero_branch_form(
                physical,
                machine.alternative.key,
                displacement,
            )
            .map_err(OptimizedResolvedSelectedFormLayoutError::X86_64)?;
            (
                encoded.bytes().to_vec(),
                encoded.footprint().register_reads.clone(),
                encoded.footprint().encoded.clone(),
            )
        }
        (Architecture::Aarch64, None) => {
            let encoded = encode_aarch64_selected_nonzero_branch_form(
                physical,
                machine.alternative.key,
                displacement,
            )
            .map_err(OptimizedResolvedSelectedFormLayoutError::Aarch64)?;
            (
                encoded.bytes().to_vec(),
                encoded.footprint().register_reads.clone(),
                encoded.footprint().encoded.clone(),
            )
        }
    };
    if let Some((source_read, action)) = fused {
        validate_fused_branch_footprint(
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
    if u64::try_from(bytes.len()).ok() != Some(branch_size) {
        return Err(OptimizedResolvedSelectedFormLayoutError::BranchSizeMismatch(instruction.id));
    }
    let declared_size_matches = match machine.alternative.size {
        MachineSizeKnowledge::ExactBytes(expected) => u64::from(expected) == branch_size,
        MachineSizeKnowledge::EncoderResolved {
            minimum_bytes,
            maximum_bytes,
        } => {
            branch_size >= u64::from(minimum_bytes)
                && maximum_bytes.is_none_or(|maximum| branch_size <= u64::from(maximum))
        }
    };
    if !declared_size_matches {
        return Err(OptimizedResolvedSelectedFormLayoutError::BranchSizeMismatch(instruction.id));
    }
    Ok((
        bytes,
        Some(Box::new(ResolvedConditionalBranchEvidence {
            source_block: block.id,
            when_nonzero_edge: when_nonzero.psi_edge,
            when_nonzero_block: when_nonzero.block,
            when_nonzero_offset: nonzero_offset,
            when_zero_edge: when_zero.psi_edge,
            when_zero_block: when_zero.block,
            when_zero_offset: zero_offset,
            byte_displacement: displacement,
            decoded_register_reads: register_reads,
            decoded_effects: effects,
        })),
    ))
}

fn find_block(
    function: &SelectedFunction,
    id: SelectedBlockId,
) -> Result<&SelectedBlock, OptimizedResolvedSelectedFormLayoutError> {
    function
        .blocks
        .iter()
        .find(|block| block.id == id)
        .ok_or(OptimizedResolvedSelectedFormLayoutError::UnsupportedFunctionShape(function.machine))
}

pub(super) fn block_instructions(block: &SelectedBlock) -> Vec<&SelectedInstruction> {
    block
        .instructions
        .iter()
        .chain(std::iter::once(match &block.terminator {
            SelectedTerminator::ConditionalBranch { instruction, .. }
            | SelectedTerminator::Return { instruction, .. } => instruction,
        }))
        .collect()
}

fn planned_size(
    architecture: Architecture,
    instruction: &SelectedInstruction,
    row: &SelectedFormEncodingRow,
) -> Result<u64, OptimizedResolvedSelectedFormLayoutError> {
    match &row.machine_disposition {
        Aarch64CbnzInstructionDisposition::ElidedCompareI64ZeroV1 { .. } => {
            if architecture == Architecture::Aarch64
                && matches!(
                    instruction.kind,
                    omega_selected_instructions::SelectedInstructionKind::CompareI64Zero
                )
                && matches!(row.state, SelectedFormEncodingState::Encoded { .. })
            {
                return Ok(0);
            }
            return Err(
                OptimizedResolvedSelectedFormLayoutError::UnexpectedEncodingState(instruction.id),
            );
        }
        Aarch64CbnzInstructionDisposition::FusedBranchNonZeroToCbnzV1 { .. } => {
            if architecture == Architecture::Aarch64
                && matches!(row.state, SelectedFormEncodingState::DeferredControl { .. })
            {
                return Ok(branch_size(architecture));
            }
            return Err(
                OptimizedResolvedSelectedFormLayoutError::UnexpectedEncodingState(instruction.id),
            );
        }
        Aarch64CbnzInstructionDisposition::RetainedV1 => {}
    }
    match &row.state {
        SelectedFormEncodingState::Encoded { bytes, .. } => u64::try_from(bytes.len())
            .map_err(|_| OptimizedResolvedSelectedFormLayoutError::OffsetOverflow),
        SelectedFormEncodingState::DeferredControl {
            reason: DeferredControlEncodingReason::RequiresResolvedBranchLayout,
        } => {
            if row.instruction != instruction.id {
                return Err(
                    OptimizedResolvedSelectedFormLayoutError::MissingInstruction(instruction.id),
                );
            }
            Ok(branch_size(architecture))
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

fn fusion_action<'fusion>(
    fusion: Option<&'fusion StagedOptimizedAarch64CbnzFusion>,
    machine: MachineId,
    block: SelectedBlockId,
    compare: SelectedInstructionId,
    branch: SelectedInstructionId,
) -> Result<&'fusion Aarch64CbnzFusionAction, OptimizedResolvedSelectedFormLayoutError> {
    fusion
        .and_then(|fusion| {
            fusion.fusion().plan().actions.iter().find(|action| {
                action.machine == machine
                    && action.block == block
                    && action.compare == compare
                    && action.branch == branch
            })
        })
        .ok_or(OptimizedResolvedSelectedFormLayoutError::UnexpectedEncodingState(branch))
}

#[allow(clippy::too_many_arguments)]
fn validate_fused_branch_footprint(
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
        return Err(OptimizedResolvedSelectedFormLayoutError::UnexpectedEncodingState(instruction));
    };
    if register_reads != [source_read.view]
        || source_read.units != view.units
        || &action.source_read != source_read
        || action.when_nonzero_edge != when_nonzero.psi_edge
        || action.when_nonzero_block != when_nonzero.block
        || action.when_zero_edge != when_zero.psi_edge
        || action.when_zero_block != when_zero.block
        || effects.external_operand_reads != []
        || effects.external_operand_writes != []
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
