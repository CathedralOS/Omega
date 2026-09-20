//! Producer application and dense-identifier reconstruction for one function.

use register_model::RegisterOperandAccess;
use selected_instructions::{
    SelectedFunction, SelectedInstruction, SelectedInstructionId, SelectedInstructionProvenance,
    SelectedOperand, SelectedTerminator, VirtualRegisterId, VirtualRegisterOrigin,
};

use crate::{LiteralFoldAction, LiteralFoldError};

use super::constraints::AdmittedPairs;

pub(super) fn apply_action(
    function_index: usize,
    function: &mut SelectedFunction,
    action: LiteralFoldAction,
    rows: &AdmittedPairs<'_>,
) -> Result<(), LiteralFoldError> {
    let block = function
        .blocks
        .iter_mut()
        .find(|block| block.id == action.block)
        .ok_or(LiteralFoldError::DecisionMismatch {
            function: function_index,
        })?;
    let literal_index = block
        .instructions
        .iter()
        .position(|instruction| instruction.id == action.literal_instruction)
        .ok_or(LiteralFoldError::DecisionMismatch {
            function: function_index,
        })?;
    let removed_position =
        u32::try_from(literal_index).map_err(|_| LiteralFoldError::WorkOverflow)?;
    let literal = block.instructions.remove(literal_index);
    for settlement in &mut function.boundary_settlements {
        if settlement.block == action.block && settlement.instruction_index > removed_position {
            settlement.instruction_index -= 1;
        }
    }
    let consumer = block
        .instructions
        .get_mut(literal_index)
        .filter(|instruction| instruction.id == action.consumer_instruction)
        .ok_or(LiteralFoldError::DecisionMismatch {
            function: function_index,
        })?;

    let pair = rows
        .for_consumer_row(consumer.kind, action.immediate_constraint)
        .ok_or(LiteralFoldError::ConsumerMismatch {
            function: function_index,
        })?;
    let result_scalar = action
        .result
        .and_then(|result| {
            function
                .virtual_registers
                .iter()
                .find(|register| register.id == result)
        })
        .map(|register| register.scalar_type);
    let rewritten_kind = pair
        .rule
        .rewrite_consumer(consumer.kind, action.immediate, result_scalar)
        .ok_or(LiteralFoldError::ConsumerMismatch {
            function: function_index,
        })?;
    let row = pair.row;

    let consumer_provenance = consumer.provenance.clone();
    let mut operations = literal.provenance.operations;
    operations.extend(consumer_provenance.operations);
    let mut fuel = literal.provenance.fuel;
    fuel.extend(consumer_provenance.fuel);
    // Bind each rewritten row operand to its recorded register: `Use`
    // positions take the surviving source operand — the left operand under a
    // right-literal grammar, the right operand under a left-literal one —
    // and `Def` positions take the scalar result, so unary constant folds
    // bind only their result.
    let mut registers = Vec::with_capacity(row.operands.len());
    for constraint in &row.operands {
        let register = match constraint.access {
            RegisterOperandAccess::Use => Some(action.surviving),
            RegisterOperandAccess::Def => action.result,
            _ => None,
        };
        registers.push(register.ok_or(LiteralFoldError::ConsumerMismatch {
            function: function_index,
        })?);
    }
    // The operand rebuild below replaces the consumer's operands wholesale;
    // the declared unit-effect surface admits only operands whose unit
    // bindings would not be silently dropped.
    if !pair.rule.unit_effects().admits_consumer(consumer) {
        return Err(LiteralFoldError::ConsumerMismatch {
            function: function_index,
        });
    }
    consumer.kind = rewritten_kind;
    consumer.constraint = action.immediate_constraint;
    consumer.operands = row
        .operands
        .iter()
        .zip(registers.iter())
        .map(|(constraint, register)| selected_operand(constraint, *register))
        .collect();
    consumer.implicit_uses = row.implicit_uses.clone();
    consumer.implicit_defs = row.implicit_defs.clone();
    consumer.clobbers = row.clobbers.clone();
    consumer.provenance = SelectedInstructionProvenance {
        operations,
        values: consumer_provenance.values,
        edges: consumer_provenance.edges,
        obligations: consumer_provenance.obligations,
        fuel,
    };

    let victim_index =
        usize::try_from(action.victim.0).map_err(|_| LiteralFoldError::IdentifierUnderflow {
            function: function_index,
        })?;
    if function
        .virtual_registers
        .get(victim_index)
        .map(|register| register.id)
        != Some(action.victim)
    {
        return Err(LiteralFoldError::DecisionMismatch {
            function: function_index,
        });
    }
    function.virtual_registers.remove(victim_index);
    redensify(
        function_index,
        function,
        action.literal_instruction,
        action.victim,
    )
}

fn redensify(
    function_index: usize,
    function: &mut SelectedFunction,
    removed_instruction: SelectedInstructionId,
    removed_register: VirtualRegisterId,
) -> Result<(), LiteralFoldError> {
    for call in &mut function.calls {
        call.instruction =
            lower_instruction(function_index, call.instruction, removed_instruction)?;
    }
    for access in &mut function.memory_accesses {
        access.instruction =
            lower_instruction(function_index, access.instruction, removed_instruction)?;
    }
    for register in &mut function.virtual_registers {
        register.id = lower_register(function_index, register.id, removed_register)?;
        match &mut register.origin {
            VirtualRegisterOrigin::InstructionResult { instruction, .. }
            | VirtualRegisterOrigin::SpillAddress { instruction, .. }
            | VirtualRegisterOrigin::StructuralObservation { instruction, .. }
            | VirtualRegisterOrigin::ScalarAbiAddress { instruction, .. }
            | VirtualRegisterOrigin::InstructionScratch { instruction, .. }
            | VirtualRegisterOrigin::AbiTransport { instruction, .. } => {
                *instruction =
                    lower_instruction(function_index, *instruction, removed_instruction)?;
            }
            VirtualRegisterOrigin::EntryParameter { .. }
            | VirtualRegisterOrigin::StructuralParameter { .. }
            | VirtualRegisterOrigin::BlockParameter { .. } => {}
        }
    }
    for block in &mut function.blocks {
        for instruction in &mut block.instructions {
            lower_selected_instruction(
                function_index,
                instruction,
                removed_instruction,
                removed_register,
            )?;
        }
        let successors = match &mut block.terminator {
            SelectedTerminator::Jump { successor, .. } => vec![successor],
            SelectedTerminator::ConditionalBranch {
                when_nonzero,
                when_zero,
                ..
            } => vec![when_nonzero, when_zero],
            SelectedTerminator::ConditionalBranchU64LessThan {
                when_less,
                when_not_less,
                ..
            }
            | SelectedTerminator::ConditionalBranchI64LessThan {
                when_less,
                when_not_less,
                ..
            } => vec![when_less, when_not_less],
            SelectedTerminator::Return { .. }
            | SelectedTerminator::Crash { .. }
            | SelectedTerminator::HostedExitProcess { .. } => Vec::new(),
        };
        for successor in successors {
            for binding in &mut successor.structural_bindings {
                if let selected_instructions::SelectedStructuralTransport::Descriptor {
                    argument,
                    ..
                }
                | selected_instructions::SelectedStructuralTransport::WholeValue {
                    argument,
                    ..
                } = &mut binding.transport
                {
                    *argument = lower_register(function_index, *argument, removed_register)?;
                }
            }
            if let Some(case) = &mut successor.structural_case {
                for payload in &mut case.payloads {
                    match &mut payload.transport {
                        selected_instructions::SelectedCasePayloadTransport::Unused => {}
                        selected_instructions::SelectedCasePayloadTransport::Unmaterialized {
                            parameter,
                        } => {
                            *parameter =
                                lower_register(function_index, *parameter, removed_register)?;
                        }
                        selected_instructions::SelectedCasePayloadTransport::Registers {
                            argument,
                            parameter,
                        } => {
                            *argument =
                                lower_register(function_index, *argument, removed_register)?;
                            *parameter =
                                lower_register(function_index, *parameter, removed_register)?;
                        }
                    }
                }
            }
            for binding in &mut successor.bindings {
                if let selected_instructions::SelectedValueTransport::Registers {
                    argument,
                    parameter,
                } = &mut binding.transport
                {
                    *argument = lower_register(function_index, *argument, removed_register)?;
                    *parameter = lower_register(function_index, *parameter, removed_register)?;
                }
            }
        }
        match &mut block.terminator {
            SelectedTerminator::ConditionalBranch { instruction, .. }
            | SelectedTerminator::ConditionalBranchU64LessThan { instruction, .. }
            | SelectedTerminator::ConditionalBranchI64LessThan { instruction, .. }
            | SelectedTerminator::Jump { instruction, .. }
            | SelectedTerminator::Return { instruction, .. }
            | SelectedTerminator::Crash { instruction, .. }
            | SelectedTerminator::HostedExitProcess { instruction, .. } => {
                lower_selected_instruction(
                    function_index,
                    instruction,
                    removed_instruction,
                    removed_register,
                )?;
            }
        }
    }
    Ok(())
}

fn lower_selected_instruction(
    function_index: usize,
    instruction: &mut SelectedInstruction,
    removed_instruction: SelectedInstructionId,
    removed_register: VirtualRegisterId,
) -> Result<(), LiteralFoldError> {
    instruction.id = lower_instruction(function_index, instruction.id, removed_instruction)?;
    for operand in &mut instruction.operands {
        operand.virtual_register =
            lower_register(function_index, operand.virtual_register, removed_register)?;
    }
    Ok(())
}

fn lower_instruction(
    function_index: usize,
    id: SelectedInstructionId,
    removed: SelectedInstructionId,
) -> Result<SelectedInstructionId, LiteralFoldError> {
    if id == removed {
        return Err(LiteralFoldError::IdentifierUnderflow {
            function: function_index,
        });
    }
    Ok(SelectedInstructionId(if id > removed {
        id.0.checked_sub(1)
            .ok_or(LiteralFoldError::IdentifierUnderflow {
                function: function_index,
            })?
    } else {
        id.0
    }))
}

fn lower_register(
    function_index: usize,
    id: VirtualRegisterId,
    removed: VirtualRegisterId,
) -> Result<VirtualRegisterId, LiteralFoldError> {
    if id == removed {
        return Err(LiteralFoldError::IdentifierUnderflow {
            function: function_index,
        });
    }
    Ok(VirtualRegisterId(if id > removed {
        id.0.checked_sub(1)
            .ok_or(LiteralFoldError::IdentifierUnderflow {
                function: function_index,
            })?
    } else {
        id.0
    }))
}

fn selected_operand(
    constraint: &register_model::RegisterOperandConstraint,
    register: VirtualRegisterId,
) -> SelectedOperand {
    SelectedOperand {
        operand: constraint.operand,
        virtual_register: register,
        access: constraint.access,
        class: constraint.class,
        fixed_view: constraint.fixed_view,
        tied_to: constraint.tied_to,
        early_clobber: constraint.early_clobber,
    }
}
