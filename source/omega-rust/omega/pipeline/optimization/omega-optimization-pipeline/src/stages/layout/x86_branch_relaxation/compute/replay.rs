//! Independent ordered replay, byte validation, and action reconstruction.

use omega_isa_x86_64::{
    validate_x86_64_selected_i64_less_than_branch_form,
    validate_x86_64_selected_short_nonzero_branch_form,
    validate_x86_64_selected_u64_less_than_branch_form,
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
    branch_inspection::replay_inspect_branch,
    reflow::reflow_replay_functions,
    work::{ordered_branch_locations, replay_charge, total_bytes},
};

pub(super) fn replay_trace(
    source: &StagedOptimizedResolvedSelectedFormLayout,
    physical: &ValidatedPhysicalRegisterModel,
    budget: OptimizationWorkBudget,
) -> Result<RelaxationTrace, OptimizedX86BranchRelaxationError> {
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
        let predicate = old
            .branch
            .as_deref()
            .ok_or(OptimizedX86BranchRelaxationError::MalformedBranch(
                old.instruction,
            ))?
            .predicate;
        let (short_alternative, bytes) = match predicate {
            ResolvedConditionalBranchPredicate::NonZeroV1 => {
                let bytes = [0x75, displacement as i8 as u8];
                validate_x86_64_selected_short_nonzero_branch_form(
                    physical,
                    old.alternative,
                    displacement,
                    &bytes,
                )
                .map_err(OptimizedX86BranchRelaxationError::X86_64)?;
                (old.alternative, bytes)
            }
            ResolvedConditionalBranchPredicate::U64LessThanV1 => {
                let alternative = MachineAlternativeKey {
                    family: MachineAlternativeFamily::ConditionalBranchU64LessThan,
                    variant: 1,
                };
                let bytes = [0x72, displacement as i8 as u8];
                validate_x86_64_selected_u64_less_than_branch_form(
                    physical,
                    alternative,
                    displacement,
                    &bytes,
                )
                .map_err(OptimizedX86BranchRelaxationError::X86_64)?;
                (alternative, bytes)
            }
            ResolvedConditionalBranchPredicate::I64LessThanV1 => {
                let alternative = MachineAlternativeKey {
                    family: MachineAlternativeFamily::ConditionalBranchI64LessThan,
                    variant: 1,
                };
                let bytes = [0x7c, displacement as i8 as u8];
                validate_x86_64_selected_i64_less_than_branch_form(
                    physical,
                    alternative,
                    displacement,
                    &bytes,
                )
                .map_err(OptimizedX86BranchRelaxationError::X86_64)?;
                (alternative, bytes)
            }
        };
        functions[function_index].blocks[block_index].instructions[instruction_index].alternative =
            short_alternative;
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
    Ok(RelaxationTrace {
        usage,
        attempts,
        actions,
        functions,
    })
}
