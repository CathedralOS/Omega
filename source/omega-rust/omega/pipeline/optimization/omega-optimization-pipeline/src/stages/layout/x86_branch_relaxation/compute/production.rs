//! Production fixed-point scan and short-branch commit mechanics.

use omega_isa_x86_64::{
    encode_x86_64_selected_short_nonzero_branch_form,
    encode_x86_64_selected_u64_less_than_branch_form,
};
use omega_optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};
use omega_register_model::ValidatedPhysicalRegisterModel;
use omega_selected_instructions::{MachineAlternativeFamily, MachineAlternativeKey};

use crate::{ResolvedConditionalBranchPredicate, StagedOptimizedResolvedSelectedFormLayout};

use super::super::{
    error::{OptimizedX86BranchRelaxationError, X86BranchRelaxationWorkAxis},
    identity::{RevisionRoots, revision_identity},
    model::{
        X86BranchRelaxationAction, X86BranchRelaxationAttempt, X86BranchRelaxationAttemptOutcome,
    },
};
use super::{
    RelaxationTrace,
    branch_inspection::inspect_production_branch,
    reflow::reflow_production_functions,
    work::{charge, total_bytes},
};

pub(super) fn compute_trace(
    source: &StagedOptimizedResolvedSelectedFormLayout,
    physical: &ValidatedPhysicalRegisterModel,
    budget: OptimizationWorkBudget,
) -> Result<RelaxationTrace, OptimizedX86BranchRelaxationError> {
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
        let predicate = old
            .branch
            .as_deref()
            .ok_or(OptimizedX86BranchRelaxationError::MalformedBranch(
                old.instruction,
            ))?
            .predicate;
        let short_alternative = match predicate {
            ResolvedConditionalBranchPredicate::NonZeroV1 => old.alternative,
            ResolvedConditionalBranchPredicate::U64LessThanV1 => MachineAlternativeKey {
                family: MachineAlternativeFamily::ConditionalBranchU64LessThan,
                variant: 1,
            },
        };
        let encoded = match predicate {
            ResolvedConditionalBranchPredicate::NonZeroV1 => {
                encode_x86_64_selected_short_nonzero_branch_form(
                    physical,
                    short_alternative,
                    displacement,
                )
            }
            ResolvedConditionalBranchPredicate::U64LessThanV1 => {
                encode_x86_64_selected_u64_less_than_branch_form(
                    physical,
                    short_alternative,
                    displacement,
                )
            }
        }
        .map_err(OptimizedX86BranchRelaxationError::X86_64)?;
        functions[function_index].blocks[block_index].instructions[instruction_index].alternative =
            short_alternative;
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
    Ok(RelaxationTrace {
        usage,
        attempts,
        actions,
        functions,
    })
}
