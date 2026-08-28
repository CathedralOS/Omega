use std::collections::{BTreeMap, BTreeSet};

use crate::model::{
    TerminalBlockLiveness, TerminalEntryDefinition, TerminalFunctionLiveness,
    TerminalInstructionLiveness, TerminalLivenessError, TerminalLivenessPlan,
    TerminalLivenessPosition, TerminalOperandPosition, TerminalSuccessorLiveness,
};
use omega_register_model::{RegisterOperandAccess, RegisterUnitId};
use omega_terminal_selected_instructions::{
    TerminalSelectedBlock, TerminalSelectedFunction, TerminalSelectedInstruction,
    TerminalSelectedStructuralUnitFunction, TerminalSelectedTerminator, TerminalVirtualRegisterId,
    TerminalVirtualRegisterOrigin,
};

pub(crate) fn compute_terminal_liveness(
    selected: &impl crate::ValidatedTerminalSelectedAnalysis,
) -> Result<TerminalLivenessPlan, TerminalLivenessError> {
    let plan = selected.selected_plan();
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
    Ok(TerminalLivenessPlan {
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
    function: &TerminalSelectedStructuralUnitFunction,
) -> Result<TerminalFunctionLiveness, TerminalLivenessError> {
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
struct StructuralUnitInstructionFacts<'a> {
    id: omega_terminal_selected_instructions::TerminalSelectedInstructionId,
    uses: &'a [RegisterUnitId],
    defs: &'a [RegisterUnitId],
    clobbers: &'a [RegisterUnitId],
}

fn compute_structural_unit_facts(
    function_index: usize,
    machine: psi_core::MachineId,
    entry_block: omega_terminal_selected_instructions::TerminalSelectedBlockId,
    source_entry_block: psi_core::BlockId,
    instructions: &[StructuralUnitInstructionFacts<'_>],
) -> Result<TerminalFunctionLiveness, TerminalLivenessError> {
    let mut unit_live = BTreeSet::new();
    let mut rows = Vec::with_capacity(instructions.len());
    for (ordinal, instruction) in instructions.iter().enumerate().rev() {
        let position = TerminalLivenessPosition(u32::try_from(ordinal).map_err(|_| {
            TerminalLivenessError::NonDensePositions {
                function: function_index,
            }
        })?);
        let unit_live_out = sorted(&unit_live);
        for unit in instruction.defs.iter().chain(instruction.clobbers) {
            unit_live.remove(unit);
        }
        unit_live.extend(instruction.uses.iter().copied());
        rows.push(TerminalInstructionLiveness {
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
    Ok(TerminalFunctionLiveness {
        machine,
        entry_definitions: Vec::new(),
        operand_positions: Vec::new(),
        blocks: vec![TerminalBlockLiveness {
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
    function: &TerminalSelectedFunction,
) -> Result<TerminalFunctionLiveness, TerminalLivenessError> {
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
            positions.insert(instruction.id, TerminalLivenessPosition(next_position));
            next_position =
                next_position
                    .checked_add(1)
                    .ok_or(TerminalLivenessError::NonDensePositions {
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
                TerminalVirtualRegisterOrigin::EntryParameter { .. }
            )
        })
        .map(|register| TerminalEntryDefinition {
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
                .map(move |operand| TerminalOperandPosition {
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
    Ok(TerminalFunctionLiveness {
        machine: function.machine,
        entry_definitions,
        operand_positions,
        blocks,
    })
}

fn reject_unsupported_constraints(
    function_index: usize,
    function: &TerminalSelectedFunction,
) -> Result<(), TerminalLivenessError> {
    let mut tied_edges = Vec::new();
    let mut early_rows = Vec::new();
    for instruction in function.blocks.iter().flat_map(block_instructions) {
        for operand in &instruction.operands {
            if operand.access == RegisterOperandAccess::UseDef {
                return Err(TerminalLivenessError::UnsupportedUseDef {
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
                return Err(TerminalLivenessError::UnsupportedEarlyClobber {
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
                return Err(TerminalLivenessError::UnsupportedTiedOperand {
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
                return Err(TerminalLivenessError::UnsupportedTiedOperand {
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
            return Err(TerminalLivenessError::UnsupportedEarlyClobber {
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
    seed: TerminalVirtualRegisterId,
    edges: &[(TerminalVirtualRegisterId, TerminalVirtualRegisterId)],
) -> BTreeSet<TerminalVirtualRegisterId> {
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
    block: &TerminalSelectedBlock,
    mut virtual_live: BTreeSet<TerminalVirtualRegisterId>,
    mut unit_live: BTreeSet<RegisterUnitId>,
) -> (
    BTreeSet<TerminalVirtualRegisterId>,
    BTreeSet<RegisterUnitId>,
) {
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
    block: &TerminalSelectedBlock,
    positions: &BTreeMap<
        omega_terminal_selected_instructions::TerminalSelectedInstructionId,
        TerminalLivenessPosition,
    >,
    virtual_entry: &BTreeMap<
        omega_terminal_selected_instructions::TerminalSelectedBlockId,
        BTreeSet<TerminalVirtualRegisterId>,
    >,
    virtual_exit: &BTreeMap<
        omega_terminal_selected_instructions::TerminalSelectedBlockId,
        BTreeSet<TerminalVirtualRegisterId>,
    >,
    unit_entry: &BTreeMap<
        omega_terminal_selected_instructions::TerminalSelectedBlockId,
        BTreeSet<RegisterUnitId>,
    >,
    unit_exit: &BTreeMap<
        omega_terminal_selected_instructions::TerminalSelectedBlockId,
        BTreeSet<RegisterUnitId>,
    >,
) -> TerminalBlockLiveness {
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
        instructions.push(TerminalInstructionLiveness {
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
    let successors = match &block.terminator {
        TerminalSelectedTerminator::ConditionalBranch {
            instruction,
            when_nonzero,
            when_zero,
        } => [when_nonzero, when_zero]
            .into_iter()
            .enumerate()
            .map(|(ordinal, successor)| TerminalSuccessorLiveness {
                terminator: instruction.id,
                polarity_ordinal: ordinal as u8,
                psi_edge: successor.psi_edge,
                target: successor.block,
                virtual_live: sorted(&virtual_entry[&successor.block]),
                unit_live: sorted(&unit_entry[&successor.block]),
            })
            .collect(),
        TerminalSelectedTerminator::Return { .. } => Vec::new(),
    };
    TerminalBlockLiveness {
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
    instruction: &TerminalSelectedInstruction,
) -> (
    BTreeSet<TerminalVirtualRegisterId>,
    BTreeSet<TerminalVirtualRegisterId>,
) {
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

fn successor_blocks(
    block: &TerminalSelectedBlock,
) -> Vec<omega_terminal_selected_instructions::TerminalSelectedBlockId> {
    match &block.terminator {
        TerminalSelectedTerminator::ConditionalBranch {
            when_nonzero,
            when_zero,
            ..
        } => vec![when_nonzero.block, when_zero.block],
        TerminalSelectedTerminator::Return { .. } => Vec::new(),
    }
}

fn block_instructions(block: &TerminalSelectedBlock) -> Vec<&TerminalSelectedInstruction> {
    block
        .instructions
        .iter()
        .chain(std::iter::once(match &block.terminator {
            TerminalSelectedTerminator::ConditionalBranch { instruction, .. }
            | TerminalSelectedTerminator::Return { instruction, .. } => instruction,
        }))
        .collect()
}

fn sorted<T: Copy + Ord>(values: &BTreeSet<T>) -> Vec<T> {
    values.iter().copied().collect()
}

#[cfg(test)]
pub(crate) mod tests {
    use omega_register_model::{
        RegisterClassId, RegisterConstraintFamily, RegisterConstraintKey, RegisterOperandAccess,
        RegisterUnitId,
    };
    use omega_terminal_selected_instructions::{
        TerminalSelectedBlock, TerminalSelectedBlockId, TerminalSelectedFunction,
        TerminalSelectedInstruction, TerminalSelectedInstructionId,
        TerminalSelectedInstructionKind, TerminalSelectedInstructionProvenance,
        TerminalSelectedOperand, TerminalSelectedTerminator, TerminalVirtualRegisterId,
    };
    use psi_core::{BlockId, EdgeId, MachineId};

    use super::{
        StructuralUnitInstructionFacts, compute_structural_unit_facts,
        reject_unsupported_constraints,
    };
    use crate::TerminalLivenessError;

    #[test]
    fn structural_unit_call_and_terminal_callee_retain_exact_unit_liveness() {
        let caller_machine = MachineId::new(1).unwrap();
        let callee_machine = MachineId::new(2).unwrap();
        let block = TerminalSelectedBlockId(0);
        let caller_source = BlockId::new(1).unwrap();
        let callee_source = BlockId::new(2).unwrap();
        let call_uses = [RegisterUnitId(1), RegisterUnitId(2)];
        let call_defs = [RegisterUnitId(3)];
        let call_clobbers = [RegisterUnitId(4)];
        let return_uses = [RegisterUnitId(3)];
        let caller = compute_structural_unit_facts(
            0,
            caller_machine,
            block,
            caller_source,
            &[
                StructuralUnitInstructionFacts {
                    id: TerminalSelectedInstructionId(0),
                    uses: &call_uses,
                    defs: &call_defs,
                    clobbers: &call_clobbers,
                },
                StructuralUnitInstructionFacts {
                    id: TerminalSelectedInstructionId(1),
                    uses: &return_uses,
                    defs: &[],
                    clobbers: &[],
                },
            ],
        )
        .unwrap();
        assert!(caller.entry_definitions.is_empty());
        assert!(caller.operand_positions.is_empty());
        assert_eq!(caller.blocks[0].unit_live_in, call_uses);
        assert!(caller.blocks[0].unit_live_out.is_empty());
        assert_eq!(caller.blocks[0].instructions.len(), 2);
        assert_eq!(
            caller.blocks[0].instructions[0].position,
            crate::TerminalLivenessPosition(0)
        );
        assert_eq!(caller.blocks[0].instructions[0].unit_live_out, return_uses);
        assert_eq!(
            caller.blocks[0].instructions[0].unit_clobbers,
            call_clobbers
        );
        assert_eq!(
            caller.blocks[0].instructions[1].instruction,
            TerminalSelectedInstructionId(1)
        );

        let callee_uses = [RegisterUnitId(5)];
        let callee_defs = [RegisterUnitId(6)];
        let callee = compute_structural_unit_facts(
            1,
            callee_machine,
            block,
            callee_source,
            &[StructuralUnitInstructionFacts {
                id: TerminalSelectedInstructionId(0),
                uses: &callee_uses,
                defs: &callee_defs,
                clobbers: &[],
            }],
        )
        .unwrap();
        assert_eq!(callee.machine, callee_machine);
        assert_eq!(callee.blocks[0].instructions.len(), 1);
        assert_eq!(callee.blocks[0].unit_live_in, callee_uses);
        assert_eq!(callee.blocks[0].instructions[0].unit_defs, callee_defs);
    }

    fn function_with_operand(access: RegisterOperandAccess) -> TerminalSelectedFunction {
        let key = RegisterConstraintKey {
            family: RegisterConstraintFamily::Instruction,
            variant: 99,
        };
        let instruction = TerminalSelectedInstruction {
            id: TerminalSelectedInstructionId(0),
            kind: TerminalSelectedInstructionKind::CompareI64Zero,
            constraint: key,
            operands: vec![TerminalSelectedOperand {
                operand: 0,
                virtual_register: TerminalVirtualRegisterId(0),
                access,
                class: RegisterClassId(0),
                fixed_view: None,
                tied_to: None,
                early_clobber: false,
            }],
            implicit_uses: Vec::new(),
            implicit_defs: Vec::new(),
            clobbers: Vec::new(),
            provenance: TerminalSelectedInstructionProvenance::default(),
        };
        TerminalSelectedFunction {
            machine: MachineId::new(1).unwrap(),
            attachment: None,
            provenance: Default::default(),
            entry_block: TerminalSelectedBlockId(0),
            virtual_registers: Vec::new(),
            blocks: vec![TerminalSelectedBlock {
                id: TerminalSelectedBlockId(0),
                source_block: BlockId::new(1).unwrap(),
                instructions: vec![instruction],
                terminator: TerminalSelectedTerminator::Return {
                    instruction: TerminalSelectedInstruction {
                        id: TerminalSelectedInstructionId(1),
                        kind: TerminalSelectedInstructionKind::ReturnI64,
                        constraint: key,
                        operands: Vec::new(),
                        implicit_uses: Vec::new(),
                        implicit_defs: Vec::new(),
                        clobbers: Vec::new(),
                        provenance: TerminalSelectedInstructionProvenance::default(),
                    },
                    psi_return_edge: EdgeId::new(1).unwrap(),
                },
            }],
        }
    }

    pub(crate) fn supported_tied_function() -> TerminalSelectedFunction {
        let mut function = function_with_operand(RegisterOperandAccess::Use);
        function.blocks[0].instructions[0]
            .operands
            .push(TerminalSelectedOperand {
                operand: 1,
                virtual_register: TerminalVirtualRegisterId(1),
                access: RegisterOperandAccess::Def,
                class: RegisterClassId(0),
                fixed_view: None,
                tied_to: Some(0),
                early_clobber: false,
            });
        function
    }

    pub(crate) fn supported_tied_component_function() -> TerminalSelectedFunction {
        let mut function = supported_tied_function();
        let key = function.blocks[0].instructions[0].constraint;
        function.blocks[0]
            .instructions
            .push(TerminalSelectedInstruction {
                id: TerminalSelectedInstructionId(1),
                kind: TerminalSelectedInstructionKind::CompareI64Zero,
                constraint: key,
                operands: vec![
                    TerminalSelectedOperand {
                        operand: 0,
                        virtual_register: TerminalVirtualRegisterId(1),
                        access: RegisterOperandAccess::Use,
                        class: RegisterClassId(0),
                        fixed_view: None,
                        tied_to: None,
                        early_clobber: false,
                    },
                    TerminalSelectedOperand {
                        operand: 1,
                        virtual_register: TerminalVirtualRegisterId(2),
                        access: RegisterOperandAccess::Def,
                        class: RegisterClassId(0),
                        fixed_view: None,
                        tied_to: Some(0),
                        early_clobber: false,
                    },
                ],
                implicit_uses: Vec::new(),
                implicit_defs: Vec::new(),
                clobbers: Vec::new(),
                provenance: TerminalSelectedInstructionProvenance::default(),
            });
        let TerminalSelectedTerminator::Return { instruction, .. } =
            &mut function.blocks[0].terminator
        else {
            unreachable!()
        };
        instruction.id = TerminalSelectedInstructionId(2);
        function
    }

    pub(crate) fn supported_early_clobber_function() -> TerminalSelectedFunction {
        let mut function = function_with_operand(RegisterOperandAccess::Use);
        function.blocks[0].instructions[0]
            .operands
            .push(TerminalSelectedOperand {
                operand: 1,
                virtual_register: TerminalVirtualRegisterId(1),
                access: RegisterOperandAccess::Def,
                class: RegisterClassId(0),
                fixed_view: None,
                tied_to: None,
                early_clobber: true,
            });
        function
    }

    pub(crate) fn supported_multiple_early_clobber_function() -> TerminalSelectedFunction {
        let mut function = supported_early_clobber_function();
        let key = function.blocks[0].instructions[0].constraint;
        function.blocks[0]
            .instructions
            .push(TerminalSelectedInstruction {
                id: TerminalSelectedInstructionId(1),
                kind: TerminalSelectedInstructionKind::CompareI64Zero,
                constraint: key,
                operands: vec![
                    TerminalSelectedOperand {
                        operand: 0,
                        virtual_register: TerminalVirtualRegisterId(1),
                        access: RegisterOperandAccess::Use,
                        class: RegisterClassId(0),
                        fixed_view: None,
                        tied_to: None,
                        early_clobber: false,
                    },
                    TerminalSelectedOperand {
                        operand: 1,
                        virtual_register: TerminalVirtualRegisterId(2),
                        access: RegisterOperandAccess::Def,
                        class: RegisterClassId(0),
                        fixed_view: None,
                        tied_to: None,
                        early_clobber: true,
                    },
                ],
                implicit_uses: Vec::new(),
                implicit_defs: Vec::new(),
                clobbers: Vec::new(),
                provenance: TerminalSelectedInstructionProvenance::default(),
            });
        let TerminalSelectedTerminator::Return { instruction, .. } =
            &mut function.blocks[0].terminator
        else {
            unreachable!()
        };
        instruction.id = TerminalSelectedInstructionId(2);
        function
    }

    pub(crate) fn supported_isolated_tied_early_clobber_function() -> TerminalSelectedFunction {
        let mut function = function_with_operand(RegisterOperandAccess::Use);
        function.blocks[0].instructions[0].operands.extend([
            TerminalSelectedOperand {
                operand: 1,
                virtual_register: TerminalVirtualRegisterId(1),
                access: RegisterOperandAccess::Use,
                class: RegisterClassId(0),
                fixed_view: None,
                tied_to: None,
                early_clobber: false,
            },
            TerminalSelectedOperand {
                operand: 2,
                virtual_register: TerminalVirtualRegisterId(2),
                access: RegisterOperandAccess::Def,
                class: RegisterClassId(0),
                fixed_view: None,
                tied_to: Some(0),
                early_clobber: true,
            },
        ]);
        function
    }

    pub(crate) fn supported_multiple_isolated_tied_early_clobber_function()
    -> TerminalSelectedFunction {
        let mut function = supported_isolated_tied_early_clobber_function();
        let key = function.blocks[0].instructions[0].constraint;
        function.blocks[0]
            .instructions
            .push(TerminalSelectedInstruction {
                id: TerminalSelectedInstructionId(1),
                kind: TerminalSelectedInstructionKind::CompareI64Zero,
                constraint: key,
                operands: vec![
                    TerminalSelectedOperand {
                        operand: 0,
                        virtual_register: TerminalVirtualRegisterId(3),
                        access: RegisterOperandAccess::Use,
                        class: RegisterClassId(0),
                        fixed_view: None,
                        tied_to: None,
                        early_clobber: false,
                    },
                    TerminalSelectedOperand {
                        operand: 1,
                        virtual_register: TerminalVirtualRegisterId(4),
                        access: RegisterOperandAccess::Use,
                        class: RegisterClassId(0),
                        fixed_view: None,
                        tied_to: None,
                        early_clobber: false,
                    },
                    TerminalSelectedOperand {
                        operand: 2,
                        virtual_register: TerminalVirtualRegisterId(5),
                        access: RegisterOperandAccess::Def,
                        class: RegisterClassId(0),
                        fixed_view: None,
                        tied_to: Some(0),
                        early_clobber: true,
                    },
                ],
                implicit_uses: Vec::new(),
                implicit_defs: Vec::new(),
                clobbers: Vec::new(),
                provenance: TerminalSelectedInstructionProvenance::default(),
            });
        let TerminalSelectedTerminator::Return { instruction, .. } =
            &mut function.blocks[0].terminator
        else {
            unreachable!()
        };
        instruction.id = TerminalSelectedInstructionId(2);
        function
    }

    pub(crate) fn supported_component_tied_early_clobber_function() -> TerminalSelectedFunction {
        let mut function = supported_isolated_tied_early_clobber_function();
        let key = function.blocks[0].instructions[0].constraint;
        let mut early = function.blocks[0].instructions.remove(0);
        early.id = TerminalSelectedInstructionId(1);
        early.operands[0].virtual_register = TerminalVirtualRegisterId(1);
        early.operands[1].virtual_register = TerminalVirtualRegisterId(2);
        early.operands[2].virtual_register = TerminalVirtualRegisterId(3);
        function.blocks[0]
            .instructions
            .push(TerminalSelectedInstruction {
                id: TerminalSelectedInstructionId(0),
                kind: TerminalSelectedInstructionKind::CompareI64Zero,
                constraint: key,
                operands: vec![
                    TerminalSelectedOperand {
                        operand: 0,
                        virtual_register: TerminalVirtualRegisterId(0),
                        access: RegisterOperandAccess::Use,
                        class: RegisterClassId(0),
                        fixed_view: None,
                        tied_to: None,
                        early_clobber: false,
                    },
                    TerminalSelectedOperand {
                        operand: 1,
                        virtual_register: TerminalVirtualRegisterId(1),
                        access: RegisterOperandAccess::Def,
                        class: RegisterClassId(0),
                        fixed_view: None,
                        tied_to: Some(0),
                        early_clobber: false,
                    },
                ],
                implicit_uses: Vec::new(),
                implicit_defs: Vec::new(),
                clobbers: Vec::new(),
                provenance: TerminalSelectedInstructionProvenance::default(),
            });
        function.blocks[0].instructions.push(early);
        let TerminalSelectedTerminator::Return { instruction, .. } =
            &mut function.blocks[0].terminator
        else {
            unreachable!()
        };
        instruction.id = TerminalSelectedInstructionId(2);
        function
    }

    pub(crate) fn supported_multiple_component_tied_early_clobber_function()
    -> TerminalSelectedFunction {
        let mut function = supported_component_tied_early_clobber_function();
        let mut ordinary = function.blocks[0].instructions[0].clone();
        ordinary.id = TerminalSelectedInstructionId(2);
        for operand in &mut ordinary.operands {
            operand.virtual_register.0 += 4;
        }
        let mut early = function.blocks[0].instructions[1].clone();
        early.id = TerminalSelectedInstructionId(3);
        for operand in &mut early.operands {
            operand.virtual_register.0 += 4;
        }
        function.blocks[0].instructions.extend([ordinary, early]);
        let TerminalSelectedTerminator::Return { instruction, .. } =
            &mut function.blocks[0].terminator
        else {
            unreachable!()
        };
        instruction.id = TerminalSelectedInstructionId(4);
        function
    }

    #[test]
    fn admits_only_distinct_use_to_def_ties_and_rejects_other_phase_frontiers() {
        let use_def = function_with_operand(RegisterOperandAccess::UseDef);
        assert!(matches!(
            reject_unsupported_constraints(0, &use_def),
            Err(TerminalLivenessError::UnsupportedUseDef { .. })
        ));

        let supported = supported_tied_function();
        assert_eq!(reject_unsupported_constraints(0, &supported), Ok(()));

        let component = supported_tied_component_function();
        assert_eq!(reject_unsupported_constraints(0, &component), Ok(()));

        let mut multiple_defs = supported_tied_function();
        multiple_defs.blocks[0].instructions[0]
            .operands
            .push(TerminalSelectedOperand {
                operand: 2,
                virtual_register: TerminalVirtualRegisterId(2),
                access: RegisterOperandAccess::Def,
                class: RegisterClassId(0),
                fixed_view: None,
                tied_to: Some(0),
                early_clobber: false,
            });
        assert_eq!(reject_unsupported_constraints(0, &multiple_defs), Ok(()));

        let early_supported = supported_early_clobber_function();
        assert_eq!(reject_unsupported_constraints(0, &early_supported), Ok(()));

        let multiple_early = supported_multiple_early_clobber_function();
        assert_eq!(reject_unsupported_constraints(0, &multiple_early), Ok(()));

        let composed = supported_isolated_tied_early_clobber_function();
        assert_eq!(reject_unsupported_constraints(0, &composed), Ok(()));

        let multiple_composed = supported_multiple_isolated_tied_early_clobber_function();
        assert_eq!(
            reject_unsupported_constraints(0, &multiple_composed),
            Ok(())
        );

        let component_composed = supported_component_tied_early_clobber_function();
        assert_eq!(
            reject_unsupported_constraints(0, &component_composed),
            Ok(())
        );

        let multiple_components = supported_multiple_component_tied_early_clobber_function();
        assert_eq!(
            reject_unsupported_constraints(0, &multiple_components),
            Ok(())
        );

        let mut ordinary_reuse = supported_isolated_tied_early_clobber_function();
        let key = ordinary_reuse.blocks[0].instructions[0].constraint;
        ordinary_reuse.blocks[0]
            .instructions
            .push(TerminalSelectedInstruction {
                id: TerminalSelectedInstructionId(1),
                kind: TerminalSelectedInstructionKind::CompareI64Zero,
                constraint: key,
                operands: vec![TerminalSelectedOperand {
                    operand: 0,
                    virtual_register: TerminalVirtualRegisterId(2),
                    access: RegisterOperandAccess::Use,
                    class: RegisterClassId(0),
                    fixed_view: None,
                    tied_to: None,
                    early_clobber: false,
                }],
                implicit_uses: Vec::new(),
                implicit_defs: Vec::new(),
                clobbers: Vec::new(),
                provenance: TerminalSelectedInstructionProvenance::default(),
            });
        assert_eq!(reject_unsupported_constraints(0, &ordinary_reuse), Ok(()));

        let mut no_unrelated = supported_isolated_tied_early_clobber_function();
        no_unrelated.blocks[0].instructions[0].operands.remove(1);
        assert!(matches!(
            reject_unsupported_constraints(0, &no_unrelated),
            Err(TerminalLivenessError::UnsupportedEarlyClobber { .. })
        ));

        let mut tied_unrelated = supported_isolated_tied_early_clobber_function();
        tied_unrelated.blocks[0].instructions[0].operands[1].tied_to = Some(0);
        assert!(matches!(
            reject_unsupported_constraints(0, &tied_unrelated),
            Err(TerminalLivenessError::UnsupportedEarlyClobber { .. })
        ));

        let mut extra_definition = supported_isolated_tied_early_clobber_function();
        extra_definition.blocks[0].instructions[0].operands[1].access = RegisterOperandAccess::Def;
        assert!(matches!(
            reject_unsupported_constraints(0, &extra_definition),
            Err(TerminalLivenessError::UnsupportedEarlyClobber { .. })
        ));

        let mut duplicate_composed = supported_isolated_tied_early_clobber_function();
        duplicate_composed.blocks[0].instructions[0].operands[1].virtual_register =
            TerminalVirtualRegisterId(0);
        assert!(matches!(
            reject_unsupported_constraints(0, &duplicate_composed),
            Err(TerminalLivenessError::UnsupportedEarlyClobber { .. })
        ));

        let mut nonisolated = supported_isolated_tied_early_clobber_function();
        let key = nonisolated.blocks[0].instructions[0].constraint;
        nonisolated.blocks[0]
            .instructions
            .push(TerminalSelectedInstruction {
                id: TerminalSelectedInstructionId(1),
                kind: TerminalSelectedInstructionKind::CompareI64Zero,
                constraint: key,
                operands: vec![
                    TerminalSelectedOperand {
                        operand: 0,
                        virtual_register: TerminalVirtualRegisterId(2),
                        access: RegisterOperandAccess::Use,
                        class: RegisterClassId(0),
                        fixed_view: None,
                        tied_to: None,
                        early_clobber: false,
                    },
                    TerminalSelectedOperand {
                        operand: 1,
                        virtual_register: TerminalVirtualRegisterId(3),
                        access: RegisterOperandAccess::Def,
                        class: RegisterClassId(0),
                        fixed_view: None,
                        tied_to: Some(0),
                        early_clobber: false,
                    },
                ],
                implicit_uses: Vec::new(),
                implicit_defs: Vec::new(),
                clobbers: Vec::new(),
                provenance: TerminalSelectedInstructionProvenance::default(),
            });
        assert!(matches!(
            reject_unsupported_constraints(0, &nonisolated),
            Ok(())
        ));

        let mut same_component_second_early = supported_component_tied_early_clobber_function();
        same_component_second_early.blocks[0].instructions[0].operands[1].early_clobber = true;
        same_component_second_early.blocks[0].instructions[0]
            .operands
            .push(TerminalSelectedOperand {
                operand: 2,
                virtual_register: TerminalVirtualRegisterId(4),
                access: RegisterOperandAccess::Use,
                class: RegisterClassId(0),
                fixed_view: None,
                tied_to: None,
                early_clobber: false,
            });
        assert!(matches!(
            reject_unsupported_constraints(0, &same_component_second_early),
            Err(TerminalLivenessError::UnsupportedEarlyClobber { .. })
        ));

        let mut tied = function_with_operand(RegisterOperandAccess::Use);
        tied.blocks[0].instructions[0].operands[0].tied_to = Some(0);
        assert!(matches!(
            reject_unsupported_constraints(0, &tied),
            Err(TerminalLivenessError::UnsupportedTiedOperand { .. })
        ));

        let mut early = function_with_operand(RegisterOperandAccess::Def);
        early.blocks[0].instructions[0].operands[0].early_clobber = true;
        assert!(matches!(
            reject_unsupported_constraints(0, &early),
            Err(TerminalLivenessError::UnsupportedEarlyClobber { .. })
        ));

        let mut duplicate = supported_early_clobber_function();
        duplicate.blocks[0].instructions[0].operands[1].virtual_register =
            TerminalVirtualRegisterId(0);
        assert!(matches!(
            reject_unsupported_constraints(0, &duplicate),
            Err(TerminalLivenessError::UnsupportedEarlyClobber { .. })
        ));

        let mut second_definition = supported_early_clobber_function();
        second_definition.blocks[0].instructions[0]
            .operands
            .push(TerminalSelectedOperand {
                operand: 2,
                virtual_register: TerminalVirtualRegisterId(2),
                access: RegisterOperandAccess::Def,
                class: RegisterClassId(0),
                fixed_view: None,
                tied_to: None,
                early_clobber: false,
            });
        assert!(matches!(
            reject_unsupported_constraints(0, &second_definition),
            Err(TerminalLivenessError::UnsupportedEarlyClobber { .. })
        ));

        let mut tied_overlap = supported_early_clobber_function();
        let key = tied_overlap.blocks[0].instructions[0].constraint;
        tied_overlap.blocks[0]
            .instructions
            .push(TerminalSelectedInstruction {
                id: TerminalSelectedInstructionId(1),
                kind: TerminalSelectedInstructionKind::CompareI64Zero,
                constraint: key,
                operands: vec![
                    TerminalSelectedOperand {
                        operand: 0,
                        virtual_register: TerminalVirtualRegisterId(1),
                        access: RegisterOperandAccess::Use,
                        class: RegisterClassId(0),
                        fixed_view: None,
                        tied_to: None,
                        early_clobber: false,
                    },
                    TerminalSelectedOperand {
                        operand: 1,
                        virtual_register: TerminalVirtualRegisterId(2),
                        access: RegisterOperandAccess::Def,
                        class: RegisterClassId(0),
                        fixed_view: None,
                        tied_to: Some(0),
                        early_clobber: false,
                    },
                ],
                implicit_uses: Vec::new(),
                implicit_defs: Vec::new(),
                clobbers: Vec::new(),
                provenance: TerminalSelectedInstructionProvenance::default(),
            });
        assert!(matches!(
            reject_unsupported_constraints(0, &tied_overlap),
            Err(TerminalLivenessError::UnsupportedEarlyClobber { .. })
        ));
    }
}
