use omega_optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};
use omega_regalloc::ValidatedSelectedAnalysis;
use omega_register_model::{RegisterOperandAccess, ValidatedPhysicalRegisterModel};
use omega_selected_instructions::{
    MachineAlternativeFamily, MachineEncodedControlEffect, MachineEncodedMemoryEffect,
    MachineEncodedStackEffect, MachineEncodedTrapBehavior, SelectedInstruction,
    SelectedInstructionKind, SelectedInstructionPlan,
};
use omega_target::Architecture;
use psi_core::IntegerValue;

use crate::{
    Aarch64MovnInstructionDisposition, Aarch64MovnMaterializationAction,
    Aarch64MovnMaterializationAttempt, Aarch64MovnMaterializationAttemptOutcome,
    Aarch64MovnMaterializationBlock, Aarch64MovnMaterializationError,
    Aarch64MovnMaterializationFunction, Aarch64MovnMaterializationIdentity,
    Aarch64MovnMaterializationInstruction, Aarch64MovnMaterializationPlan,
    Aarch64MovnMaterializationPolicy, Aarch64MovnMaterializationWorkAxis, Aarch64MovnPatch,
    Aarch64MovnRecipe, PhysicalOperandFootprint, PostAllocationMachineInstruction,
    PostAllocationMachinePlan, QualifiedPhysicalWrite, ValidatedAarch64MovnMaterialization,
    ValidatedPostAllocationMachinePlan, aarch64_movn_materialization_identity,
    materialization_receipt,
};

/// Independently reconstruct every recipe, attempt, action, revision, work
/// counter, and disposition. The replay deliberately does not call producer
/// helpers, including its MOVN recipe derivation.
pub fn validate_aarch64_movn_materialization<S: ValidatedSelectedAnalysis>(
    selected: &S,
    source: &ValidatedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    plan: Aarch64MovnMaterializationPlan,
) -> Result<ValidatedAarch64MovnMaterialization, Aarch64MovnMaterializationError> {
    if plan.policy
        != Aarch64MovnMaterializationPolicy::Aarch64SelectShortestMovnSeededI64MaterializationV1
    {
        return Err(Aarch64MovnMaterializationError::ArtifactMismatch);
    }
    let expected = replay_from_parts(
        selected.selected_plan(),
        selected.selected_identity(),
        source.plan(),
        source.receipt().identity(),
        physical,
        plan.budget,
    )?;
    if plan != expected {
        return Err(Aarch64MovnMaterializationError::ArtifactMismatch);
    }
    let receipt = materialization_receipt(&plan)?;
    Ok(ValidatedAarch64MovnMaterialization::new(plan, receipt))
}

pub(crate) fn replay_from_parts(
    selected: &SelectedInstructionPlan,
    selected_identity: omega_selected_instructions::SelectedInstructionPlanIdentity,
    source: &PostAllocationMachinePlan,
    source_identity: crate::PostAllocationMachineIdentity,
    physical: &ValidatedPhysicalRegisterModel,
    budget: OptimizationWorkBudget,
) -> Result<Aarch64MovnMaterializationPlan, Aarch64MovnMaterializationError> {
    independent_validate_roots(
        selected,
        selected_identity,
        source,
        source_identity,
        physical,
    )?;
    let mut functions = independent_baseline_roster(source);
    let mut attempts = Vec::new();
    let mut actions = Vec::new();
    let mut usage = OptimizationWorkUsage::default();

    loop {
        independent_charge(
            &mut usage.iterations,
            budget.iterations(),
            Aarch64MovnMaterializationWorkAxis::Iterations,
        )?;
        let iteration = usage.iterations;
        let input = super::identity::revision_identity(
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
                    independent_charge(
                        &mut usage.rule_evaluations,
                        budget.rule_evaluations(),
                        Aarch64MovnMaterializationWorkAxis::RuleEvaluations,
                    )?;
                    let machine = machine_block.instructions.get(instruction_index).ok_or(
                        Aarch64MovnMaterializationError::InstructionRosterMismatch(instruction.id),
                    )?;
                    let literal_bits = independent_integer_bits(value, instruction.id)?;
                    independent_validate_materialization(instruction, machine, physical)?;
                    let destination = independent_qualified_write(instruction, machine)?;
                    let baseline_word_count = independent_zero_seed_word_count(literal_bits);
                    let recipe = independent_movn_recipe(literal_bits);
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
                        independent_charge(
                            &mut usage.candidates,
                            budget.candidates(),
                            Aarch64MovnMaterializationWorkAxis::Candidates,
                        )?;
                        independent_charge(
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
        independent_charge(
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
        let output = super::identity::revision_identity(
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

    let output_revision = super::identity::revision_identity(
        source_identity,
        selected_identity,
        source.target,
        physical.identity(),
        &functions,
    );
    let mut expected = Aarch64MovnMaterializationPlan {
        identity: Aarch64MovnMaterializationIdentity::from_bytes([0; 32]),
        source: source_identity,
        selected: selected_identity,
        target: source.target,
        physical_register_model: physical.identity(),
        policy:
            Aarch64MovnMaterializationPolicy::Aarch64SelectShortestMovnSeededI64MaterializationV1,
        budget,
        usage,
        output_revision,
        attempts,
        actions,
        functions,
    };
    expected.identity = aarch64_movn_materialization_identity(&expected);
    Ok(expected)
}

fn independent_validate_roots(
    selected: &SelectedInstructionPlan,
    selected_identity: omega_selected_instructions::SelectedInstructionPlanIdentity,
    source: &PostAllocationMachinePlan,
    source_identity: crate::PostAllocationMachineIdentity,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<(), Aarch64MovnMaterializationError> {
    if selected.target.architecture != Architecture::Aarch64
        || source.target.architecture != Architecture::Aarch64
        || physical.model().architecture != Architecture::Aarch64
    {
        return Err(Aarch64MovnMaterializationError::UnsupportedTarget(
            source.target,
        ));
    }
    if source.identity != source_identity
        || source.selected != selected_identity
        || selected.target != source.target
        || source.physical_register_model != physical.identity()
        || selected.functions.len() != source.functions.len()
    {
        return Err(Aarch64MovnMaterializationError::RootMismatch);
    }
    Ok(())
}

fn independent_baseline_roster(
    source: &PostAllocationMachinePlan,
) -> Vec<Aarch64MovnMaterializationFunction> {
    source
        .functions
        .iter()
        .map(|function| Aarch64MovnMaterializationFunction {
            machine: function.machine,
            blocks: function
                .blocks
                .iter()
                .map(|block| Aarch64MovnMaterializationBlock {
                    block: block.block,
                    instructions: block
                        .instructions
                        .iter()
                        .map(|instruction| Aarch64MovnMaterializationInstruction {
                            instruction: instruction.instruction,
                            disposition: Aarch64MovnInstructionDisposition::RetainedV1,
                        })
                        .collect(),
                })
                .collect(),
        })
        .collect()
}

fn independent_integer_bits(
    value: IntegerValue,
    instruction: omega_selected_instructions::SelectedInstructionId,
) -> Result<u64, Aarch64MovnMaterializationError> {
    match value {
        IntegerValue::Signed(value) => i64::try_from(value)
            .map(|value| value as u64)
            .map_err(|_| Aarch64MovnMaterializationError::IntegerOutsideI64Bits(instruction)),
        IntegerValue::Unsigned(value) => u64::try_from(value)
            .map_err(|_| Aarch64MovnMaterializationError::IntegerOutsideI64Bits(instruction)),
    }
}

fn independent_validate_materialization(
    selected: &SelectedInstruction,
    machine: &PostAllocationMachineInstruction,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<(), Aarch64MovnMaterializationError> {
    let encoded = &machine.alternative.encoded;
    if machine.instruction != selected.id
        || selected.operands.len() != 1
        || !selected.implicit_uses.is_empty()
        || !selected.implicit_defs.is_empty()
        || !selected.clobbers.is_empty()
        || machine.alternative.key.family != MachineAlternativeFamily::MaterializeI64
        || machine.alternative.key.variant != 0
        || !encoded.external_operand_reads.is_empty()
        || encoded.external_operand_writes != [0]
        || !encoded.implicit_unit_uses.is_empty()
        || !encoded.implicit_unit_defs.is_empty()
        || !encoded.implicit_unit_clobbers.is_empty()
        || encoded.memory != MachineEncodedMemoryEffect::NoneV1
        || encoded.stack != MachineEncodedStackEffect::UnchangedV1
        || encoded.trap != MachineEncodedTrapBehavior::NeverV1
        || encoded.control != MachineEncodedControlEffect::FallThroughV1
        || machine.operands.len() != 1
        || !machine.implicit_unit_uses.is_empty()
        || !machine.implicit_unit_defs.is_empty()
        || !machine.implicit_unit_clobbers.is_empty()
        || !machine.unit_uses.is_empty()
        || !machine.unit_clobbers.is_empty()
    {
        return Err(Aarch64MovnMaterializationError::InvalidMaterializationFootprint(selected.id));
    }
    let selected_operand = &selected.operands[0];
    let operand = &machine.operands[0];
    if selected_operand.operand != 0
        || selected_operand.access != RegisterOperandAccess::Def
        || operand.operand != 0
        || operand.virtual_register != selected_operand.virtual_register
        || operand.class != selected_operand.class
        || operand.access != RegisterOperandAccess::Def
        || !operand.read_units.is_empty()
        || machine.unit_defs != operand.write_units
        || operand.write_semantics.is_none()
    {
        return Err(Aarch64MovnMaterializationError::InvalidMaterializationFootprint(selected.id));
    }
    independent_validate_x_view(operand, physical, selected.id)
}

fn independent_validate_x_view(
    operand: &PhysicalOperandFootprint,
    physical: &ValidatedPhysicalRegisterModel,
    instruction: omega_selected_instructions::SelectedInstructionId,
) -> Result<(), Aarch64MovnMaterializationError> {
    let view = physical
        .model()
        .views
        .iter()
        .find(|view| view.id == operand.view)
        .ok_or(Aarch64MovnMaterializationError::InvalidPhysicalDestination(
            instruction,
        ))?;
    let valid_index = view
        .name
        .strip_prefix('x')
        .and_then(|name| name.parse::<u8>().ok())
        .is_some_and(|index| index <= 30);
    if !valid_index
        || view.bits != 64
        || !view.allocatable
        || view.class != operand.class
        || view.units != operand.storage_units
        || view.write_units != operand.write_units
        || Some(view.write_semantics) != operand.write_semantics
    {
        return Err(Aarch64MovnMaterializationError::InvalidPhysicalDestination(
            instruction,
        ));
    }
    Ok(())
}

fn independent_qualified_write(
    selected: &SelectedInstruction,
    machine: &PostAllocationMachineInstruction,
) -> Result<QualifiedPhysicalWrite, Aarch64MovnMaterializationError> {
    let operand = machine
        .operands
        .first()
        .ok_or(Aarch64MovnMaterializationError::InvalidMaterializationFootprint(selected.id))?;
    Ok(QualifiedPhysicalWrite {
        instruction: selected.id,
        operand: operand.operand,
        virtual_register: operand.virtual_register,
        class: operand.class,
        view: operand.view,
        storage_units: operand.storage_units.clone(),
        write_units: operand.write_units.clone(),
        write_semantics: operand
            .write_semantics
            .ok_or(Aarch64MovnMaterializationError::InvalidMaterializationFootprint(selected.id))?,
    })
}

fn independent_zero_seed_word_count(bits: u64) -> u8 {
    let upper_nonzero = [16_u32, 32, 48]
        .into_iter()
        .filter(|shift| ((bits >> shift) & 0xffff) != 0)
        .count();
    1 + upper_nonzero as u8
}

fn independent_movn_recipe(bits: u64) -> Aarch64MovnRecipe {
    let chunks = [
        (bits & 0xffff) as u16,
        ((bits >> 16) & 0xffff) as u16,
        ((bits >> 32) & 0xffff) as u16,
        ((bits >> 48) & 0xffff) as u16,
    ];
    let mut seed_halfword = 0_u8;
    for halfword in 0_u8..4 {
        if chunks[usize::from(halfword)] != u16::MAX {
            seed_halfword = halfword;
            break;
        }
    }
    let mut patches = Vec::new();
    for halfword in 0_u8..4 {
        let immediate = chunks[usize::from(halfword)];
        if halfword != seed_halfword && immediate != u16::MAX {
            patches.push(Aarch64MovnPatch {
                halfword,
                immediate,
            });
        }
    }
    Aarch64MovnRecipe {
        seed_halfword,
        seed_immediate: chunks[usize::from(seed_halfword)] ^ u16::MAX,
        patches,
    }
}

fn independent_charge(
    usage: &mut u64,
    budget: u64,
    axis: Aarch64MovnMaterializationWorkAxis,
) -> Result<(), Aarch64MovnMaterializationError> {
    let next = usage
        .checked_add(1)
        .ok_or(Aarch64MovnMaterializationError::BudgetExceeded(axis))?;
    if next > budget {
        return Err(Aarch64MovnMaterializationError::BudgetExceeded(axis));
    }
    *usage = next;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::independent_movn_recipe;

    #[test]
    fn validator_derives_all_ones_as_single_movn() {
        let recipe = independent_movn_recipe(u64::MAX);
        assert_eq!(recipe.seed_halfword, 0);
        assert_eq!(recipe.seed_immediate, 0);
        assert!(recipe.patches.is_empty());
    }
}
