use omega_optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};
use omega_register_model::{
    RegisterInstructionConstraint, RegisterOperandAccess, TargetRegisterEnvironmentConstraintKeys,
    TargetRegisterEnvironmentIdentity, ValidatedPhysicalRegisterModel,
    ValidatedRegisterConstraintCatalog, ValidatedRegisterReservationProfile,
    target_register_environment_identity,
};
use omega_terminal_selected_instructions::{
    TerminalSelectedFunction, TerminalSelectedInstruction, TerminalSelectedInstructionId,
    TerminalSelectedInstructionKind, TerminalSelectedInstructionPlan,
    TerminalSelectedInstructionProvenance, TerminalSelectedOperand, TerminalSelectedTerminator,
    TerminalVirtualRegisterId, TerminalVirtualRegisterOrigin,
};
use psi_core::IntegerValue;

use crate::{
    TerminalFunctionLiteralFold, TerminalLiteralFoldAction, TerminalLiteralFoldError,
    TerminalLiteralFoldPolicy, TerminalRecoveryClassification, TerminalRecoveryVictimRole,
    ValidatedTerminalAllocationLegality, ValidatedTerminalAllocatorAvailability,
    ValidatedTerminalLiveRanges, ValidatedTerminalRecoveryClassifications,
    ValidatedTerminalSelectedAnalysis, ValidatedTerminalSpillChoices,
};

#[allow(clippy::too_many_arguments)]
pub(crate) fn validate_literal_fold_roots<S: ValidatedTerminalSelectedAnalysis>(
    selected: &S,
    ranges: &ValidatedTerminalLiveRanges,
    legality: &ValidatedTerminalAllocationLegality,
    spill_choices: &ValidatedTerminalSpillChoices,
    recovery: &ValidatedTerminalRecoveryClassifications,
    availability: &ValidatedTerminalAllocatorAvailability,
    register_environment: TargetRegisterEnvironmentIdentity,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
    reservations: &ValidatedRegisterReservationProfile,
    selected_keys: TargetRegisterEnvironmentConstraintKeys,
) -> Result<(), TerminalLiteralFoldError> {
    if ranges.receipt().selected() != selected.selected_identity()
        || ranges.receipt().optimization_unit() != selected.optimization_unit_identity()
        || ranges.receipt().fuel_schedule() != selected.fuel_schedule_identity()
        || legality.receipt().ranges() != ranges.receipt().identity()
        || legality.receipt().register_environment() != register_environment
        || legality.receipt().allocator_availability() != availability.receipt().identity()
        || availability.receipt().register_environment() != register_environment
        || availability.receipt().physical() != physical.identity()
        || constraints.physical_identity() != physical.identity()
        || reservations.physical_identity() != physical.identity()
        || reservations.target() != selected.selected_plan().target
        || target_register_environment_identity(
            selected.selected_plan().target,
            physical,
            constraints,
            reservations,
            selected_keys,
        ) != register_environment
        || spill_choices.receipt().ranges() != ranges.receipt().identity()
        || spill_choices.receipt().legality() != legality.receipt().identity()
        || spill_choices.receipt().register_environment() != register_environment
        || spill_choices.receipt().allocator_availability() != availability.receipt().identity()
        || recovery.receipt().selected() != selected.selected_identity()
        || recovery.receipt().ranges() != ranges.receipt().identity()
        || recovery.receipt().legality() != legality.receipt().identity()
        || recovery.receipt().spill_choices() != spill_choices.receipt().identity()
        || recovery.receipt().register_environment() != register_environment
        || recovery.receipt().allocator_availability() != availability.receipt().identity()
        || recovery.receipt().optimization_unit() != selected.optimization_unit_identity()
        || recovery.receipt().fuel_schedule() != selected.fuel_schedule_identity()
        || selected.selected_plan().functions.len() != recovery.plan().functions.len()
    {
        return Err(TerminalLiteralFoldError::RootMismatch);
    }
    Ok(())
}

pub(crate) struct ImmediateRows<'a> {
    add: Option<&'a RegisterInstructionConstraint>,
    subtract: Option<&'a RegisterInstructionConstraint>,
}

pub(crate) fn immediate_rows(
    constraints: &ValidatedRegisterConstraintCatalog,
    keys: TargetRegisterEnvironmentConstraintKeys,
    policy: TerminalLiteralFoldPolicy,
) -> Result<ImmediateRows<'_>, TerminalLiteralFoldError> {
    let find = |key| {
        constraints
            .catalog()
            .constraints
            .iter()
            .find(|row| row.key == key)
            .ok_or(TerminalLiteralFoldError::ImmediateConstraintMismatch)
    };
    let (add, subtract) = match policy {
        TerminalLiteralFoldPolicy::SelectedIncomingU12ExactAddImmediateV1 => {
            (Some(find(keys.add_i64_immediate)?), None)
        }
        TerminalLiteralFoldPolicy::SelectedIncomingU12ExactSubtractImmediateV1 => {
            (None, Some(find(keys.subtract_i64_immediate)?))
        }
        TerminalLiteralFoldPolicy::SelectedIncomingU12ExactAddAndSubtractImmediateV1 => (
            Some(find(keys.add_i64_immediate)?),
            Some(find(keys.subtract_i64_immediate)?),
        ),
    };
    for row in [add, subtract].into_iter().flatten() {
        validate_immediate_row(row)?;
    }
    Ok(ImmediateRows { add, subtract })
}

fn validate_immediate_row(
    row: &RegisterInstructionConstraint,
) -> Result<(), TerminalLiteralFoldError> {
    let [left, result] = row.operands.as_slice() else {
        return Err(TerminalLiteralFoldError::ImmediateConstraintMismatch);
    };
    if left.operand != 0
        || left.access != RegisterOperandAccess::Use
        || result.operand != 1
        || result.access != RegisterOperandAccess::Def
        || left.class != result.class
        || [left, result].iter().any(|operand| {
            operand.fixed_view.is_some() || operand.tied_to.is_some() || operand.early_clobber
        })
        || !row.implicit_uses.is_empty()
        || !row.implicit_defs.is_empty()
        || !row.clobbers.is_empty()
    {
        return Err(TerminalLiteralFoldError::ImmediateConstraintMismatch);
    }
    Ok(())
}

pub(crate) fn fold_usage(
    selected: &impl ValidatedTerminalSelectedAnalysis,
    applied: usize,
) -> Result<OptimizationWorkUsage, TerminalLiteralFoldError> {
    let functions = u64::try_from(selected.selected_plan().functions.len())
        .map_err(|_| TerminalLiteralFoldError::WorkOverflow)?;
    let validation_steps = selected
        .selected_plan()
        .functions
        .iter()
        .try_fold(0_u64, |total, function| {
            let instructions = function.blocks.iter().try_fold(0_u64, |count, block| {
                count.checked_add(
                    u64::try_from(block.instructions.len())
                        .ok()?
                        .checked_add(1)?,
                )
            })?;
            total
                .checked_add(u64::try_from(function.virtual_registers.len()).ok()?)?
                .checked_add(instructions)
        })
        .ok_or(TerminalLiteralFoldError::WorkOverflow)?;
    let applied = u64::try_from(applied).map_err(|_| TerminalLiteralFoldError::WorkOverflow)?;
    Ok(OptimizationWorkUsage {
        rule_evaluations: functions,
        candidates: applied,
        validation_steps,
        commits: applied,
        iterations: 1,
    })
}

pub(crate) fn replay_actions(
    selected: &impl ValidatedTerminalSelectedAnalysis,
    recovery: &ValidatedTerminalRecoveryClassifications,
    rows: &ImmediateRows<'_>,
) -> Result<
    (
        Vec<TerminalFunctionLiteralFold>,
        TerminalSelectedInstructionPlan,
    ),
    TerminalLiteralFoldError,
> {
    let mut output = selected.selected_plan().clone();
    let mut functions = Vec::with_capacity(output.functions.len());
    for function_index in 0..output.functions.len() {
        let source = &selected.selected_plan().functions[function_index];
        let recovery_function = recovery.plan().functions.get(function_index).ok_or(
            TerminalLiteralFoldError::FunctionMismatch {
                function: function_index,
            },
        )?;
        if source.machine != recovery_function.machine {
            return Err(TerminalLiteralFoldError::FunctionMismatch {
                function: function_index,
            });
        }
        validate_dense(function_index, source)?;
        let action = match &recovery_function.classification {
            None => None,
            Some(classification) => Some(action_from_classification(
                function_index,
                source,
                classification,
                rows,
            )?),
        };
        if let Some(action) = action {
            apply_action(
                function_index,
                &mut output.functions[function_index],
                action,
                rows,
            )?;
        }
        functions.push(TerminalFunctionLiteralFold {
            machine: source.machine,
            action,
        });
    }
    Ok((functions, output))
}

fn action_from_classification(
    function_index: usize,
    function: &TerminalSelectedFunction,
    candidate: &crate::TerminalPressureRecoveryClassification,
    rows: &ImmediateRows<'_>,
) -> Result<TerminalLiteralFoldAction, TerminalLiteralFoldError> {
    if candidate.role != TerminalRecoveryVictimRole::Incoming {
        return Err(TerminalLiteralFoldError::UnsupportedVictimRole {
            function: function_index,
        });
    }
    let TerminalRecoveryClassification::ImmediateU64RematerializationCandidate {
        defining_instruction,
        value: IntegerValue::Unsigned(value),
        provenance,
        future_uses,
        ..
    } = &candidate.classification
    else {
        return Err(TerminalLiteralFoldError::ClassificationNotAdmitted {
            function: function_index,
        });
    };
    let immediate = u64::try_from(*value)
        .ok()
        .filter(|value| *value <= 4095)
        .ok_or(TerminalLiteralFoldError::UnsupportedImmediate {
            function: function_index,
        })?;
    let [future_use] = future_uses.as_slice() else {
        return Err(TerminalLiteralFoldError::FutureUseMismatch {
            function: function_index,
        });
    };
    if future_use.operand != 1 || future_use.block != candidate.block {
        return Err(TerminalLiteralFoldError::FutureUseMismatch {
            function: function_index,
        });
    }
    let block = function
        .blocks
        .iter()
        .find(|block| block.id == candidate.block)
        .ok_or(TerminalLiteralFoldError::LiteralMismatch {
            function: function_index,
        })?;
    let literal_index = block
        .instructions
        .iter()
        .position(|instruction| instruction.id == *defining_instruction)
        .ok_or(TerminalLiteralFoldError::LiteralMismatch {
            function: function_index,
        })?;
    let literal = &block.instructions[literal_index];
    let consumer = block
        .instructions
        .get(literal_index + 1)
        .filter(|instruction| instruction.id == future_use.instruction)
        .ok_or(TerminalLiteralFoldError::ConsumerMismatch {
            function: function_index,
        })?;
    if literal.kind
        != (TerminalSelectedInstructionKind::MaterializeI64 {
            value: IntegerValue::Unsigned(*value),
        })
        || literal.provenance != *provenance
        || literal.operands.len() != 1
        || literal.operands[0].virtual_register != candidate.victim
        || literal.operands[0].access != RegisterOperandAccess::Def
    {
        return Err(TerminalLiteralFoldError::LiteralMismatch {
            function: function_index,
        });
    }
    let row = match consumer.kind {
        TerminalSelectedInstructionKind::ExactAddI64 { .. } => rows.add,
        TerminalSelectedInstructionKind::ExactSubtractI64 { .. } => rows.subtract,
        _ => None,
    }
    .ok_or(TerminalLiteralFoldError::ConsumerMismatch {
        function: function_index,
    })?;
    let [left, right, result] = consumer.operands.as_slice() else {
        return Err(TerminalLiteralFoldError::ConsumerMismatch {
            function: function_index,
        });
    };
    if left.access != RegisterOperandAccess::Use
        || right.access != RegisterOperandAccess::Use
        || right.virtual_register != candidate.victim
        || result.access != RegisterOperandAccess::Def
        || left.class != row.operands[0].class
        || result.class != row.operands[1].class
    {
        return Err(TerminalLiteralFoldError::ConsumerMismatch {
            function: function_index,
        });
    }
    Ok(TerminalLiteralFoldAction {
        block: candidate.block,
        pressure_point: candidate.point,
        literal_instruction: *defining_instruction,
        victim: candidate.victim,
        consumer_instruction: consumer.id,
        left: left.virtual_register,
        result: result.virtual_register,
        immediate,
        immediate_constraint: row.key,
    })
}

fn validate_dense(
    function_index: usize,
    function: &TerminalSelectedFunction,
) -> Result<(), TerminalLiteralFoldError> {
    if function
        .virtual_registers
        .iter()
        .enumerate()
        .any(|(index, register)| usize::try_from(register.id.0) != Ok(index))
    {
        return Err(TerminalLiteralFoldError::FunctionMismatch {
            function: function_index,
        });
    }
    let mut ids = function
        .blocks
        .iter()
        .flat_map(|block| {
            block
                .instructions
                .iter()
                .map(|instruction| instruction.id.0)
                .chain(std::iter::once(match &block.terminator {
                    TerminalSelectedTerminator::ConditionalBranch { instruction, .. }
                    | TerminalSelectedTerminator::Return { instruction, .. } => instruction.id.0,
                }))
        })
        .collect::<Vec<_>>();
    ids.sort_unstable();
    let count = u32::try_from(ids.len()).map_err(|_| TerminalLiteralFoldError::WorkOverflow)?;
    if ids != (0..count).collect::<Vec<_>>() {
        return Err(TerminalLiteralFoldError::FunctionMismatch {
            function: function_index,
        });
    }
    Ok(())
}

fn apply_action(
    function_index: usize,
    function: &mut TerminalSelectedFunction,
    action: TerminalLiteralFoldAction,
    rows: &ImmediateRows<'_>,
) -> Result<(), TerminalLiteralFoldError> {
    let block = function
        .blocks
        .iter_mut()
        .find(|block| block.id == action.block)
        .ok_or(TerminalLiteralFoldError::DecisionMismatch {
            function: function_index,
        })?;
    let literal_index = block
        .instructions
        .iter()
        .position(|instruction| instruction.id == action.literal_instruction)
        .ok_or(TerminalLiteralFoldError::DecisionMismatch {
            function: function_index,
        })?;
    let literal = block.instructions.remove(literal_index);
    let consumer = block
        .instructions
        .get_mut(literal_index)
        .filter(|instruction| instruction.id == action.consumer_instruction)
        .ok_or(TerminalLiteralFoldError::DecisionMismatch {
            function: function_index,
        })?;
    let (row, kind) = match consumer.kind {
        TerminalSelectedInstructionKind::ExactAddI64 {
            obligation,
            accepted_fact,
        } => (
            rows.add,
            TerminalSelectedInstructionKind::ExactAddI64Immediate {
                immediate: IntegerValue::Unsigned(u128::from(action.immediate)),
                obligation,
                accepted_fact,
            },
        ),
        TerminalSelectedInstructionKind::ExactSubtractI64 {
            obligation,
            accepted_fact,
        } => (
            rows.subtract,
            TerminalSelectedInstructionKind::ExactSubtractI64Immediate {
                immediate: IntegerValue::Unsigned(u128::from(action.immediate)),
                obligation,
                accepted_fact,
            },
        ),
        _ => (None, consumer.kind),
    };
    let row = row
        .filter(|row| row.key == action.immediate_constraint)
        .ok_or(TerminalLiteralFoldError::ConsumerMismatch {
            function: function_index,
        })?;
    let consumer_provenance = consumer.provenance.clone();
    let mut operations = literal.provenance.operations;
    operations.extend(consumer_provenance.operations);
    let mut fuel = literal.provenance.fuel;
    fuel.extend(consumer_provenance.fuel);
    consumer.kind = kind;
    consumer.constraint = action.immediate_constraint;
    consumer.operands = vec![
        selected_operand(&row.operands[0], action.left),
        selected_operand(&row.operands[1], action.result),
    ];
    consumer.implicit_uses = row.implicit_uses.clone();
    consumer.implicit_defs = row.implicit_defs.clone();
    consumer.clobbers = row.clobbers.clone();
    consumer.provenance = TerminalSelectedInstructionProvenance {
        operations,
        values: consumer_provenance.values,
        edges: consumer_provenance.edges,
        obligations: consumer_provenance.obligations,
        fuel,
    };

    let victim_index = usize::try_from(action.victim.0).map_err(|_| {
        TerminalLiteralFoldError::IdentifierUnderflow {
            function: function_index,
        }
    })?;
    if function
        .virtual_registers
        .get(victim_index)
        .map(|register| register.id)
        != Some(action.victim)
    {
        return Err(TerminalLiteralFoldError::DecisionMismatch {
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
    function: &mut TerminalSelectedFunction,
    removed_instruction: TerminalSelectedInstructionId,
    removed_register: TerminalVirtualRegisterId,
) -> Result<(), TerminalLiteralFoldError> {
    for register in &mut function.virtual_registers {
        register.id = lower_register(function_index, register.id, removed_register)?;
        match &mut register.origin {
            TerminalVirtualRegisterOrigin::InstructionResult { instruction, .. }
            | TerminalVirtualRegisterOrigin::LegalizationTemporary { instruction, .. } => {
                *instruction =
                    lower_instruction(function_index, *instruction, removed_instruction)?;
            }
            TerminalVirtualRegisterOrigin::EntryParameter { .. } => {}
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
        match &mut block.terminator {
            TerminalSelectedTerminator::ConditionalBranch { instruction, .. }
            | TerminalSelectedTerminator::Return { instruction, .. } => {
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
    instruction: &mut TerminalSelectedInstruction,
    removed_instruction: TerminalSelectedInstructionId,
    removed_register: TerminalVirtualRegisterId,
) -> Result<(), TerminalLiteralFoldError> {
    instruction.id = lower_instruction(function_index, instruction.id, removed_instruction)?;
    for operand in &mut instruction.operands {
        operand.virtual_register =
            lower_register(function_index, operand.virtual_register, removed_register)?;
    }
    Ok(())
}

fn lower_instruction(
    function_index: usize,
    id: TerminalSelectedInstructionId,
    removed: TerminalSelectedInstructionId,
) -> Result<TerminalSelectedInstructionId, TerminalLiteralFoldError> {
    if id == removed {
        return Err(TerminalLiteralFoldError::IdentifierUnderflow {
            function: function_index,
        });
    }
    Ok(TerminalSelectedInstructionId(if id > removed {
        id.0.checked_sub(1)
            .ok_or(TerminalLiteralFoldError::IdentifierUnderflow {
                function: function_index,
            })?
    } else {
        id.0
    }))
}

fn lower_register(
    function_index: usize,
    id: TerminalVirtualRegisterId,
    removed: TerminalVirtualRegisterId,
) -> Result<TerminalVirtualRegisterId, TerminalLiteralFoldError> {
    if id == removed {
        return Err(TerminalLiteralFoldError::IdentifierUnderflow {
            function: function_index,
        });
    }
    Ok(TerminalVirtualRegisterId(if id > removed {
        id.0.checked_sub(1)
            .ok_or(TerminalLiteralFoldError::IdentifierUnderflow {
                function: function_index,
            })?
    } else {
        id.0
    }))
}

fn selected_operand(
    constraint: &omega_register_model::RegisterOperandConstraint,
    register: TerminalVirtualRegisterId,
) -> TerminalSelectedOperand {
    TerminalSelectedOperand {
        operand: constraint.operand,
        virtual_register: register,
        access: constraint.access,
        class: constraint.class,
        fixed_view: constraint.fixed_view,
        tied_to: constraint.tied_to,
        early_clobber: constraint.early_clobber,
    }
}

pub(crate) fn ensure_budget(
    usage: OptimizationWorkUsage,
    budget: OptimizationWorkBudget,
) -> Result<(), TerminalLiteralFoldError> {
    if usage.within(budget) {
        Ok(())
    } else {
        Err(TerminalLiteralFoldError::BudgetExceeded {
            required: usage,
            budget,
        })
    }
}
