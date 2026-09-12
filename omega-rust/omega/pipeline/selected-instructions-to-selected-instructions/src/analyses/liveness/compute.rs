use std::collections::{BTreeMap, BTreeSet};

use crate::analyses::liveness::model::LivenessError;
use register_model::RegisterOperandAccess;
use selected_instructions::{
    BlockLiveness, EntryDefinition, FunctionLiveness, InstructionLiveness, LivenessPlan,
    LivenessPosition, OperandPosition, SuccessorLiveness,
};
use selected_instructions::{
    SelectedBlock, SelectedFunction, SelectedInstruction, VirtualRegisterId,
};

mod control;
mod flow;

#[cfg(test)]
pub(crate) use flow::BLOCK_VISITS;

#[cfg(test)]
std::thread_local! {
    pub(crate) static FUNCTION_COMPUTATIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

pub(crate) fn compute_terminal_liveness(
    selected: &impl crate::ValidatedSelectedAnalysis,
) -> Result<LivenessPlan, LivenessError> {
    let plan = selected.selected_plan();
    let functions = plan
        .functions
        .iter()
        .enumerate()
        .map(|(index, function)| compute_function(index, function))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(LivenessPlan {
        selected: selected.selected_identity(),
        optimization_unit: selected.optimization_unit_identity(),
        fuel_schedule: selected.fuel_schedule_identity(),
        target: plan.target,
        functions,
    })
}

pub(crate) fn compute_terminal_liveness_reusing(
    previous: &impl crate::ValidatedSelectedAnalysis,
    previous_liveness: &crate::ValidatedLiveness,
    selected: &impl crate::ValidatedSelectedAnalysis,
) -> Result<LivenessPlan, LivenessError> {
    let plan = selected.selected_plan();
    let prior = previous_liveness.plan();
    let compatible = prior.selected == previous.selected_identity()
        && prior.optimization_unit == previous.optimization_unit_identity()
        && prior.fuel_schedule == previous.fuel_schedule_identity()
        && prior.target == previous.selected_plan().target
        && prior.optimization_unit == selected.optimization_unit_identity()
        && prior.fuel_schedule == selected.fuel_schedule_identity()
        && prior.target == plan.target
        && prior.functions.len() == previous.selected_plan().functions.len();
    let functions = plan
        .functions
        .iter()
        .enumerate()
        .map(|(function_index, function)| {
            // Shared immutable bodies are equal by construction; detached equal
            // bodies remain eligible. Allocation identity never grants evidence.
            if compatible
                && (previous
                    .selected_plan()
                    .functions
                    .shares_function_storage(&plan.functions, function_index)
                    || previous.selected_plan().functions.get(function_index) == Some(function))
                && let Some(facts) = prior.functions.get(function_index)
            {
                return Ok(facts.clone());
            }
            compute_function(function_index, function)
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(LivenessPlan {
        selected: selected.selected_identity(),
        optimization_unit: selected.optimization_unit_identity(),
        fuel_schedule: selected.fuel_schedule_identity(),
        target: plan.target,
        functions,
    })
}

pub(crate) fn compute_function(
    function_index: usize,
    function: &SelectedFunction,
) -> Result<FunctionLiveness, LivenessError> {
    #[cfg(test)]
    FUNCTION_COMPUTATIONS.set(FUNCTION_COMPUTATIONS.get() + 1);
    reject_unsupported_constraints(function_index, function)?;
    super::edge_values::validate_transports(function_index, function)?;
    let flow = flow::FunctionFlow::compute(function_index, function)?;

    let mut next_position = 0_u32;
    let mut positions = BTreeMap::new();
    for block in &function.blocks {
        for instruction in block_instructions(block) {
            positions.insert(instruction.id, LivenessPosition(next_position));
            next_position =
                next_position
                    .checked_add(1)
                    .ok_or(LivenessError::NonDensePositions {
                        function: function_index,
                    })?;
        }
    }

    let entry_definitions = function
        .virtual_registers
        .iter()
        .filter(|register| function.is_entry_register(register))
        .map(|register| EntryDefinition {
            virtual_register: register.id,
            class: register.class,
            fixed_view: register.entry_fixed_view,
        })
        .collect();
    let operand_positions = function
        .blocks
        .iter()
        .flat_map(block_instructions)
        .flat_map(|instruction| {
            let instruction_position = positions[&instruction.id];
            instruction
                .operands
                .iter()
                .map(move |operand| OperandPosition {
                    position: instruction_position,
                    instruction: instruction.id,
                    operand: operand.operand,
                    virtual_register: operand.virtual_register,
                    access: operand.access,
                    class: operand.class,
                    fixed_view: operand.fixed_view,
                    tied_to: operand.tied_to,
                    early_clobber: operand.early_clobber,
                })
        })
        .collect();
    let blocks = function
        .blocks
        .iter()
        .enumerate()
        .map(|(block_index, block)| {
            materialize_block(function_index, block, &positions, &flow, block_index)
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(FunctionLiveness {
        machine: function.machine,
        entry_definitions,
        operand_positions,
        blocks,
    })
}

pub(super) fn reject_unsupported_constraints(
    function_index: usize,
    function: &SelectedFunction,
) -> Result<(), LivenessError> {
    let mut tied_edges = Vec::new();
    let mut early_rows = Vec::new();
    for instruction in function.blocks.iter().flat_map(block_instructions) {
        for operand in &instruction.operands {
            if operand.access == RegisterOperandAccess::UseDef {
                return Err(LivenessError::UnsupportedUseDef {
                    function: function_index,
                    instruction: instruction.id.0,
                    operand: operand.operand,
                });
            }
        }
        let early = instruction
            .operands
            .iter()
            .filter(|operand| operand.early_clobber)
            .collect::<Vec<_>>();
        if early.len() > 1 {
            // Independent early outputs share the instruction's after point,
            // so ordinary Def/Def interference separates even dead scratch.
            // Each output also retains every untied Use as an early hazard.
            let mut participants = BTreeSet::new();
            let mut positions = BTreeSet::new();
            let valid = instruction.operands.iter().all(|operand| {
                operand.tied_to.is_none()
                    && matches!(
                        (operand.access, operand.early_clobber),
                        (RegisterOperandAccess::Use, false) | (RegisterOperandAccess::Def, true)
                    )
                    && participants.insert(operand.virtual_register)
                    && positions.insert(operand.operand)
            }) && instruction
                .operands
                .iter()
                .any(|operand| operand.access == RegisterOperandAccess::Use);
            if !valid {
                return Err(LivenessError::UnsupportedEarlyClobber {
                    function: function_index,
                    instruction: instruction.id.0,
                    operand: early[1].operand,
                });
            }
            for definition in early {
                early_rows.push((
                    instruction.id.0,
                    definition.operand,
                    definition.virtual_register,
                    None,
                    instruction
                        .operands
                        .iter()
                        .filter(|operand| operand.access == RegisterOperandAccess::Use)
                        .map(|operand| (operand.virtual_register, operand.operand))
                        .collect::<Vec<_>>(),
                ));
            }
        } else if !early.is_empty() {
            let definition = early[0];
            let mut participants = BTreeSet::new();
            let tied_source = definition.tied_to.and_then(|operand| {
                instruction
                    .operands
                    .iter()
                    .find(|candidate| candidate.operand == operand)
            });
            let valid_source = tied_source.is_none_or(|source| {
                source.access == RegisterOperandAccess::Use
                    && source.operand < definition.operand
                    && source.virtual_register != definition.virtual_register
                    && source.class == definition.class
                    && source.tied_to.is_none()
            });
            let valid = early.len() == 1
                && definition.access == RegisterOperandAccess::Def
                && instruction.operands.len() > 1
                && valid_source
                && instruction.operands.iter().all(|operand| {
                    (operand.operand == definition.operand
                        || (tied_source.is_some_and(|source| source.operand == operand.operand)
                            && operand.tied_to.is_none())
                        || (operand.tied_to.is_none()
                            && operand.access == RegisterOperandAccess::Use))
                        && participants.insert(operand.virtual_register)
                });
            let unrelated_use_count = instruction
                .operands
                .iter()
                .filter(|operand| {
                    operand.operand != definition.operand
                        && tied_source.is_none_or(|source| source.operand != operand.operand)
                })
                .count();
            if !valid || tied_source.is_some() && unrelated_use_count == 0 {
                let operand = early.get(1).copied().unwrap_or(definition).operand;
                return Err(LivenessError::UnsupportedEarlyClobber {
                    function: function_index,
                    instruction: instruction.id.0,
                    operand,
                });
            }
            early_rows.push((
                instruction.id.0,
                definition.operand,
                definition.virtual_register,
                tied_source.map(|source| source.virtual_register),
                instruction
                    .operands
                    .iter()
                    .filter(|operand| {
                        operand.operand != definition.operand
                            && tied_source.is_none_or(|source| source.operand != operand.operand)
                    })
                    .map(|operand| (operand.virtual_register, operand.operand))
                    .collect::<Vec<_>>(),
            ));
        }
        let tied = instruction
            .operands
            .iter()
            .filter(|operand| operand.tied_to.is_some())
            .collect::<Vec<_>>();
        for definition in tied {
            let Some(use_operand) = instruction
                .operands
                .iter()
                .find(|operand| Some(operand.operand) == definition.tied_to)
            else {
                return Err(LivenessError::UnsupportedTiedOperand {
                    function: function_index,
                    instruction: instruction.id.0,
                    operand: definition.operand,
                });
            };
            if definition.access != RegisterOperandAccess::Def
                || use_operand.access != RegisterOperandAccess::Use
                || definition.operand <= use_operand.operand
                || definition.virtual_register == use_operand.virtual_register
                || definition.class != use_operand.class
                || use_operand.tied_to.is_some()
            {
                return Err(LivenessError::UnsupportedTiedOperand {
                    function: function_index,
                    instruction: instruction.id.0,
                    operand: definition.operand,
                });
            }
            tied_edges.push((use_operand.virtual_register, definition.virtual_register));
        }
    }
    // SingleEarlyDefTiedComponentAgainstUntiedUsesV1 admits an ordinary tied
    // component only when it owns exactly one early-clobber definition.
    for (instruction, def_operand, definition, tied_source, unrelated) in &early_rows {
        let valid_tie = tied_source.is_none_or(|source| {
            let component = tied_component(source, &tied_edges);
            tied_edges.contains(&(source, *definition))
                && component.contains(definition)
                && early_rows
                    .iter()
                    .filter(|(_, _, candidate, candidate_source, _)| {
                        candidate_source.is_some() && component.contains(candidate)
                    })
                    .count()
                    == 1
        });
        let untied_definition_is_free = tied_source.is_some()
            || tied_edges
                .iter()
                .all(|(left, right)| left != definition && right != definition);
        if !valid_tie
            || !untied_definition_is_free
            || unrelated.iter().any(|(register, _)| {
                tied_edges
                    .iter()
                    .any(|(left, right)| left == register || right == register)
            })
        {
            return Err(LivenessError::UnsupportedEarlyClobber {
                function: function_index,
                instruction: *instruction,
                operand: unrelated
                    .iter()
                    .find(|(register, _)| {
                        tied_edges
                            .iter()
                            .any(|(left, right)| left == register || right == register)
                    })
                    .map_or(*def_operand, |(_, operand)| *operand),
            });
        }
    }
    Ok(())
}

fn tied_component(
    seed: VirtualRegisterId,
    edges: &[(VirtualRegisterId, VirtualRegisterId)],
) -> BTreeSet<VirtualRegisterId> {
    let mut component = BTreeSet::from([seed]);
    loop {
        let previous_len = component.len();
        for (left, right) in edges {
            if component.contains(left) || component.contains(right) {
                component.insert(*left);
                component.insert(*right);
            }
        }
        if component.len() == previous_len {
            return component;
        }
    }
}

fn materialize_block(
    function_index: usize,
    block: &SelectedBlock,
    positions: &BTreeMap<selected_instructions::SelectedInstructionId, LivenessPosition>,
    flow: &flow::FunctionFlow,
    block_index: usize,
) -> Result<BlockLiveness, LivenessError> {
    let state = &flow.blocks[block_index];
    let mut virtual_live = state.virtual_exit.clone();
    let mut unit_live = state.unit_exit.clone();
    let mut instructions = Vec::with_capacity(block.instructions.len() + 1);
    for instruction in block_instructions(block).rev() {
        let virtual_live_out = sorted(&virtual_live);
        let unit_live_out = sorted(&unit_live);
        let (uses, defs) = virtual_uses_defs(instruction);
        for definition in &defs {
            virtual_live.remove(definition);
        }
        virtual_live.extend(uses.iter().copied());
        for killed in instruction
            .implicit_defs
            .iter()
            .chain(&instruction.clobbers)
        {
            unit_live.remove(killed);
        }
        unit_live.extend(instruction.implicit_uses.iter().copied());
        instructions.push(InstructionLiveness {
            position: positions[&instruction.id],
            instruction: instruction.id,
            virtual_uses: sorted(&uses),
            virtual_defs: sorted(&defs),
            virtual_live_in: sorted(&virtual_live),
            virtual_live_out,
            unit_uses: instruction.implicit_uses.clone(),
            unit_defs: instruction.implicit_defs.clone(),
            unit_clobbers: instruction.clobbers.clone(),
            unit_live_in: sorted(&unit_live),
            unit_live_out,
        });
    }
    instructions.reverse();
    let terminator = control::instruction(&block.terminator);
    let successors = control::successors(&block.terminator)
        .enumerate()
        .map(|(ordinal, successor)| {
            Ok(SuccessorLiveness {
                terminator: terminator.id,
                polarity_ordinal: ordinal as u8,
                psi_edge: successor.psi_edge,
                target: successor.block,
                virtual_live: sorted(
                    &flow.blocks[flow.edges[block_index][ordinal].target]
                        .virtual_entry
                        .iter()
                        .map(|register| {
                            flow.incoming_argument(
                                function_index,
                                &flow.edges[block_index][ordinal],
                                *register,
                            )
                        })
                        .collect::<Result<BTreeSet<_>, _>>()?,
                ),
                unit_live: sorted(&flow.blocks[flow.edges[block_index][ordinal].target].unit_entry),
            })
        })
        .collect::<Result<Vec<_>, LivenessError>>()?;
    Ok(BlockLiveness {
        block: block.id,
        source_block: block.source_block(),
        virtual_live_in: sorted(&state.virtual_entry),
        virtual_live_out: sorted(&state.virtual_exit),
        unit_live_in: sorted(&state.unit_entry),
        unit_live_out: sorted(&state.unit_exit),
        instructions,
        successors,
    })
}

fn virtual_uses_defs(
    instruction: &SelectedInstruction,
) -> (BTreeSet<VirtualRegisterId>, BTreeSet<VirtualRegisterId>) {
    let mut uses = BTreeSet::new();
    let mut defs = BTreeSet::new();
    for operand in &instruction.operands {
        match operand.access {
            RegisterOperandAccess::Use => {
                uses.insert(operand.virtual_register);
            }
            RegisterOperandAccess::Def => {
                defs.insert(operand.virtual_register);
            }
            RegisterOperandAccess::UseDef => {
                uses.insert(operand.virtual_register);
                defs.insert(operand.virtual_register);
            }
        }
    }
    (uses, defs)
}

fn block_instructions(
    block: &SelectedBlock,
) -> impl DoubleEndedIterator<Item = &SelectedInstruction> {
    block
        .instructions
        .iter()
        .chain(std::iter::once(control::instruction(&block.terminator)))
}

fn sorted<T: Copy + Ord>(values: &BTreeSet<T>) -> Vec<T> {
    values.iter().copied().collect()
}
