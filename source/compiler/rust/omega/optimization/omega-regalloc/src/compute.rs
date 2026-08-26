use std::collections::{BTreeMap, BTreeSet};

use crate::model::{
    TerminalBlockLiveness, TerminalEntryDefinition, TerminalFunctionLiveness,
    TerminalInstructionLiveness, TerminalLivenessError, TerminalLivenessPlan,
    TerminalLivenessPosition, TerminalOperandPosition, TerminalSuccessorLiveness,
};
use omega_register_model::{RegisterOperandAccess, RegisterUnitId};
use omega_terminal_selected_instructions::{
    TerminalSelectedBlock, TerminalSelectedFunction, TerminalSelectedInstruction,
    TerminalSelectedTerminator, TerminalVirtualRegisterId, TerminalVirtualRegisterOrigin,
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
    Ok(TerminalLivenessPlan {
        selected: selected.selected_identity(),
        optimization_unit: selected.optimization_unit_identity(),
        fuel_schedule: selected.fuel_schedule_identity(),
        target: plan.target,
        functions,
    })
}

fn compute_function(
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
    for instruction in function.blocks.iter().flat_map(block_instructions) {
        for operand in &instruction.operands {
            if operand.access == RegisterOperandAccess::UseDef {
                return Err(TerminalLivenessError::UnsupportedUseDef {
                    function: function_index,
                    instruction: instruction.id.0,
                    operand: operand.operand,
                });
            }
            if operand.tied_to.is_some() {
                return Err(TerminalLivenessError::UnsupportedTiedOperand {
                    function: function_index,
                    instruction: instruction.id.0,
                    operand: operand.operand,
                });
            }
            if operand.early_clobber {
                return Err(TerminalLivenessError::UnsupportedEarlyClobber {
                    function: function_index,
                    instruction: instruction.id.0,
                    operand: operand.operand,
                });
            }
        }
    }
    Ok(())
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
mod tests {
    use omega_register_model::{
        RegisterClassId, RegisterConstraintFamily, RegisterConstraintKey, RegisterOperandAccess,
    };
    use omega_terminal_selected_instructions::{
        TerminalSelectedBlock, TerminalSelectedBlockId, TerminalSelectedFunction,
        TerminalSelectedInstruction, TerminalSelectedInstructionId,
        TerminalSelectedInstructionKind, TerminalSelectedInstructionProvenance,
        TerminalSelectedOperand, TerminalSelectedTerminator, TerminalVirtualRegisterId,
    };
    use psi_core::{BlockId, EdgeId, MachineId};

    use super::reject_unsupported_constraints;
    use crate::TerminalLivenessError;

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

    #[test]
    fn v1_rejects_use_def_ties_and_early_clobber_at_the_analysis_boundary() {
        let use_def = function_with_operand(RegisterOperandAccess::UseDef);
        assert!(matches!(
            reject_unsupported_constraints(0, &use_def),
            Err(TerminalLivenessError::UnsupportedUseDef { .. })
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
    }
}
