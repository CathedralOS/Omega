use std::collections::BTreeMap;

use omega_isa_x86_64::{
    encode_x86_64_selected_nonzero_branch_form, encode_x86_64_selected_short_nonzero_branch_form,
    validate_x86_64_selected_nonzero_branch_form,
    validate_x86_64_selected_short_nonzero_branch_form,
};
use omega_optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};
use omega_register_model::ValidatedPhysicalRegisterModel;
use omega_selected_instructions::{SelectedBlockId, SelectedInstructionId};

use crate::{
    ResolvedSelectedFormRow, ResolvedSelectedFunctionLayout,
    StagedOptimizedResolvedSelectedFormLayout,
};

use super::{
    error::{OptimizedX86BranchRelaxationError, X86BranchRelaxationWorkAxis},
    identity::{RevisionRoots, artifact_identity, revision_identity},
    model::{
        StagedOptimizedX86BranchRelaxation, X86BranchRelaxationAction, X86BranchRelaxationAttempt,
        X86BranchRelaxationAttemptOutcome, X86BranchRelaxationPolicy,
    },
};

pub(super) fn compute_relaxation(
    source: &StagedOptimizedResolvedSelectedFormLayout,
    physical: &ValidatedPhysicalRegisterModel,
    budget: OptimizationWorkBudget,
) -> Result<StagedOptimizedX86BranchRelaxation, OptimizedX86BranchRelaxationError> {
    let mut functions = source.functions().to_vec();
    let mut attempts = Vec::new();
    let mut actions = Vec::new();
    let mut usage = OptimizationWorkUsage::default();
    let roots = RevisionRoots {
        source: source.identity(),
        selected: source.selected(),
        machine: source.machine(),
        pre_layout: source.pre_layout(),
        target: source.target(),
    };
    loop {
        charge(
            &mut usage.iterations,
            budget.iterations(),
            X86BranchRelaxationWorkAxis::Iterations,
        )?;
        let iteration = usage.iterations;
        let input = revision_identity(roots, &functions);
        let previous_bytes = total_bytes(&functions)?;
        let mut selected_action = None;
        'scan: for (function_index, function) in functions.iter().enumerate() {
            for (block_index, block) in function.blocks.iter().enumerate() {
                for (instruction_index, row) in block.instructions.iter().enumerate() {
                    let Some(branch) = row.branch.as_deref() else {
                        continue;
                    };
                    charge(
                        &mut usage.rule_evaluations,
                        budget.rule_evaluations(),
                        X86BranchRelaxationWorkAxis::RuleEvaluations,
                    )?;
                    let (outcome, short_displacement) = inspect_production_branch(
                        &functions[function_index],
                        block_index,
                        instruction_index,
                        physical,
                    )?;
                    attempts.push(X86BranchRelaxationAttempt {
                        iteration,
                        input,
                        instruction: row.instruction,
                        offset: row.offset,
                        byte_displacement: branch.byte_displacement,
                        encoded_bytes: u8::try_from(row.bytes.len())
                            .map_err(|_| OptimizedX86BranchRelaxationError::OffsetOverflow)?,
                        outcome,
                    });
                    if outcome == X86BranchRelaxationAttemptOutcome::SelectedForRelaxation {
                        charge(
                            &mut usage.candidates,
                            budget.candidates(),
                            X86BranchRelaxationWorkAxis::Candidates,
                        )?;
                        charge(
                            &mut usage.validation_steps,
                            budget.validation_steps(),
                            X86BranchRelaxationWorkAxis::ValidationSteps,
                        )?;
                        let short_displacement = short_displacement.ok_or(
                            OptimizedX86BranchRelaxationError::MalformedBranch(row.instruction),
                        )?;
                        selected_action = Some((
                            function_index,
                            block_index,
                            instruction_index,
                            short_displacement,
                        ));
                        break 'scan;
                    }
                }
            }
        }
        let Some((function_index, block_index, instruction_index, displacement)) = selected_action
        else {
            break;
        };
        charge(
            &mut usage.commits,
            budget.commits(),
            X86BranchRelaxationWorkAxis::Commits,
        )?;
        let old =
            functions[function_index].blocks[block_index].instructions[instruction_index].clone();
        let encoded = encode_x86_64_selected_short_nonzero_branch_form(
            physical,
            old.alternative,
            displacement,
        )
        .map_err(OptimizedX86BranchRelaxationError::X86_64)?;
        functions[function_index].blocks[block_index].instructions[instruction_index].bytes =
            encoded.bytes().to_vec();
        reflow_production_functions(&mut functions, physical)?;
        let new = &functions[function_index].blocks[block_index].instructions[instruction_index];
        let current_bytes = total_bytes(&functions)?;
        if previous_bytes.checked_sub(current_bytes) != Some(4) {
            return Err(OptimizedX86BranchRelaxationError::NonDecreasingByteMeasure);
        }
        let output = revision_identity(roots, &functions);
        let old_displacement = old
            .branch
            .as_deref()
            .ok_or(OptimizedX86BranchRelaxationError::MalformedBranch(
                old.instruction,
            ))?
            .byte_displacement;
        let new_displacement = new
            .branch
            .as_deref()
            .ok_or(OptimizedX86BranchRelaxationError::MalformedBranch(
                new.instruction,
            ))?
            .byte_displacement;
        actions.push(X86BranchRelaxationAction {
            iteration,
            input,
            output,
            instruction: old.instruction,
            old_offset: old.offset,
            new_offset: new.offset,
            old_displacement,
            new_displacement,
            old_bytes: old.bytes,
            new_bytes: new.bytes.clone(),
        });
    }
    finish_artifact(source, budget, usage, attempts, actions, functions)
}

pub(super) fn replay_relaxation(
    source: &StagedOptimizedResolvedSelectedFormLayout,
    physical: &ValidatedPhysicalRegisterModel,
    budget: OptimizationWorkBudget,
) -> Result<StagedOptimizedX86BranchRelaxation, OptimizedX86BranchRelaxationError> {
    let roots = RevisionRoots {
        source: source.identity(),
        selected: source.selected(),
        machine: source.machine(),
        pre_layout: source.pre_layout(),
        target: source.target(),
    };
    let mut functions = source.functions().to_vec();
    let mut attempts = Vec::new();
    let mut actions = Vec::new();
    let mut usage = OptimizationWorkUsage::default();
    loop {
        replay_charge(
            &mut usage.iterations,
            budget.iterations(),
            X86BranchRelaxationWorkAxis::Iterations,
        )?;
        let iteration = usage.iterations;
        let input = revision_identity(roots, &functions);
        let before = total_bytes(&functions)?;
        let locations = ordered_branch_locations(&functions);
        let mut chosen = None;
        for (function_index, block_index, instruction_index) in locations {
            replay_charge(
                &mut usage.rule_evaluations,
                budget.rule_evaluations(),
                X86BranchRelaxationWorkAxis::RuleEvaluations,
            )?;
            let row =
                &functions[function_index].blocks[block_index].instructions[instruction_index];
            let branch =
                row.branch
                    .as_deref()
                    .ok_or(OptimizedX86BranchRelaxationError::MalformedBranch(
                        row.instruction,
                    ))?;
            let (outcome, displacement) = replay_inspect_branch(
                &functions[function_index],
                block_index,
                instruction_index,
                physical,
            )?;
            attempts.push(X86BranchRelaxationAttempt {
                iteration,
                input,
                instruction: row.instruction,
                offset: row.offset,
                byte_displacement: branch.byte_displacement,
                encoded_bytes: u8::try_from(row.bytes.len())
                    .map_err(|_| OptimizedX86BranchRelaxationError::OffsetOverflow)?,
                outcome,
            });
            if outcome == X86BranchRelaxationAttemptOutcome::SelectedForRelaxation {
                replay_charge(
                    &mut usage.candidates,
                    budget.candidates(),
                    X86BranchRelaxationWorkAxis::Candidates,
                )?;
                replay_charge(
                    &mut usage.validation_steps,
                    budget.validation_steps(),
                    X86BranchRelaxationWorkAxis::ValidationSteps,
                )?;
                let displacement = displacement.ok_or(
                    OptimizedX86BranchRelaxationError::MalformedBranch(row.instruction),
                )?;
                chosen = Some((function_index, block_index, instruction_index, displacement));
                break;
            }
        }
        let Some((function_index, block_index, instruction_index, displacement)) = chosen else {
            break;
        };
        replay_charge(
            &mut usage.commits,
            budget.commits(),
            X86BranchRelaxationWorkAxis::Commits,
        )?;
        let old =
            functions[function_index].blocks[block_index].instructions[instruction_index].clone();
        let bytes = [0x75, displacement as i8 as u8];
        validate_x86_64_selected_short_nonzero_branch_form(
            physical,
            old.alternative,
            displacement,
            &bytes,
        )
        .map_err(OptimizedX86BranchRelaxationError::X86_64)?;
        functions[function_index].blocks[block_index].instructions[instruction_index].bytes =
            bytes.to_vec();
        reflow_replay_functions(&mut functions, physical)?;
        if before.checked_sub(total_bytes(&functions)?) != Some(4) {
            return Err(OptimizedX86BranchRelaxationError::NonDecreasingByteMeasure);
        }
        let new = &functions[function_index].blocks[block_index].instructions[instruction_index];
        let output = revision_identity(roots, &functions);
        actions.push(X86BranchRelaxationAction {
            iteration,
            input,
            output,
            instruction: old.instruction,
            old_offset: old.offset,
            new_offset: new.offset,
            old_displacement: old
                .branch
                .as_deref()
                .ok_or(OptimizedX86BranchRelaxationError::MalformedBranch(
                    old.instruction,
                ))?
                .byte_displacement,
            new_displacement: new
                .branch
                .as_deref()
                .ok_or(OptimizedX86BranchRelaxationError::MalformedBranch(
                    new.instruction,
                ))?
                .byte_displacement,
            old_bytes: old.bytes,
            new_bytes: new.bytes.clone(),
        });
    }
    finish_artifact(source, budget, usage, attempts, actions, functions)
}

fn finish_artifact(
    source: &StagedOptimizedResolvedSelectedFormLayout,
    budget: OptimizationWorkBudget,
    usage: OptimizationWorkUsage,
    attempts: Vec<X86BranchRelaxationAttempt>,
    actions: Vec<X86BranchRelaxationAction>,
    functions: Vec<ResolvedSelectedFunctionLayout>,
) -> Result<StagedOptimizedX86BranchRelaxation, OptimizedX86BranchRelaxationError> {
    let roots = RevisionRoots {
        source: source.identity(),
        selected: source.selected(),
        machine: source.machine(),
        pre_layout: source.pre_layout(),
        target: source.target(),
    };
    let output_revision = revision_identity(roots, &functions);
    let layout = source.with_replayed_functions(functions);
    let output = layout.identity();
    let policy = X86BranchRelaxationPolicy::X86RelaxConditionalBranchesToRel8V1;
    let identity = artifact_identity(
        roots,
        policy,
        budget,
        usage,
        output,
        output_revision,
        &attempts,
        &actions,
        layout.functions(),
    );
    Ok(StagedOptimizedX86BranchRelaxation {
        source: source.identity(),
        selected: source.selected(),
        machine: source.machine(),
        pre_layout: source.pre_layout(),
        target: source.target(),
        policy,
        budget,
        usage,
        output,
        output_revision,
        identity,
        attempts,
        actions,
        layout,
    })
}

pub(super) fn inspect_production_branch(
    function: &ResolvedSelectedFunctionLayout,
    block_index: usize,
    instruction_index: usize,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<(X86BranchRelaxationAttemptOutcome, Option<i64>), OptimizedX86BranchRelaxationError> {
    let row = &function.blocks[block_index].instructions[instruction_index];
    let branch =
        row.branch
            .as_deref()
            .ok_or(OptimizedX86BranchRelaxationError::MalformedBranch(
                row.instruction,
            ))?;
    if row.bytes.len() == 2 && row.bytes[0] == 0x75 {
        validate_x86_64_selected_short_nonzero_branch_form(
            physical,
            row.alternative,
            branch.byte_displacement,
            &row.bytes,
        )
        .map_err(OptimizedX86BranchRelaxationError::X86_64)?;
        return Ok((X86BranchRelaxationAttemptOutcome::AlreadyShort, None));
    }
    validate_x86_64_selected_nonzero_branch_form(
        physical,
        row.alternative,
        branch.byte_displacement,
        &row.bytes,
    )
    .map_err(OptimizedX86BranchRelaxationError::X86_64)?;
    let displacement = prospective_short_displacement(function, row, branch.when_nonzero_block)?;
    if i8::try_from(displacement).is_ok() {
        Ok((
            X86BranchRelaxationAttemptOutcome::SelectedForRelaxation,
            Some(displacement),
        ))
    } else {
        Ok((
            X86BranchRelaxationAttemptOutcome::NearDisplacementOutsideI8,
            None,
        ))
    }
}

pub(super) fn replay_inspect_branch(
    function: &ResolvedSelectedFunctionLayout,
    block_index: usize,
    instruction_index: usize,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<(X86BranchRelaxationAttemptOutcome, Option<i64>), OptimizedX86BranchRelaxationError> {
    let row = function
        .blocks
        .get(block_index)
        .and_then(|block| block.instructions.get(instruction_index))
        .ok_or(OptimizedX86BranchRelaxationError::OffsetOverflow)?;
    let branch =
        row.branch
            .as_deref()
            .ok_or(OptimizedX86BranchRelaxationError::MalformedBranch(
                row.instruction,
            ))?;
    match row.bytes.as_slice() {
        [0x75, displacement] => {
            let decoded = i64::from(*displacement as i8);
            if decoded != branch.byte_displacement {
                return Err(OptimizedX86BranchRelaxationError::MalformedBranch(
                    row.instruction,
                ));
            }
            validate_x86_64_selected_short_nonzero_branch_form(
                physical,
                row.alternative,
                decoded,
                &row.bytes,
            )
            .map_err(OptimizedX86BranchRelaxationError::X86_64)?;
            Ok((X86BranchRelaxationAttemptOutcome::AlreadyShort, None))
        }
        [0x0f, 0x85, ..] if row.bytes.len() == 6 => {
            validate_x86_64_selected_nonzero_branch_form(
                physical,
                row.alternative,
                branch.byte_displacement,
                &row.bytes,
            )
            .map_err(OptimizedX86BranchRelaxationError::X86_64)?;
            let displacement =
                prospective_short_displacement(function, row, branch.when_nonzero_block)?;
            if (-128..=127).contains(&displacement) {
                Ok((
                    X86BranchRelaxationAttemptOutcome::SelectedForRelaxation,
                    Some(displacement),
                ))
            } else {
                Ok((
                    X86BranchRelaxationAttemptOutcome::NearDisplacementOutsideI8,
                    None,
                ))
            }
        }
        _ => Err(OptimizedX86BranchRelaxationError::MalformedBranch(
            row.instruction,
        )),
    }
}

fn prospective_short_displacement(
    function: &ResolvedSelectedFunctionLayout,
    row: &ResolvedSelectedFormRow,
    target: SelectedBlockId,
) -> Result<i64, OptimizedX86BranchRelaxationError> {
    let target_offset = function
        .blocks
        .iter()
        .find(|block| block.block == target)
        .map(|block| block.offset)
        .ok_or(OptimizedX86BranchRelaxationError::MissingTargetBlock(
            target,
        ))?;
    let shifted_target = if target_offset > row.offset {
        target_offset
            .checked_sub(4)
            .ok_or(OptimizedX86BranchRelaxationError::OffsetOverflow)?
    } else {
        target_offset
    };
    checked_delta(
        shifted_target,
        row.offset
            .checked_add(2)
            .ok_or(OptimizedX86BranchRelaxationError::OffsetOverflow)?,
    )
}

pub(super) fn reflow_production_functions(
    functions: &mut [ResolvedSelectedFunctionLayout],
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<(), OptimizedX86BranchRelaxationError> {
    for function in functions {
        let offsets = assign_dense_offsets(function)?;
        for block in &mut function.blocks {
            for row in &mut block.instructions {
                let Some(branch) = row.branch.as_mut() else {
                    continue;
                };
                rewrite_branch_offsets(
                    branch,
                    row.offset,
                    row.bytes.len(),
                    &offsets,
                    row.instruction,
                )?;
                let encoded = if row.bytes.len() == 2 {
                    encode_x86_64_selected_short_nonzero_branch_form(
                        physical,
                        row.alternative,
                        branch.byte_displacement,
                    )
                } else {
                    encode_x86_64_selected_nonzero_branch_form(
                        physical,
                        row.alternative,
                        branch.byte_displacement,
                    )
                }
                .map_err(OptimizedX86BranchRelaxationError::X86_64)?;
                if encoded.footprint().encoded != branch.decoded_effects {
                    return Err(OptimizedX86BranchRelaxationError::BranchEffectsMismatch(
                        row.instruction,
                    ));
                }
                row.bytes = encoded.bytes().to_vec();
            }
        }
    }
    Ok(())
}

pub(super) fn reflow_replay_functions(
    functions: &mut [ResolvedSelectedFunctionLayout],
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<(), OptimizedX86BranchRelaxationError> {
    for function in functions {
        let mut next = 0_u64;
        let mut offsets = BTreeMap::new();
        for block in &mut function.blocks {
            block.offset = next;
            offsets.insert(block.block, next);
            let mut local = next;
            for row in &mut block.instructions {
                row.offset = local;
                local = local
                    .checked_add(
                        u64::try_from(row.bytes.len())
                            .map_err(|_| OptimizedX86BranchRelaxationError::OffsetOverflow)?,
                    )
                    .ok_or(OptimizedX86BranchRelaxationError::OffsetOverflow)?;
            }
            block.byte_count = local - next;
            next = local;
        }
        function.byte_count = next;
        for block in &mut function.blocks {
            for row in &mut block.instructions {
                let Some(branch) = row.branch.as_mut() else {
                    continue;
                };
                let nonzero = offsets.get(&branch.when_nonzero_block).copied().ok_or(
                    OptimizedX86BranchRelaxationError::MissingTargetBlock(
                        branch.when_nonzero_block,
                    ),
                )?;
                let zero = offsets.get(&branch.when_zero_block).copied().ok_or(
                    OptimizedX86BranchRelaxationError::MissingTargetBlock(branch.when_zero_block),
                )?;
                let end = row
                    .offset
                    .checked_add(
                        u64::try_from(row.bytes.len())
                            .map_err(|_| OptimizedX86BranchRelaxationError::OffsetOverflow)?,
                    )
                    .ok_or(OptimizedX86BranchRelaxationError::OffsetOverflow)?;
                if zero != end {
                    return Err(
                        OptimizedX86BranchRelaxationError::BranchFallthroughMismatch(
                            row.instruction,
                        ),
                    );
                }
                branch.when_nonzero_offset = nonzero;
                branch.when_zero_offset = zero;
                branch.byte_displacement = checked_delta(nonzero, end)?;
                if row.bytes.len() == 2 {
                    let bytes = [0x75, branch.byte_displacement as i8 as u8];
                    let decoded = validate_x86_64_selected_short_nonzero_branch_form(
                        physical,
                        row.alternative,
                        branch.byte_displacement,
                        &bytes,
                    )
                    .map_err(OptimizedX86BranchRelaxationError::X86_64)?;
                    if decoded.footprint().encoded != branch.decoded_effects {
                        return Err(OptimizedX86BranchRelaxationError::BranchEffectsMismatch(
                            row.instruction,
                        ));
                    }
                    row.bytes = bytes.to_vec();
                } else {
                    let mut bytes = vec![0x0f, 0x85];
                    let displacement = i32::try_from(branch.byte_displacement).map_err(|_| {
                        OptimizedX86BranchRelaxationError::MalformedBranch(row.instruction)
                    })?;
                    bytes.extend_from_slice(&displacement.to_le_bytes());
                    let decoded = validate_x86_64_selected_nonzero_branch_form(
                        physical,
                        row.alternative,
                        branch.byte_displacement,
                        &bytes,
                    )
                    .map_err(OptimizedX86BranchRelaxationError::X86_64)?;
                    if decoded.footprint().encoded != branch.decoded_effects {
                        return Err(OptimizedX86BranchRelaxationError::BranchEffectsMismatch(
                            row.instruction,
                        ));
                    }
                    row.bytes = bytes;
                }
            }
        }
    }
    Ok(())
}

fn assign_dense_offsets(
    function: &mut ResolvedSelectedFunctionLayout,
) -> Result<BTreeMap<SelectedBlockId, u64>, OptimizedX86BranchRelaxationError> {
    let mut offsets = BTreeMap::new();
    let mut offset = 0_u64;
    for block in &mut function.blocks {
        block.offset = offset;
        offsets.insert(block.block, offset);
        let start = offset;
        for row in &mut block.instructions {
            row.offset = offset;
            offset = offset
                .checked_add(
                    u64::try_from(row.bytes.len())
                        .map_err(|_| OptimizedX86BranchRelaxationError::OffsetOverflow)?,
                )
                .ok_or(OptimizedX86BranchRelaxationError::OffsetOverflow)?;
        }
        block.byte_count = offset - start;
    }
    function.byte_count = offset;
    Ok(offsets)
}

fn rewrite_branch_offsets(
    branch: &mut crate::ResolvedConditionalBranchEvidence,
    instruction_offset: u64,
    instruction_size: usize,
    offsets: &BTreeMap<SelectedBlockId, u64>,
    instruction: SelectedInstructionId,
) -> Result<(), OptimizedX86BranchRelaxationError> {
    let nonzero = *offsets.get(&branch.when_nonzero_block).ok_or(
        OptimizedX86BranchRelaxationError::MissingTargetBlock(branch.when_nonzero_block),
    )?;
    let zero = *offsets.get(&branch.when_zero_block).ok_or(
        OptimizedX86BranchRelaxationError::MissingTargetBlock(branch.when_zero_block),
    )?;
    let end = instruction_offset
        .checked_add(
            u64::try_from(instruction_size)
                .map_err(|_| OptimizedX86BranchRelaxationError::OffsetOverflow)?,
        )
        .ok_or(OptimizedX86BranchRelaxationError::OffsetOverflow)?;
    if zero != end {
        return Err(OptimizedX86BranchRelaxationError::BranchFallthroughMismatch(instruction));
    }
    branch.when_nonzero_offset = nonzero;
    branch.when_zero_offset = zero;
    branch.byte_displacement = checked_delta(nonzero, end)?;
    Ok(())
}

fn ordered_branch_locations(
    functions: &[ResolvedSelectedFunctionLayout],
) -> Vec<(usize, usize, usize)> {
    let mut locations = Vec::new();
    for (function_index, function) in functions.iter().enumerate() {
        for (block_index, block) in function.blocks.iter().enumerate() {
            for (instruction_index, row) in block.instructions.iter().enumerate() {
                if row.branch.is_some() {
                    locations.push((function_index, block_index, instruction_index));
                }
            }
        }
    }
    locations
}

fn total_bytes(
    functions: &[ResolvedSelectedFunctionLayout],
) -> Result<u64, OptimizedX86BranchRelaxationError> {
    functions.iter().try_fold(0_u64, |total, function| {
        total
            .checked_add(function.byte_count)
            .ok_or(OptimizedX86BranchRelaxationError::OffsetOverflow)
    })
}

pub(super) fn charge(
    usage: &mut u64,
    limit: u64,
    axis: X86BranchRelaxationWorkAxis,
) -> Result<(), OptimizedX86BranchRelaxationError> {
    *usage = usage
        .checked_add(1)
        .ok_or(OptimizedX86BranchRelaxationError::BudgetExceeded(axis))?;
    if *usage > limit {
        return Err(OptimizedX86BranchRelaxationError::BudgetExceeded(axis));
    }
    Ok(())
}

fn replay_charge(
    usage: &mut u64,
    limit: u64,
    axis: X86BranchRelaxationWorkAxis,
) -> Result<(), OptimizedX86BranchRelaxationError> {
    let next = usage
        .checked_add(1)
        .ok_or(OptimizedX86BranchRelaxationError::BudgetExceeded(axis))?;
    if next > limit {
        return Err(OptimizedX86BranchRelaxationError::BudgetExceeded(axis));
    }
    *usage = next;
    Ok(())
}

fn checked_delta(target: u64, base: u64) -> Result<i64, OptimizedX86BranchRelaxationError> {
    i64::try_from(i128::from(target) - i128::from(base))
        .map_err(|_| OptimizedX86BranchRelaxationError::OffsetOverflow)
}
