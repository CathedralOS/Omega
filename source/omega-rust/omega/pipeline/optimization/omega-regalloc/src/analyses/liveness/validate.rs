use std::collections::{BTreeMap, BTreeSet};

use crate::analyses::liveness::identity::liveness_identity;
use crate::analyses::liveness::model::{
    BlockLiveness, EntryDefinition, FunctionLiveness, InstructionLiveness, LivenessError,
    LivenessPlan, LivenessPosition, LivenessValidationReceipt, OperandPosition, SuccessorLiveness,
    ValidatedLiveness,
};
use omega_register_model::{RegisterOperandAccess, RegisterUnitId};
use omega_selected_instructions::{
    SelectedBlock, SelectedFunction, SelectedInstruction, SelectedStructuralUnitFunction,
    SelectedTerminator, VirtualRegisterId, VirtualRegisterOrigin,
};

pub fn validate_liveness(
    selected: &impl crate::ValidatedSelectedAnalysis,
    plan: LivenessPlan,
) -> Result<ValidatedLiveness, LivenessError> {
    if plan.selected != selected.selected_identity()
        || plan.optimization_unit != selected.optimization_unit_identity()
        || plan.fuel_schedule != selected.fuel_schedule_identity()
        || plan.target != selected.selected_plan().target
        || plan.functions.len() != selected.selected_plan().functions.len()
        || plan.structural_unit_functions.len()
            != selected.selected_plan().structural_unit_functions.len()
    {
        return Err(LivenessError::RootMismatch);
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
    validate_structural_unit_roster(
        &selected.selected_plan().functions,
        &selected.selected_plan().structural_unit_functions,
        &plan.structural_unit_functions,
    )?;
    let block_count = plan
        .functions
        .iter()
        .chain(&plan.structural_unit_functions)
        .map(|function| function.blocks.len())
        .sum();
    let instruction_count = plan
        .functions
        .iter()
        .chain(&plan.structural_unit_functions)
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
    let receipt = LivenessValidationReceipt {
        identity: liveness_identity(&plan),
        selected: plan.selected,
        optimization_unit: plan.optimization_unit,
        fuel_schedule: plan.fuel_schedule,
        function_count: plan.functions.len(),
        structural_unit_function_count: plan.structural_unit_functions.len(),
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
    Ok(ValidatedLiveness { plan, receipt })
}

fn validate_structural_unit_roster(
    scalar_functions: &[SelectedFunction],
    selected_functions: &[SelectedStructuralUnitFunction],
    actual_functions: &[FunctionLiveness],
) -> Result<(), LivenessError> {
    let selected_machines = selected_functions
        .iter()
        .map(|function| function.machine)
        .collect::<Vec<_>>();
    validate_structural_machine_roster(
        scalar_functions.iter().map(|function| function.machine),
        &selected_machines,
        actual_functions,
    )?;
    let mut selected_by_machine = BTreeMap::new();
    for (ordinal, function) in selected_functions.iter().enumerate() {
        selected_by_machine.insert(function.machine, (ordinal, function));
    }
    for (ordinal, actual) in actual_functions.iter().enumerate() {
        let (selected_ordinal, selected) = selected_by_machine[&actual.machine];
        debug_assert_eq!(selected_ordinal, ordinal);
        let expected = replay_structural_unit_function(ordinal, selected)?;
        validate_function(ordinal, actual, &expected)?;
    }
    Ok(())
}

fn validate_structural_machine_roster(
    scalar_machines: impl IntoIterator<Item = psi_core::MachineId>,
    selected_machines: &[psi_core::MachineId],
    actual_functions: &[FunctionLiveness],
) -> Result<(), LivenessError> {
    if selected_machines.len() != actual_functions.len() {
        return Err(LivenessError::RootMismatch);
    }
    let mut all_selected = BTreeSet::new();
    for machine in scalar_machines
        .into_iter()
        .chain(selected_machines.iter().copied())
    {
        if !all_selected.insert(machine) {
            return Err(LivenessError::DuplicateMachine {
                machine: machine.get(),
            });
        }
    }
    let mut actual_machines = BTreeSet::new();
    for (ordinal, (selected, actual)) in selected_machines.iter().zip(actual_functions).enumerate()
    {
        if !actual_machines.insert(actual.machine) {
            return Err(LivenessError::DuplicateMachine {
                machine: actual.machine.get(),
            });
        }
        if *selected != actual.machine {
            return Err(LivenessError::StructuralFunctionMismatch { function: ordinal });
        }
    }
    Ok(())
}

fn replay_structural_unit_function(
    function_index: usize,
    function: &SelectedStructuralUnitFunction,
) -> Result<FunctionLiveness, LivenessError> {
    let mut selected_rows = Vec::with_capacity(usize::from(function.call.is_some()) + 1);
    if let Some(call) = &function.call {
        selected_rows.push((
            call.id,
            call.implicit_uses.as_slice(),
            call.implicit_defs.as_slice(),
            call.clobbers.as_slice(),
        ));
    }
    let return_instruction = &function.terminator.instruction;
    selected_rows.push((
        return_instruction.id,
        return_instruction.implicit_uses.as_slice(),
        return_instruction.implicit_defs.as_slice(),
        return_instruction.clobbers.as_slice(),
    ));

    let mut live_units = BTreeSet::new();
    let mut instructions = Vec::with_capacity(selected_rows.len());
    for (ordinal, (instruction, uses, defs, clobbers)) in selected_rows.iter().enumerate().rev() {
        let position = LivenessPosition(u32::try_from(ordinal).map_err(|_| {
            LivenessError::NonDensePositions {
                function: function_index,
            }
        })?);
        let live_out = collect(&live_units);
        for unit in defs.iter().chain(clobbers.iter()) {
            live_units.remove(unit);
        }
        live_units.extend(uses.iter().copied());
        instructions.push(InstructionLiveness {
            position,
            instruction: *instruction,
            virtual_uses: Vec::new(),
            virtual_defs: Vec::new(),
            virtual_live_in: Vec::new(),
            virtual_live_out: Vec::new(),
            unit_uses: uses.to_vec(),
            unit_defs: defs.to_vec(),
            unit_clobbers: clobbers.to_vec(),
            unit_live_in: collect(&live_units),
            unit_live_out: live_out,
        });
    }
    instructions.reverse();
    Ok(FunctionLiveness {
        machine: function.machine,
        entry_definitions: Vec::new(),
        operand_positions: Vec::new(),
        blocks: vec![BlockLiveness {
            block: function.entry_block,
            source_block: function.source_entry_block,
            virtual_live_in: Vec::new(),
            virtual_live_out: Vec::new(),
            unit_live_in: collect(&live_units),
            unit_live_out: Vec::new(),
            instructions,
            successors: Vec::new(),
        }],
    })
}

fn validate_function(
    function_index: usize,
    actual: &FunctionLiveness,
    expected: &FunctionLiveness,
) -> Result<(), LivenessError> {
    if actual.machine != expected.machine || actual.blocks.len() != expected.blocks.len() {
        return Err(LivenessError::FunctionMismatch {
            function: function_index,
        });
    }
    if actual.entry_definitions != expected.entry_definitions
        || actual.operand_positions != expected.operand_positions
    {
        return Err(LivenessError::FixedConstraintMismatch {
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
        u32::try_from(positions.len()).map_err(|_| LivenessError::NonDensePositions {
            function: function_index,
        })?;
    if positions != (0..expected_position_count).collect::<Vec<_>>() {
        return Err(LivenessError::NonDensePositions {
            function: function_index,
        });
    }
    for (actual, expected) in actual.blocks.iter().zip(&expected.blocks) {
        if actual.block != expected.block
            || actual.source_block != expected.source_block
            || actual.instructions.len() != expected.instructions.len()
        {
            return Err(LivenessError::BlockMismatch {
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
            return Err(LivenessError::BlockMismatch {
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
                return Err(LivenessError::InstructionMismatch {
                    function: function_index,
                    instruction: expected_instruction.instruction.0,
                });
            }
            if actual_instruction.virtual_live_in != expected_instruction.virtual_live_in
                || actual_instruction.virtual_live_out != expected_instruction.virtual_live_out
                || actual_instruction.unit_live_in != expected_instruction.unit_live_in
                || actual_instruction.unit_live_out != expected_instruction.unit_live_out
            {
                return Err(LivenessError::TransferMismatch {
                    function: function_index,
                    instruction: expected_instruction.instruction.0,
                });
            }
        }
        if actual.successors.len() != expected.successors.len() {
            return Err(LivenessError::SuccessorMismatch {
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
                return Err(LivenessError::SuccessorMismatch {
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
    function: &SelectedFunction,
) -> Result<FunctionLiveness, LivenessError> {
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
                SelectedTerminator::ConditionalBranch {
                    when_nonzero,
                    when_zero,
                    ..
                } => vec![when_nonzero.block, when_zero.block],
                SelectedTerminator::Return { .. } => Vec::new(),
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
            position.insert(instruction.id, LivenessPosition(ordinal));
            ordinal = ordinal
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
        .flat_map(ordered_instructions)
        .flat_map(|instruction| {
            let instruction_position = position[&instruction.id];
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
        .map(|block| replay_block(block, &position, &v_in, &v_out, &u_in, &u_out))
        .collect();
    Ok(FunctionLiveness {
        machine: function.machine,
        entry_definitions,
        operand_positions,
        blocks,
    })
}

fn replay_block(
    block: &SelectedBlock,
    position: &BTreeMap<omega_selected_instructions::SelectedInstructionId, LivenessPosition>,
    v_in: &BTreeMap<omega_selected_instructions::SelectedBlockId, BTreeSet<VirtualRegisterId>>,
    v_out: &BTreeMap<omega_selected_instructions::SelectedBlockId, BTreeSet<VirtualRegisterId>>,
    u_in: &BTreeMap<omega_selected_instructions::SelectedBlockId, BTreeSet<RegisterUnitId>>,
    u_out: &BTreeMap<omega_selected_instructions::SelectedBlockId, BTreeSet<RegisterUnitId>>,
) -> BlockLiveness {
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
        instructions.push(InstructionLiveness {
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
        SelectedTerminator::ConditionalBranch {
            instruction,
            when_nonzero,
            when_zero,
        } => [when_nonzero, when_zero]
            .into_iter()
            .enumerate()
            .map(|(ordinal, successor)| SuccessorLiveness {
                terminator: instruction.id,
                polarity_ordinal: ordinal as u8,
                psi_edge: successor.psi_edge,
                target: successor.block,
                virtual_live: collect(&v_in[&successor.block]),
                unit_live: collect(&u_in[&successor.block]),
            })
            .collect(),
        SelectedTerminator::Return { .. } => Vec::new(),
    };
    BlockLiveness {
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
    function: &SelectedFunction,
) -> Result<(), LivenessError> {
    let mut tied_edges = Vec::new();
    let mut early_rows = Vec::new();
    for instruction in function.blocks.iter().flat_map(ordered_instructions) {
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
        if let Some(definition) = early.first().copied() {
            let mut values = Vec::new();
            for operand in &instruction.operands {
                if values.contains(&operand.virtual_register) {
                    return Err(LivenessError::UnsupportedEarlyClobber {
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
                return Err(LivenessError::UnsupportedEarlyClobber {
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
    let components = independently_merge_tied_components(&tied_edges);
    // Independently replay SingleEarlyDefTiedComponentAgainstUntiedUsesV1.
    for (instruction, def_operand, definition, tied_source, unrelated) in &early_rows {
        let source_and_definition_share_one_early_component = match tied_source {
            None => tied_edges
                .iter()
                .all(|(left, right)| left != definition && right != definition),
            Some(source) => {
                let Some(component) = components
                    .iter()
                    .find(|component| component.contains(source) && component.contains(definition))
                else {
                    return Err(LivenessError::UnsupportedEarlyClobber {
                        function: function_index,
                        instruction: *instruction,
                        operand: *def_operand,
                    });
                };
                tied_edges.contains(&(*source, *definition))
                    && early_rows
                        .iter()
                        .filter(|(_, _, candidate, candidate_source, _)| {
                            candidate_source.is_some() && component.contains(candidate)
                        })
                        .count()
                        == 1
            }
        };
        let related_unrelated_operand = unrelated.iter().find(|(register, _)| {
            tied_edges
                .iter()
                .any(|(left, right)| left == register || right == register)
        });
        if !source_and_definition_share_one_early_component || related_unrelated_operand.is_some() {
            return Err(LivenessError::UnsupportedEarlyClobber {
                function: function_index,
                instruction: *instruction,
                operand: related_unrelated_operand.map_or(*def_operand, |(_, operand)| *operand),
            });
        }
    }
    Ok(())
}

fn independently_merge_tied_components(
    edges: &[(VirtualRegisterId, VirtualRegisterId)],
) -> Vec<BTreeSet<VirtualRegisterId>> {
    let mut components = Vec::<BTreeSet<_>>::new();
    for (left, right) in edges {
        let left_component = components
            .iter()
            .position(|component| component.contains(left));
        let right_component = components
            .iter()
            .position(|component| component.contains(right));
        match (left_component, right_component) {
            (None, None) => components.push(BTreeSet::from([*left, *right])),
            (Some(component), None) => {
                components[component].insert(*right);
            }
            (None, Some(component)) => {
                components[component].insert(*left);
            }
            (Some(left_component), Some(right_component)) if left_component != right_component => {
                let (keep, remove) = if left_component < right_component {
                    (left_component, right_component)
                } else {
                    (right_component, left_component)
                };
                let removed = components.remove(remove);
                components[keep].extend(removed);
            }
            (Some(_), Some(_)) => {}
        }
    }
    components
}

fn ordered_instructions(block: &SelectedBlock) -> Vec<&SelectedInstruction> {
    block
        .instructions
        .iter()
        .chain(std::iter::once(match &block.terminator {
            SelectedTerminator::ConditionalBranch { instruction, .. }
            | SelectedTerminator::Return { instruction, .. } => instruction,
        }))
        .collect()
}

fn require_canonical<T: Ord>(
    function: usize,
    instruction: Option<u32>,
    set: &[T],
) -> Result<(), LivenessError> {
    if set.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(LivenessError::NonCanonicalSet {
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
    use omega_register_model::{RegisterClassId, RegisterOperandAccess, RegisterUnitId};
    use omega_selected_instructions::{
        SelectedBlockId, SelectedInstructionId, SelectedOperand, VirtualRegisterId,
    };
    use psi_core::{BlockId, MachineId};

    use super::{
        reject_v1_unsupported, replay_function, validate_function,
        validate_structural_machine_roster,
    };

    fn structural_liveness(machine: MachineId) -> crate::FunctionLiveness {
        crate::FunctionLiveness {
            machine,
            entry_definitions: Vec::new(),
            operand_positions: Vec::new(),
            blocks: vec![crate::BlockLiveness {
                block: SelectedBlockId(0),
                source_block: BlockId::new(machine.get()).unwrap(),
                virtual_live_in: Vec::new(),
                virtual_live_out: Vec::new(),
                unit_live_in: vec![RegisterUnitId(1)],
                unit_live_out: Vec::new(),
                instructions: vec![crate::InstructionLiveness {
                    position: crate::LivenessPosition(0),
                    instruction: SelectedInstructionId(0),
                    virtual_uses: Vec::new(),
                    virtual_defs: Vec::new(),
                    virtual_live_in: Vec::new(),
                    virtual_live_out: Vec::new(),
                    unit_uses: vec![RegisterUnitId(1)],
                    unit_defs: vec![RegisterUnitId(2)],
                    unit_clobbers: Vec::new(),
                    unit_live_in: vec![RegisterUnitId(1)],
                    unit_live_out: Vec::new(),
                }],
                successors: Vec::new(),
            }],
        }
    }

    #[test]
    fn structural_roster_rejects_erasure_order_identity_duplicate_and_unit_drift() {
        let scalar = MachineId::new(9).unwrap();
        let caller = MachineId::new(1).unwrap();
        let callee = MachineId::new(2).unwrap();
        let selected = [caller, callee];
        let exact = [structural_liveness(caller), structural_liveness(callee)];
        validate_structural_machine_roster([scalar], &selected, &exact).unwrap();

        assert_eq!(
            validate_structural_machine_roster([scalar], &selected, &exact[..1]),
            Err(crate::LivenessError::RootMismatch)
        );
        let swapped = [exact[1].clone(), exact[0].clone()];
        assert_eq!(
            validate_structural_machine_roster([scalar], &selected, &swapped),
            Err(crate::LivenessError::StructuralFunctionMismatch { function: 0 })
        );
        let foreign = MachineId::new(3).unwrap();
        let drifted = [structural_liveness(foreign), exact[1].clone()];
        assert_eq!(
            validate_structural_machine_roster([scalar], &selected, &drifted),
            Err(crate::LivenessError::StructuralFunctionMismatch { function: 0 })
        );
        assert_eq!(
            validate_structural_machine_roster([caller], &selected, &exact),
            Err(crate::LivenessError::DuplicateMachine {
                machine: caller.get()
            })
        );

        let mut unit_drift = exact[0].clone();
        unit_drift.blocks[0].instructions[0].unit_uses[0] = RegisterUnitId(3);
        assert_eq!(
            validate_function(0, &unit_drift, &exact[0]),
            Err(crate::LivenessError::InstructionMismatch {
                function: 0,
                instruction: 0
            })
        );
    }

    #[test]
    fn independent_liveness_replay_accepts_exact_distinct_tie() {
        let function = crate::analyses::liveness::tests::supported_tied_function();
        let computed = crate::analyses::liveness::compute::compute_function(0, &function).unwrap();
        let replayed = replay_function(0, &function).unwrap();
        assert_eq!(computed, replayed);
        assert_eq!(computed.operand_positions[1].tied_to, Some(0));
    }

    #[test]
    fn independent_liveness_replay_accepts_transitive_tied_component() {
        let function = crate::analyses::liveness::tests::supported_tied_component_function();
        let computed = crate::analyses::liveness::compute::compute_function(0, &function).unwrap();
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
        let function = crate::analyses::liveness::tests::supported_early_clobber_function();
        let computed = crate::analyses::liveness::compute::compute_function(0, &function).unwrap();
        let replayed = replay_function(0, &function).unwrap();
        assert_eq!(computed, replayed);
        assert!(!computed.operand_positions[0].early_clobber);
        assert!(computed.operand_positions[1].early_clobber);
        assert_eq!(computed.blocks[0].instructions[0].virtual_uses.len(), 1);
        assert_eq!(computed.blocks[0].instructions[0].virtual_defs.len(), 1);
    }

    #[test]
    fn independent_liveness_replay_accepts_multiple_early_clobber_rows() {
        let function =
            crate::analyses::liveness::tests::supported_multiple_early_clobber_function();
        let computed = crate::analyses::liveness::compute::compute_function(0, &function).unwrap();
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
            crate::analyses::liveness::tests::supported_multiple_isolated_tied_early_clobber_function();
        let computed = crate::analyses::liveness::compute::compute_function(0, &function).unwrap();
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

    #[test]
    fn independent_liveness_replay_accepts_one_early_def_in_a_larger_tied_component() {
        let function =
            crate::analyses::liveness::tests::supported_component_tied_early_clobber_function();
        let computed = crate::analyses::liveness::compute::compute_function(0, &function).unwrap();
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
        assert_eq!(
            computed
                .operand_positions
                .iter()
                .filter(|operand| operand.early_clobber)
                .count(),
            1
        );

        let multiple =
            crate::analyses::liveness::tests::supported_multiple_component_tied_early_clobber_function();
        assert_eq!(
            crate::analyses::liveness::compute::compute_function(0, &multiple).unwrap(),
            replay_function(0, &multiple).unwrap()
        );

        let mut two_early = function;
        two_early.blocks[0].instructions[0].operands[1].early_clobber = true;
        two_early.blocks[0].instructions[0]
            .operands
            .push(SelectedOperand {
                operand: 2,
                virtual_register: VirtualRegisterId(4),
                access: RegisterOperandAccess::Use,
                class: RegisterClassId(0),
                fixed_view: None,
                tied_to: None,
                early_clobber: false,
            });
        assert!(matches!(
            reject_v1_unsupported(0, &two_early),
            Err(crate::LivenessError::UnsupportedEarlyClobber { .. })
        ));
    }
}
