use std::collections::{BTreeMap, BTreeSet};

use crate::identity::terminal_liveness_identity;
use crate::model::{
    TerminalBlockLiveness, TerminalEntryDefinition, TerminalFunctionLiveness,
    TerminalInstructionLiveness, TerminalLivenessError, TerminalLivenessPlan,
    TerminalLivenessPosition, TerminalLivenessValidationReceipt, TerminalOperandPosition,
    TerminalSuccessorLiveness, ValidatedTerminalLiveness,
};
use omega_register_model::{RegisterOperandAccess, RegisterUnitId};
use omega_terminal_selected_instructions::{
    TerminalSelectedBlock, TerminalSelectedFunction, TerminalSelectedInstruction,
    TerminalSelectedTerminator, TerminalVirtualRegisterId, TerminalVirtualRegisterOrigin,
};

pub fn validate_terminal_liveness(
    selected: &impl crate::ValidatedTerminalSelectedAnalysis,
    plan: TerminalLivenessPlan,
) -> Result<ValidatedTerminalLiveness, TerminalLivenessError> {
    if plan.selected != selected.selected_identity()
        || plan.optimization_unit != selected.optimization_unit_identity()
        || plan.fuel_schedule != selected.fuel_schedule_identity()
        || plan.target != selected.selected_plan().target
        || plan.functions.len() != selected.selected_plan().functions.len()
    {
        return Err(TerminalLivenessError::RootMismatch);
    }
    for (function_index, (selected_function, actual)) in selected
        .selected_plan()
        .functions
        .iter()
        .zip(&plan.functions)
        .enumerate()
    {
        let expected = replay_function(function_index, selected_function)?;
        validate_function(function_index, actual, &expected)?;
    }
    let block_count = plan
        .functions
        .iter()
        .map(|function| function.blocks.len())
        .sum();
    let instruction_count = plan
        .functions
        .iter()
        .flat_map(|function| &function.blocks)
        .map(|block| block.instructions.len())
        .sum();
    let successor_count = plan
        .functions
        .iter()
        .flat_map(|function| &function.blocks)
        .map(|block| block.successors.len())
        .sum();
    let tied_pair_count = plan
        .functions
        .iter()
        .flat_map(|function| &function.operand_positions)
        .filter(|operand| operand.tied_to.is_some())
        .count();
    let early_clobber_count = plan
        .functions
        .iter()
        .flat_map(|function| &function.operand_positions)
        .filter(|operand| operand.early_clobber)
        .count();
    let receipt = TerminalLivenessValidationReceipt {
        identity: terminal_liveness_identity(&plan),
        selected: plan.selected,
        optimization_unit: plan.optimization_unit,
        fuel_schedule: plan.fuel_schedule,
        function_count: plan.functions.len(),
        block_count,
        virtual_register_count: selected
            .selected_plan()
            .functions
            .iter()
            .map(|function| function.virtual_registers.len())
            .sum(),
        instruction_count,
        successor_count,
        tied_pair_count,
        early_clobber_count,
    };
    Ok(ValidatedTerminalLiveness { plan, receipt })
}

fn validate_function(
    function_index: usize,
    actual: &TerminalFunctionLiveness,
    expected: &TerminalFunctionLiveness,
) -> Result<(), TerminalLivenessError> {
    if actual.machine != expected.machine || actual.blocks.len() != expected.blocks.len() {
        return Err(TerminalLivenessError::FunctionMismatch {
            function: function_index,
        });
    }
    if actual.entry_definitions != expected.entry_definitions
        || actual.operand_positions != expected.operand_positions
    {
        return Err(TerminalLivenessError::FixedConstraintMismatch {
            function: function_index,
        });
    }
    let positions = actual
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .map(|instruction| instruction.position.0)
        .collect::<Vec<_>>();
    let expected_position_count =
        u32::try_from(positions.len()).map_err(|_| TerminalLivenessError::NonDensePositions {
            function: function_index,
        })?;
    if positions != (0..expected_position_count).collect::<Vec<_>>() {
        return Err(TerminalLivenessError::NonDensePositions {
            function: function_index,
        });
    }
    for (actual, expected) in actual.blocks.iter().zip(&expected.blocks) {
        if actual.block != expected.block
            || actual.source_block != expected.source_block
            || actual.instructions.len() != expected.instructions.len()
        {
            return Err(TerminalLivenessError::BlockMismatch {
                function: function_index,
                block: expected.block.0,
            });
        }
        for set in [&actual.virtual_live_in, &actual.virtual_live_out] {
            require_canonical(function_index, None, set)?;
        }
        for set in [&actual.unit_live_in, &actual.unit_live_out] {
            require_canonical(function_index, None, set)?;
        }
        if actual.virtual_live_in != expected.virtual_live_in
            || actual.virtual_live_out != expected.virtual_live_out
            || actual.unit_live_in != expected.unit_live_in
            || actual.unit_live_out != expected.unit_live_out
        {
            return Err(TerminalLivenessError::BlockMismatch {
                function: function_index,
                block: expected.block.0,
            });
        }
        for (actual_instruction, expected_instruction) in
            actual.instructions.iter().zip(&expected.instructions)
        {
            for set in [
                &actual_instruction.virtual_uses,
                &actual_instruction.virtual_defs,
                &actual_instruction.virtual_live_in,
                &actual_instruction.virtual_live_out,
            ] {
                require_canonical(
                    function_index,
                    Some(expected_instruction.instruction.0),
                    set,
                )?;
            }
            for set in [
                &actual_instruction.unit_uses,
                &actual_instruction.unit_defs,
                &actual_instruction.unit_clobbers,
                &actual_instruction.unit_live_in,
                &actual_instruction.unit_live_out,
            ] {
                require_canonical(
                    function_index,
                    Some(expected_instruction.instruction.0),
                    set,
                )?;
            }
            if actual_instruction.position != expected_instruction.position
                || actual_instruction.instruction != expected_instruction.instruction
                || actual_instruction.virtual_uses != expected_instruction.virtual_uses
                || actual_instruction.virtual_defs != expected_instruction.virtual_defs
                || actual_instruction.unit_uses != expected_instruction.unit_uses
                || actual_instruction.unit_defs != expected_instruction.unit_defs
                || actual_instruction.unit_clobbers != expected_instruction.unit_clobbers
            {
                return Err(TerminalLivenessError::InstructionMismatch {
                    function: function_index,
                    instruction: expected_instruction.instruction.0,
                });
            }
            if actual_instruction.virtual_live_in != expected_instruction.virtual_live_in
                || actual_instruction.virtual_live_out != expected_instruction.virtual_live_out
                || actual_instruction.unit_live_in != expected_instruction.unit_live_in
                || actual_instruction.unit_live_out != expected_instruction.unit_live_out
            {
                return Err(TerminalLivenessError::TransferMismatch {
                    function: function_index,
                    instruction: expected_instruction.instruction.0,
                });
            }
        }
        if actual.successors.len() != expected.successors.len() {
            return Err(TerminalLivenessError::SuccessorMismatch {
                function: function_index,
                block: expected.block.0,
                ordinal: 0,
            });
        }
        for (actual_successor, expected_successor) in
            actual.successors.iter().zip(&expected.successors)
        {
            require_canonical(
                function_index,
                Some(expected_successor.terminator.0),
                &actual_successor.virtual_live,
            )?;
            require_canonical(
                function_index,
                Some(expected_successor.terminator.0),
                &actual_successor.unit_live,
            )?;
            if actual_successor != expected_successor {
                return Err(TerminalLivenessError::SuccessorMismatch {
                    function: function_index,
                    block: expected.block.0,
                    ordinal: expected_successor.polarity_ordinal,
                });
            }
        }
    }
    Ok(())
}

fn replay_function(
    function_index: usize,
    function: &TerminalSelectedFunction,
) -> Result<TerminalFunctionLiveness, TerminalLivenessError> {
    reject_v1_unsupported(function_index, function)?;
    let block_ids = function
        .blocks
        .iter()
        .map(|block| block.id)
        .collect::<Vec<_>>();
    let mut v_in = block_ids
        .iter()
        .copied()
        .map(|block| (block, BTreeSet::new()))
        .collect::<BTreeMap<_, _>>();
    let mut v_out = v_in.clone();
    let mut u_in = block_ids
        .iter()
        .copied()
        .map(|block| (block, BTreeSet::new()))
        .collect::<BTreeMap<_, _>>();
    let mut u_out = u_in.clone();
    loop {
        let old = (v_in.clone(), v_out.clone(), u_in.clone(), u_out.clone());
        for block in function.blocks.iter().rev() {
            let targets = match &block.terminator {
                TerminalSelectedTerminator::ConditionalBranch {
                    when_nonzero,
                    when_zero,
                    ..
                } => vec![when_nonzero.block, when_zero.block],
                TerminalSelectedTerminator::Return { .. } => Vec::new(),
            };
            let vo = targets
                .iter()
                .filter_map(|target| v_in.get(target))
                .flat_map(|set| set.iter().copied())
                .collect::<BTreeSet<_>>();
            let uo = targets
                .iter()
                .filter_map(|target| u_in.get(target))
                .flat_map(|set| set.iter().copied())
                .collect::<BTreeSet<_>>();
            let mut vi = vo.clone();
            let mut ui = uo.clone();
            for instruction in ordered_instructions(block).into_iter().rev() {
                for operand in &instruction.operands {
                    if operand.access == RegisterOperandAccess::Def {
                        vi.remove(&operand.virtual_register);
                    }
                }
                for operand in &instruction.operands {
                    if operand.access == RegisterOperandAccess::Use {
                        vi.insert(operand.virtual_register);
                    }
                }
                for unit in instruction
                    .implicit_defs
                    .iter()
                    .chain(&instruction.clobbers)
                {
                    ui.remove(unit);
                }
                ui.extend(instruction.implicit_uses.iter().copied());
            }
            v_out.insert(block.id, vo);
            u_out.insert(block.id, uo);
            v_in.insert(block.id, vi);
            u_in.insert(block.id, ui);
        }
        if old == (v_in.clone(), v_out.clone(), u_in.clone(), u_out.clone()) {
            break;
        }
    }

    let mut ordinal = 0_u32;
    let mut position = BTreeMap::new();
    for block in &function.blocks {
        for instruction in ordered_instructions(block) {
            position.insert(instruction.id, TerminalLivenessPosition(ordinal));
            ordinal = ordinal
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
        .flat_map(ordered_instructions)
        .flat_map(|instruction| {
            let instruction_position = position[&instruction.id];
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
        .map(|block| replay_block(block, &position, &v_in, &v_out, &u_in, &u_out))
        .collect();
    Ok(TerminalFunctionLiveness {
        machine: function.machine,
        entry_definitions,
        operand_positions,
        blocks,
    })
}

fn replay_block(
    block: &TerminalSelectedBlock,
    position: &BTreeMap<
        omega_terminal_selected_instructions::TerminalSelectedInstructionId,
        TerminalLivenessPosition,
    >,
    v_in: &BTreeMap<
        omega_terminal_selected_instructions::TerminalSelectedBlockId,
        BTreeSet<TerminalVirtualRegisterId>,
    >,
    v_out: &BTreeMap<
        omega_terminal_selected_instructions::TerminalSelectedBlockId,
        BTreeSet<TerminalVirtualRegisterId>,
    >,
    u_in: &BTreeMap<
        omega_terminal_selected_instructions::TerminalSelectedBlockId,
        BTreeSet<RegisterUnitId>,
    >,
    u_out: &BTreeMap<
        omega_terminal_selected_instructions::TerminalSelectedBlockId,
        BTreeSet<RegisterUnitId>,
    >,
) -> TerminalBlockLiveness {
    let mut vl = v_out[&block.id].clone();
    let mut ul = u_out[&block.id].clone();
    let mut instructions = Vec::new();
    for instruction in ordered_instructions(block).into_iter().rev() {
        let vlo = collect(&vl);
        let ulo = collect(&ul);
        let uses = instruction
            .operands
            .iter()
            .filter(|operand| operand.access == RegisterOperandAccess::Use)
            .map(|operand| operand.virtual_register)
            .collect::<BTreeSet<_>>();
        let defs = instruction
            .operands
            .iter()
            .filter(|operand| operand.access == RegisterOperandAccess::Def)
            .map(|operand| operand.virtual_register)
            .collect::<BTreeSet<_>>();
        for value in &defs {
            vl.remove(value);
        }
        vl.extend(uses.iter().copied());
        for unit in instruction
            .implicit_defs
            .iter()
            .chain(&instruction.clobbers)
        {
            ul.remove(unit);
        }
        ul.extend(instruction.implicit_uses.iter().copied());
        instructions.push(TerminalInstructionLiveness {
            position: position[&instruction.id],
            instruction: instruction.id,
            virtual_uses: collect(&uses),
            virtual_defs: collect(&defs),
            virtual_live_in: collect(&vl),
            virtual_live_out: vlo,
            unit_uses: instruction.implicit_uses.clone(),
            unit_defs: instruction.implicit_defs.clone(),
            unit_clobbers: instruction.clobbers.clone(),
            unit_live_in: collect(&ul),
            unit_live_out: ulo,
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
                virtual_live: collect(&v_in[&successor.block]),
                unit_live: collect(&u_in[&successor.block]),
            })
            .collect(),
        TerminalSelectedTerminator::Return { .. } => Vec::new(),
    };
    TerminalBlockLiveness {
        block: block.id,
        source_block: block.source_block,
        virtual_live_in: collect(&v_in[&block.id]),
        virtual_live_out: collect(&v_out[&block.id]),
        unit_live_in: collect(&u_in[&block.id]),
        unit_live_out: collect(&u_out[&block.id]),
        instructions,
        successors,
    }
}

fn reject_v1_unsupported(
    function_index: usize,
    function: &TerminalSelectedFunction,
) -> Result<(), TerminalLivenessError> {
    let mut tied_edges = Vec::new();
    let mut early_rows = Vec::new();
    for instruction in function.blocks.iter().flat_map(ordered_instructions) {
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
        if let Some(definition) = early.first().copied() {
            let mut values = Vec::new();
            for operand in &instruction.operands {
                if values.contains(&operand.virtual_register) {
                    return Err(TerminalLivenessError::UnsupportedEarlyClobber {
                        function: function_index,
                        instruction: instruction.id.0,
                        operand: operand.operand,
                    });
                }
                values.push(operand.virtual_register);
            }
            let tied_source = definition.tied_to.and_then(|operand| {
                instruction
                    .operands
                    .iter()
                    .find(|candidate| candidate.operand == operand)
            });
            let source_is_valid = match tied_source {
                None => true,
                Some(source) => {
                    source.access == RegisterOperandAccess::Use
                        && source.operand < definition.operand
                        && source.virtual_register != definition.virtual_register
                        && source.class == definition.class
                        && source.tied_to.is_none()
                }
            };
            let unrelated = instruction
                .operands
                .iter()
                .filter(|operand| {
                    operand.operand != definition.operand
                        && tied_source.is_none_or(|source| source.operand != operand.operand)
                })
                .collect::<Vec<_>>();
            if early.len() != 1
                || definition.access != RegisterOperandAccess::Def
                || instruction.operands.len() < 2
                || !source_is_valid
                || tied_source.is_some() && unrelated.is_empty()
                || instruction.operands.iter().any(|operand| {
                    operand.operand != definition.operand
                        && (operand.access != RegisterOperandAccess::Use
                            || operand.tied_to.is_some())
                })
            {
                return Err(TerminalLivenessError::UnsupportedEarlyClobber {
                    function: function_index,
                    instruction: instruction.id.0,
                    operand: early.get(1).copied().unwrap_or(definition).operand,
                });
            }
            early_rows.push((
                instruction.id.0,
                definition.operand,
                definition.virtual_register,
                tied_source.map(|source| source.virtual_register),
                unrelated
                    .into_iter()
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
    for (instruction, def_operand, definition, tied_source, unrelated) in early_rows {
        let source_and_definition_form_one_edge = match tied_source {
            None => tied_edges
                .iter()
                .all(|(left, right)| *left != definition && *right != definition),
            Some(source) => {
                let incident = tied_edges
                    .iter()
                    .filter(|(left, right)| {
                        *left == source
                            || *right == source
                            || *left == definition
                            || *right == definition
                    })
                    .collect::<Vec<_>>();
                incident.len() == 1 && *incident[0] == (source, definition)
            }
        };
        let related_unrelated_operand = unrelated.iter().find(|(register, _)| {
            tied_edges
                .iter()
                .any(|(left, right)| left == register || right == register)
        });
        if !source_and_definition_form_one_edge || related_unrelated_operand.is_some() {
            return Err(TerminalLivenessError::UnsupportedEarlyClobber {
                function: function_index,
                instruction,
                operand: related_unrelated_operand.map_or(def_operand, |(_, operand)| *operand),
            });
        }
    }
    Ok(())
}

fn ordered_instructions(block: &TerminalSelectedBlock) -> Vec<&TerminalSelectedInstruction> {
    block
        .instructions
        .iter()
        .chain(std::iter::once(match &block.terminator {
            TerminalSelectedTerminator::ConditionalBranch { instruction, .. }
            | TerminalSelectedTerminator::Return { instruction, .. } => instruction,
        }))
        .collect()
}

fn require_canonical<T: Ord>(
    function: usize,
    instruction: Option<u32>,
    set: &[T],
) -> Result<(), TerminalLivenessError> {
    if set.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(TerminalLivenessError::NonCanonicalSet {
            function,
            instruction,
        });
    }
    Ok(())
}

fn collect<T: Copy + Ord>(set: &BTreeSet<T>) -> Vec<T> {
    set.iter().copied().collect()
}

#[cfg(test)]
mod tests {
    use super::replay_function;

    #[test]
    fn independent_liveness_replay_accepts_exact_distinct_tie() {
        let function = crate::compute::tests::supported_tied_function();
        let computed = crate::compute::compute_function(0, &function).unwrap();
        let replayed = replay_function(0, &function).unwrap();
        assert_eq!(computed, replayed);
        assert_eq!(computed.operand_positions[1].tied_to, Some(0));
    }

    #[test]
    fn independent_liveness_replay_accepts_transitive_tied_component() {
        let function = crate::compute::tests::supported_tied_component_function();
        let computed = crate::compute::compute_function(0, &function).unwrap();
        let replayed = replay_function(0, &function).unwrap();
        assert_eq!(computed, replayed);
        assert_eq!(
            computed
                .operand_positions
                .iter()
                .filter(|operand| operand.tied_to.is_some())
                .count(),
            2
        );
    }

    #[test]
    fn independent_liveness_replay_accepts_exact_early_clobber() {
        let function = crate::compute::tests::supported_early_clobber_function();
        let computed = crate::compute::compute_function(0, &function).unwrap();
        let replayed = replay_function(0, &function).unwrap();
        assert_eq!(computed, replayed);
        assert!(!computed.operand_positions[0].early_clobber);
        assert!(computed.operand_positions[1].early_clobber);
        assert_eq!(computed.blocks[0].instructions[0].virtual_uses.len(), 1);
        assert_eq!(computed.blocks[0].instructions[0].virtual_defs.len(), 1);
    }

    #[test]
    fn independent_liveness_replay_accepts_multiple_early_clobber_rows() {
        let function = crate::compute::tests::supported_multiple_early_clobber_function();
        let computed = crate::compute::compute_function(0, &function).unwrap();
        let replayed = replay_function(0, &function).unwrap();
        assert_eq!(computed, replayed);
        assert_eq!(
            computed
                .operand_positions
                .iter()
                .filter(|operand| operand.early_clobber)
                .count(),
            2
        );
        assert_eq!(computed.blocks[0].instructions[1].virtual_uses.len(), 1);
        assert_eq!(computed.blocks[0].instructions[1].virtual_defs.len(), 1);
    }

    #[test]
    fn independent_liveness_replay_accepts_multiple_isolated_tied_early_clobbers() {
        let function =
            crate::compute::tests::supported_multiple_isolated_tied_early_clobber_function();
        let computed = crate::compute::compute_function(0, &function).unwrap();
        let replayed = replay_function(0, &function).unwrap();
        assert_eq!(computed, replayed);
        assert_eq!(
            computed
                .operand_positions
                .iter()
                .filter(|operand| operand.tied_to.is_some() && operand.early_clobber)
                .count(),
            2
        );
        assert_eq!(computed.blocks[0].instructions[0].virtual_uses.len(), 2);
        assert_eq!(computed.blocks[0].instructions[0].virtual_defs.len(), 1);
    }
}
