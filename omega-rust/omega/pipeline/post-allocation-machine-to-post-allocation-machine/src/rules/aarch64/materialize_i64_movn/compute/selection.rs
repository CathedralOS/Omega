use optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};
use register_model::ValidatedPhysicalRegisterModel;
use selected_instructions::{SelectedInstructionKind, SelectedInstructionPlan};

use crate::{
    Aarch64MovnInstructionDisposition, Aarch64MovnMaterializationAction,
    Aarch64MovnMaterializationAttempt, Aarch64MovnMaterializationAttemptOutcome,
    Aarch64MovnMaterializationError, Aarch64MovnMaterializationFunction,
    Aarch64MovnMaterializationWorkAxis,
};
use physical_instructions::{PostAllocationMachineIdentity, PostAllocationMachinePlan};

use super::budget::charge;
use super::materialization::{integer_bits, qualified_write, validate_materialization};
use super::{movn_recipe, zero_seed_word_count};

pub(super) struct SelectedRewrites {
    pub(super) functions: Vec<Aarch64MovnMaterializationFunction>,
    pub(super) attempts: Vec<Aarch64MovnMaterializationAttempt>,
    pub(super) actions: Vec<Aarch64MovnMaterializationAction>,
    pub(super) usage: OptimizationWorkUsage,
}

pub(super) fn select(
    selected: &SelectedInstructionPlan,
    selected_identity: selected_instructions::SelectedInstructionPlanIdentity,
    source: &PostAllocationMachinePlan,
    source_identity: PostAllocationMachineIdentity,
    physical: &ValidatedPhysicalRegisterModel,
    budget: OptimizationWorkBudget,
    mut functions: Vec<Aarch64MovnMaterializationFunction>,
) -> Result<SelectedRewrites, Aarch64MovnMaterializationError> {
    let mut attempts = Vec::new();
    let mut actions = Vec::new();
    let mut usage = OptimizationWorkUsage::default();

    loop {
        charge(
            &mut usage.iterations,
            budget.iterations(),
            Aarch64MovnMaterializationWorkAxis::Iterations,
        )?;
        let iteration = usage.iterations;
        let input = super::super::identity::revision_identity(
            source_identity,
            selected_identity,
            source.target,
            physical.identity(),
            &functions,
        );
        let mut candidate = None;
        'scan: for (function_index, selected_function) in selected.functions.iter().enumerate() {
            let machine_function = source.functions.get(function_index).ok_or(
                Aarch64MovnMaterializationError::FunctionRosterMismatch(function_index),
            )?;
            if machine_function.machine != selected_function.machine {
                return Err(Aarch64MovnMaterializationError::FunctionRosterMismatch(
                    function_index,
                ));
            }
            for (block_index, block) in selected_function.blocks.iter().enumerate() {
                let machine_block = machine_function.blocks.get(block_index).ok_or(
                    Aarch64MovnMaterializationError::BlockRosterMismatch {
                        function: function_index,
                        block: block_index,
                    },
                )?;
                if machine_block.block != block.id
                    || machine_block.instructions.len() != block.instructions.len() + 1
                {
                    return Err(Aarch64MovnMaterializationError::BlockRosterMismatch {
                        function: function_index,
                        block: block_index,
                    });
                }
                for (instruction_index, instruction) in block.instructions.iter().enumerate() {
                    let SelectedInstructionKind::MaterializeI64 { value } = instruction.kind else {
                        continue;
                    };
                    charge(
                        &mut usage.rule_evaluations,
                        budget.rule_evaluations(),
                        Aarch64MovnMaterializationWorkAxis::RuleEvaluations,
                    )?;
                    let machine = machine_block.instructions.get(instruction_index).ok_or(
                        Aarch64MovnMaterializationError::InstructionRosterMismatch(instruction.id),
                    )?;
                    let literal_bits = integer_bits(value, instruction.id)?;
                    validate_materialization(instruction, machine, physical)?;
                    let destination = qualified_write(instruction, machine)?;
                    let baseline_word_count = zero_seed_word_count(literal_bits);
                    let recipe = movn_recipe(literal_bits);
                    let already_selected =
                        actions
                            .iter()
                            .any(|action: &Aarch64MovnMaterializationAction| {
                                action.machine == selected_function.machine
                                    && action.block == block.id
                                    && action.instruction == instruction.id
                            });
                    let candidate_words = recipe
                        .word_count()
                        .ok_or(Aarch64MovnMaterializationError::CountOverflow)?;
                    let outcome = if already_selected {
                        Aarch64MovnMaterializationAttemptOutcome::AlreadySelected
                    } else if candidate_words >= baseline_word_count {
                        Aarch64MovnMaterializationAttemptOutcome::BaselineNotLonger
                    } else {
                        Aarch64MovnMaterializationAttemptOutcome::SelectedForRewrite
                    };
                    attempts.push(Aarch64MovnMaterializationAttempt {
                        iteration,
                        input,
                        machine: selected_function.machine,
                        block: block.id,
                        instruction: instruction.id,
                        literal_bits,
                        destination: destination.clone(),
                        baseline_word_count,
                        recipe: recipe.clone(),
                        outcome,
                    });
                    if outcome == Aarch64MovnMaterializationAttemptOutcome::SelectedForRewrite {
                        charge(
                            &mut usage.candidates,
                            budget.candidates(),
                            Aarch64MovnMaterializationWorkAxis::Candidates,
                        )?;
                        charge(
                            &mut usage.validation_steps,
                            budget.validation_steps(),
                            Aarch64MovnMaterializationWorkAxis::ValidationSteps,
                        )?;
                        candidate = Some((
                            function_index,
                            block_index,
                            selected_function.machine,
                            block.id,
                            instruction.id,
                            literal_bits,
                            destination,
                            baseline_word_count,
                            recipe,
                        ));
                        break 'scan;
                    }
                }
            }
        }

        let Some((
            function_index,
            block_index,
            machine,
            block,
            instruction,
            literal_bits,
            destination,
            baseline_word_count,
            recipe,
        )) = candidate
        else {
            break;
        };
        charge(
            &mut usage.commits,
            budget.commits(),
            Aarch64MovnMaterializationWorkAxis::Commits,
        )?;
        let row = functions[function_index].blocks[block_index]
            .instructions
            .iter_mut()
            .find(|row| row.instruction == instruction)
            .ok_or(Aarch64MovnMaterializationError::InstructionRosterMismatch(
                instruction,
            ))?;
        row.disposition = Aarch64MovnInstructionDisposition::MovnSeededMaterializationV1 {
            literal_bits,
            destination: destination.clone(),
            baseline_word_count,
            recipe: recipe.clone(),
        };
        let output = super::super::identity::revision_identity(
            source_identity,
            selected_identity,
            source.target,
            physical.identity(),
            &functions,
        );
        actions.push(Aarch64MovnMaterializationAction {
            iteration,
            input,
            output,
            machine,
            block,
            instruction,
            literal_bits,
            destination,
            baseline_word_count,
            recipe,
        });
    }
    Ok(SelectedRewrites {
        functions,
        attempts,
        actions,
        usage,
    })
}
