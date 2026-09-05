use std::collections::{BTreeMap, BTreeSet};

use crate::analyses::liveness::model::{
    BlockLiveness, EntryDefinition, FunctionLiveness, InstructionLiveness, LivenessError,
    LivenessPlan, LivenessPosition, OperandPosition, SuccessorLiveness,
};
use register_model::{RegisterOperandAccess, RegisterUnitId};
use selected_instructions::{
    SelectedBlock, SelectedFunction, SelectedInstruction, SelectedStructuralUnitFunction,
    VirtualRegisterId, VirtualRegisterOrigin,
};

mod control;

pub(crate) fn compute_terminal_liveness(
    selected: &impl crate::ValidatedSelectedAnalysis,
) -> Result<LivenessPlan, LivenessError> {
    let plan = selected.selected_plan();
    if !plan.projected_structural_call_returns.is_empty() {
        return Err(LivenessError::ProjectedStructuralCallReturnUnsupported);
    }
    let functions = plan
        .functions
        .iter()
        .enumerate()
        .map(|(index, function)| compute_function(index, function))
        .collect::<Result<Vec<_>, _>>()?;
    let structural_unit_functions = plan
        .structural_unit_functions
        .iter()
        .enumerate()
        .map(|(index, function)| compute_structural_unit_function(index, function))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(LivenessPlan {
        selected: selected.selected_identity(),
        optimization_unit: selected.optimization_unit_identity(),
        fuel_schedule: selected.fuel_schedule_identity(),
        target: plan.target,
        functions,
        structural_unit_functions,
    })
}

pub(crate) fn compute_structural_unit_function(
    function_index: usize,
    function: &SelectedStructuralUnitFunction,
) -> Result<FunctionLiveness, LivenessError> {
    let mut instructions = Vec::with_capacity(usize::from(function.call.is_some()) + 1);
    if let Some(call) = &function.call {
        instructions.push(StructuralUnitInstructionFacts {
            id: call.id,
            uses: &call.implicit_uses,
            defs: &call.implicit_defs,
            clobbers: &call.clobbers,
        });
    }
    let terminator = &function.terminator.instruction;
    instructions.push(StructuralUnitInstructionFacts {
        id: terminator.id,
        uses: &terminator.implicit_uses,
        defs: &terminator.implicit_defs,
        clobbers: &terminator.clobbers,
    });
    compute_structural_unit_facts(
        function_index,
        function.machine,
        function.entry_block,
        function.source_entry_block,
        &instructions,
    )
}

#[derive(Clone, Copy)]
pub(super) struct StructuralUnitInstructionFacts<'a> {
    pub(super) id: selected_instructions::SelectedInstructionId,
    pub(super) uses: &'a [RegisterUnitId],
    pub(super) defs: &'a [RegisterUnitId],
    pub(super) clobbers: &'a [RegisterUnitId],
}

pub(super) fn compute_structural_unit_facts(
    function_index: usize,
    machine: semantic_vocabulary::MachineId,
    entry_block: selected_instructions::SelectedBlockId,
    source_entry_block: semantic_vocabulary::BlockId,
    instructions: &[StructuralUnitInstructionFacts<'_>],
) -> Result<FunctionLiveness, LivenessError> {
    let mut unit_live = BTreeSet::new();
    let mut rows = Vec::with_capacity(instructions.len());
    for (ordinal, instruction) in instructions.iter().enumerate().rev() {
        let position = LivenessPosition(u32::try_from(ordinal).map_err(|_| {
            LivenessError::NonDensePositions {
                function: function_index,
            }
        })?);
        let unit_live_out = sorted(&unit_live);
        for unit in instruction.defs.iter().chain(instruction.clobbers) {
            unit_live.remove(unit);
        }
        unit_live.extend(instruction.uses.iter().copied());
        rows.push(InstructionLiveness {
            position,
            instruction: instruction.id,
            virtual_uses: Vec::new(),
            virtual_defs: Vec::new(),
            virtual_live_in: Vec::new(),
            virtual_live_out: Vec::new(),
            unit_uses: instruction.uses.to_vec(),
            unit_defs: instruction.defs.to_vec(),
            unit_clobbers: instruction.clobbers.to_vec(),
            unit_live_in: sorted(&unit_live),
            unit_live_out,
        });
    }
    rows.reverse();
    Ok(FunctionLiveness {
        machine,
        entry_definitions: Vec::new(),
        operand_positions: Vec::new(),
        blocks: vec![BlockLiveness {
            block: entry_block,
            source_block: source_entry_block,
            virtual_live_in: Vec::new(),
            virtual_live_out: Vec::new(),
            unit_live_in: sorted(&unit_live),
            unit_live_out: Vec::new(),
            instructions: rows,
            successors: Vec::new(),
        }],
    })
}

pub(crate) fn compute_function(
    function_index: usize,
    function: &SelectedFunction,
) -> Result<FunctionLiveness, LivenessError> {
    reject_unsupported_constraints(function_index, function)?;
    let mut virtual_entry = function
        .blocks
        .iter()
        .map(|block| (block.id, BTreeSet::new()))
        .collect::<BTreeMap<_, _>>();
    let mut virtual_exit = virtual_entry.clone();
    let mut unit_entry = function
        .blocks
        .iter()
        .map(|block| (block.id, BTreeSet::new()))
        .collect::<BTreeMap<_, _>>();
    let mut unit_exit = unit_entry.clone();

    loop {
        let mut changed = false;
        for block in function.blocks.iter().rev() {
            let successors = successor_blocks(block);
            let next_virtual_exit = successors
                .iter()
                .filter_map(|successor| virtual_entry.get(successor))
                .flat_map(|set| set.iter().copied())
                .collect::<BTreeSet<_>>();
            let next_unit_exit = successors
                .iter()
                .filter_map(|successor| unit_entry.get(successor))
                .flat_map(|set| set.iter().copied())
                .collect::<BTreeSet<_>>();
            let (next_virtual_entry, next_unit_entry) =
                reverse_block_transfer(block, next_virtual_exit.clone(), next_unit_exit.clone());
            if virtual_exit[&block.id] != next_virtual_exit {
                virtual_exit.insert(block.id, next_virtual_exit);
                changed = true;
            }
            if unit_exit[&block.id] != next_unit_exit {
                unit_exit.insert(block.id, next_unit_exit);
                changed = true;
            }
            if virtual_entry[&block.id] != next_virtual_entry {
                virtual_entry.insert(block.id, next_virtual_entry);
                changed = true;
            }
            if unit_entry[&block.id] != next_unit_entry {
                unit_entry.insert(block.id, next_unit_entry);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }

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
        .filter(|register| {
            matches!(
                register.origin,
                VirtualRegisterOrigin::EntryParameter { .. }
            )
        })
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
        .map(|block| {
            materialize_block(
                block,
                &positions,
                &virtual_entry,
                &virtual_exit,
                &unit_entry,
                &unit_exit,
            )
        })
        .collect();
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
        if !early.is_empty() {
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

fn reverse_block_transfer(
    block: &SelectedBlock,
    mut virtual_live: BTreeSet<VirtualRegisterId>,
    mut unit_live: BTreeSet<RegisterUnitId>,
) -> (BTreeSet<VirtualRegisterId>, BTreeSet<RegisterUnitId>) {
    for instruction in block_instructions(block).into_iter().rev() {
        let (uses, defs) = virtual_uses_defs(instruction);
        for definition in defs {
            virtual_live.remove(&definition);
        }
        virtual_live.extend(uses);
        for killed in instruction
            .implicit_defs
            .iter()
            .chain(&instruction.clobbers)
        {
            unit_live.remove(killed);
        }
        unit_live.extend(instruction.implicit_uses.iter().copied());
    }
    (virtual_live, unit_live)
}

fn materialize_block(
    block: &SelectedBlock,
    positions: &BTreeMap<selected_instructions::SelectedInstructionId, LivenessPosition>,
    virtual_entry: &BTreeMap<selected_instructions::SelectedBlockId, BTreeSet<VirtualRegisterId>>,
    virtual_exit: &BTreeMap<selected_instructions::SelectedBlockId, BTreeSet<VirtualRegisterId>>,
    unit_entry: &BTreeMap<selected_instructions::SelectedBlockId, BTreeSet<RegisterUnitId>>,
    unit_exit: &BTreeMap<selected_instructions::SelectedBlockId, BTreeSet<RegisterUnitId>>,
) -> BlockLiveness {
    let mut virtual_live = virtual_exit[&block.id].clone();
    let mut unit_live = unit_exit[&block.id].clone();
    let mut instructions = Vec::new();
    for instruction in block_instructions(block).into_iter().rev() {
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
        .into_iter()
        .enumerate()
        .map(|(ordinal, successor)| SuccessorLiveness {
            terminator: terminator.id,
            polarity_ordinal: ordinal as u8,
            psi_edge: successor.psi_edge,
            target: successor.block,
            virtual_live: sorted(&virtual_entry[&successor.block]),
            unit_live: sorted(&unit_entry[&successor.block]),
        })
        .collect();
    BlockLiveness {
        block: block.id,
        source_block: block.source_block,
        virtual_live_in: sorted(&virtual_entry[&block.id]),
        virtual_live_out: sorted(&virtual_exit[&block.id]),
        unit_live_in: sorted(&unit_entry[&block.id]),
        unit_live_out: sorted(&unit_exit[&block.id]),
        instructions,
        successors,
    }
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

fn successor_blocks(block: &SelectedBlock) -> Vec<selected_instructions::SelectedBlockId> {
    control::successors(&block.terminator)
        .into_iter()
        .map(|successor| successor.block)
        .collect()
}

fn block_instructions(block: &SelectedBlock) -> Vec<&SelectedInstruction> {
    block
        .instructions
        .iter()
        .chain(std::iter::once(control::instruction(&block.terminator)))
        .collect()
}

fn sorted<T: Copy + Ord>(values: &BTreeSet<T>) -> Vec<T> {
    values.iter().copied().collect()
}
