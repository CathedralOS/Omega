//! Producer application and dense-identifier reconstruction for one function.
//!
//! The rewrite runs in three phases because every identifier the actions
//! name is the source function's: first each admitted consumer is rewritten
//! in place while the source's dense ids still describe it, then every
//! folded literal instruction leaves its block — shifting boundary
//! settlement ordinals — and finally every victim register leaves the table
//! and the two identifier spaces redensify in one pass. Splitting the
//! operand rebuild from the removals keeps a later action's consumer lookup
//! from chasing ids an earlier removal already shifted.

use register_model::RegisterOperandAccess;
use selected_instructions::{
    SelectedFunction, SelectedInstruction, SelectedInstructionId, SelectedInstructionProvenance,
    SelectedOperand, SelectedTerminator, VirtualRegisterId, VirtualRegisterOrigin,
};

use crate::{LiteralFoldAction, LiteralFoldError};

use super::constraints::AdmittedPairs;

pub(super) fn apply_actions(
    function_index: usize,
    function: &mut SelectedFunction,
    actions: &[LiteralFoldAction],
    rows: &AdmittedPairs<'_>,
) -> Result<(), LiteralFoldError> {
    // Phase A: rewrite each consumer's record in place. Every action names
    // a distinct consumer — nomination declines a second literal feeding a
    // claimed instruction — so the kind each rewrite reads is still the
    // source form.
    let rewritten = actions
        .iter()
        .map(|action| rewritten_consumer(function_index, function, action, rows))
        .collect::<Result<Vec<_>, _>>()?;
    for (action, record) in actions.iter().zip(rewritten) {
        let consumer = function
            .blocks
            .iter_mut()
            .filter(|block| block.id == action.block)
            .flat_map(|block| block.instructions.iter_mut())
            .find(|instruction| instruction.id == action.consumer_instruction)
            .ok_or(LiteralFoldError::DecisionMismatch {
                function: function_index,
            })?;
        *consumer = record;
    }
    // Phase B: remove the folded literal instructions. Boundary settlements
    // name instruction ordinals inside their block, so each block's removed
    // positions are gathered in source order before the settlement indexes
    // shift by the count of removals before them.
    let mut removed_instructions = Vec::with_capacity(actions.len());
    let mut settlement_shifts = Vec::new();
    for block in &mut function.blocks {
        let mut positions = Vec::new();
        for action in actions.iter().filter(|action| action.block == block.id) {
            let position = block
                .instructions
                .iter()
                .position(|instruction| instruction.id == action.literal_instruction)
                .ok_or(LiteralFoldError::DecisionMismatch {
                    function: function_index,
                })?;
            removed_instructions.push(action.literal_instruction);
            positions.push(u32::try_from(position).map_err(|_| LiteralFoldError::WorkOverflow)?);
        }
        if positions.is_empty() {
            continue;
        }
        positions.sort_unstable();
        let retained: Vec<SelectedInstructionId> = actions
            .iter()
            .filter(|action| action.block == block.id)
            .map(|action| action.literal_instruction)
            .collect();
        block
            .instructions
            .retain(|instruction| !retained.contains(&instruction.id));
        settlement_shifts.push((block.id, positions));
    }
    for settlement in &mut function.boundary_settlements {
        let Some((_, positions)) = settlement_shifts
            .iter()
            .find(|(block, _)| *block == settlement.block)
        else {
            continue;
        };
        let shift = u32::try_from(
            positions
                .iter()
                .filter(|position| **position < settlement.instruction_index)
                .count(),
        )
        .map_err(|_| LiteralFoldError::WorkOverflow)?;
        settlement.instruction_index -= shift;
    }
    // Phase C: drop the victim registers and redensify both identifier
    // spaces over the removed sets. A surviving reference to a removed
    // instruction or register — a site the sole-use scan proved absent —
    // is an inconsistency, not a program state.
    let mut removed_registers = Vec::with_capacity(actions.len());
    for action in actions {
        let victim_index = usize::try_from(action.victim.0).map_err(|_| {
            LiteralFoldError::IdentifierUnderflow {
                function: function_index,
            }
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
        removed_registers.push(action.victim);
    }
    removed_registers.sort_unstable();
    removed_instructions.sort_unstable();
    function
        .virtual_registers
        .retain(|register| !removed_registers.contains(&register.id));
    redensify(
        function_index,
        function,
        &removed_instructions,
        &removed_registers,
    )
}

/// The consumer record `action` rewrites to, built from the unmodified
/// source function. Admission runs the same construction so a candidate
/// whose rewritten form cannot be built — a result register that cannot
/// hold the folded constant — declines instead of failing at apply time.
pub(super) fn rewritten_consumer(
    function_index: usize,
    function: &SelectedFunction,
    action: &LiteralFoldAction,
    rows: &AdmittedPairs<'_>,
) -> Result<SelectedInstruction, LiteralFoldError> {
    let block = function
        .blocks
        .iter()
        .find(|block| block.id == action.block)
        .ok_or(LiteralFoldError::DecisionMismatch {
            function: function_index,
        })?;
    let literal = block
        .instructions
        .iter()
        .find(|instruction| instruction.id == action.literal_instruction)
        .ok_or(LiteralFoldError::DecisionMismatch {
            function: function_index,
        })?;
    let literal_provenance = literal.provenance.clone();
    let mut consumer = block
        .instructions
        .iter()
        .find(|instruction| instruction.id == action.consumer_instruction)
        .ok_or(LiteralFoldError::DecisionMismatch {
            function: function_index,
        })?
        .clone();

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
    let mut operations = literal_provenance.operations;
    operations.extend(consumer_provenance.operations);
    let mut fuel = literal_provenance.fuel;
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
    if !pair.rule.unit_effects().admits_consumer(&consumer) {
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
    Ok(consumer)
}

fn redensify(
    function_index: usize,
    function: &mut SelectedFunction,
    removed_instructions: &[SelectedInstructionId],
    removed_registers: &[VirtualRegisterId],
) -> Result<(), LiteralFoldError> {
    for call in &mut function.calls {
        call.instruction =
            lower_instruction(function_index, call.instruction, removed_instructions)?;
    }
    for access in &mut function.memory_accesses {
        access.instruction =
            lower_instruction(function_index, access.instruction, removed_instructions)?;
    }
    for register in &mut function.virtual_registers {
        register.id = lower_register(function_index, register.id, removed_registers)?;
        match &mut register.origin {
            VirtualRegisterOrigin::InstructionResult { instruction, .. }
            | VirtualRegisterOrigin::SpillAddress { instruction, .. }
            | VirtualRegisterOrigin::StructuralObservation { instruction, .. }
            | VirtualRegisterOrigin::ScalarAbiAddress { instruction, .. }
            | VirtualRegisterOrigin::InstructionScratch { instruction, .. }
            | VirtualRegisterOrigin::AbiTransport { instruction, .. } => {
                *instruction =
                    lower_instruction(function_index, *instruction, removed_instructions)?;
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
                removed_instructions,
                removed_registers,
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
                if let selected_instructions::SelectedStructuralTransport::WholeValue {
                    argument,
                    ..
                }
                | selected_instructions::SelectedStructuralTransport::Address {
                    base: selected_instructions::SelectedAddressBase::Register(argument),
                    ..
                } = &mut binding.transport
                {
                    *argument = lower_register(function_index, *argument, removed_registers)?;
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
                                lower_register(function_index, *parameter, removed_registers)?;
                        }
                        selected_instructions::SelectedCasePayloadTransport::Registers {
                            argument,
                            parameter,
                        } => {
                            *argument =
                                lower_register(function_index, *argument, removed_registers)?;
                            *parameter =
                                lower_register(function_index, *parameter, removed_registers)?;
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
                    *argument = lower_register(function_index, *argument, removed_registers)?;
                    *parameter = lower_register(function_index, *parameter, removed_registers)?;
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
                    removed_instructions,
                    removed_registers,
                )?;
            }
        }
    }
    Ok(())
}

fn lower_selected_instruction(
    function_index: usize,
    instruction: &mut SelectedInstruction,
    removed_instructions: &[SelectedInstructionId],
    removed_registers: &[VirtualRegisterId],
) -> Result<(), LiteralFoldError> {
    instruction.id = lower_instruction(function_index, instruction.id, removed_instructions)?;
    for operand in &mut instruction.operands {
        operand.virtual_register =
            lower_register(function_index, operand.virtual_register, removed_registers)?;
    }
    Ok(())
}

/// The dense successor of `id` after the sorted `removed` set leaves: an
/// identifier still naming a removed member is an inconsistency — the fold's
/// custody scans already proved no surviving reference exists.
fn lower_instruction(
    function_index: usize,
    id: SelectedInstructionId,
    removed: &[SelectedInstructionId],
) -> Result<SelectedInstructionId, LiteralFoldError> {
    if removed.contains(&id) {
        return Err(LiteralFoldError::IdentifierUnderflow {
            function: function_index,
        });
    }
    Ok(SelectedInstructionId(
        id.0.checked_sub(
            u32::try_from(removed.partition_point(|removed| removed.0 < id.0)).map_err(|_| {
                LiteralFoldError::IdentifierUnderflow {
                    function: function_index,
                }
            })?,
        )
        .ok_or(LiteralFoldError::IdentifierUnderflow {
            function: function_index,
        })?,
    ))
}

fn lower_register(
    function_index: usize,
    id: VirtualRegisterId,
    removed: &[VirtualRegisterId],
) -> Result<VirtualRegisterId, LiteralFoldError> {
    if removed.contains(&id) {
        return Err(LiteralFoldError::IdentifierUnderflow {
            function: function_index,
        });
    }
    Ok(VirtualRegisterId(
        id.0.checked_sub(
            u32::try_from(removed.partition_point(|removed| removed.0 < id.0)).map_err(|_| {
                LiteralFoldError::IdentifierUnderflow {
                    function: function_index,
                }
            })?,
        )
        .ok_or(LiteralFoldError::IdentifierUnderflow {
            function: function_index,
        })?,
    ))
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
