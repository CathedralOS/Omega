use omega_optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};
use omega_register_model::{
    RegisterInstructionConstraint, RegisterOperandAccess, TargetRegisterEnvironmentConstraintKeys,
    TargetRegisterEnvironmentIdentity, ValidatedPhysicalRegisterModel,
    ValidatedRegisterConstraintCatalog, ValidatedRegisterReservationProfile,
    target_register_environment_identity,
};
use omega_selected_instructions::{
    SelectedFunction, SelectedInstruction, SelectedInstructionId, SelectedInstructionKind,
    SelectedInstructionPlan, SelectedInstructionProvenance, SelectedOperand, SelectedTerminator,
    VirtualRegisterId, VirtualRegisterOrigin,
};
use psi_core::IntegerValue;

use crate::{
    FunctionLiteralFold, LiteralFoldAction, LiteralFoldError, LiteralFoldPolicy,
    RecoveryClassification, RecoveryVictimRole, ValidatedAllocationLegality,
    ValidatedAllocatorAvailability, ValidatedLiveRanges, ValidatedRecoveryClassifications,
    ValidatedSelectedAnalysis, ValidatedSpillChoices,
};

#[allow(clippy::too_many_arguments)]
pub(crate) fn validate_literal_fold_roots<S: ValidatedSelectedAnalysis>(
    selected: &S,
    ranges: &ValidatedLiveRanges,
    legality: &ValidatedAllocationLegality,
    spill_choices: &ValidatedSpillChoices,
    recovery: &ValidatedRecoveryClassifications,
    availability: &ValidatedAllocatorAvailability,
    register_environment: TargetRegisterEnvironmentIdentity,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
    reservations: &ValidatedRegisterReservationProfile,
    selected_keys: TargetRegisterEnvironmentConstraintKeys,
) -> Result<(), LiteralFoldError> {
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
        return Err(LiteralFoldError::RootMismatch);
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
    policy: LiteralFoldPolicy,
) -> Result<ImmediateRows<'_>, LiteralFoldError> {
    let find = |key| {
        constraints
            .catalog()
            .constraints
            .iter()
            .find(|row| row.key == key)
            .ok_or(LiteralFoldError::ImmediateConstraintMismatch)
    };
    let (add, subtract) = match policy {
        LiteralFoldPolicy::SelectedIncomingU12ExactAddImmediateV1 => {
            (Some(find(keys.add_i64_immediate)?), None)
        }
        LiteralFoldPolicy::SelectedIncomingU12ExactSubtractImmediateV1 => {
            (None, Some(find(keys.subtract_i64_immediate)?))
        }
        LiteralFoldPolicy::SelectedIncomingU12ExactAddAndSubtractImmediateV1 => (
            Some(find(keys.add_i64_immediate)?),
            Some(find(keys.subtract_i64_immediate)?),
        ),
    };
    for row in [add, subtract].into_iter().flatten() {
        validate_immediate_row(row)?;
    }
    Ok(ImmediateRows { add, subtract })
}

fn validate_immediate_row(row: &RegisterInstructionConstraint) -> Result<(), LiteralFoldError> {
    let [left, result] = row.operands.as_slice() else {
        return Err(LiteralFoldError::ImmediateConstraintMismatch);
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
        return Err(LiteralFoldError::ImmediateConstraintMismatch);
    }
    Ok(())
}

pub(crate) fn fold_usage(
    selected: &impl ValidatedSelectedAnalysis,
    applied: usize,
) -> Result<OptimizationWorkUsage, LiteralFoldError> {
    let functions = u64::try_from(selected.selected_plan().functions.len())
        .map_err(|_| LiteralFoldError::WorkOverflow)?;
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
        .ok_or(LiteralFoldError::WorkOverflow)?;
    let applied = u64::try_from(applied).map_err(|_| LiteralFoldError::WorkOverflow)?;
    Ok(OptimizationWorkUsage {
        rule_evaluations: functions,
        candidates: applied,
        validation_steps,
        commits: applied,
        iterations: 1,
    })
}

pub(crate) fn replay_actions(
    selected: &impl ValidatedSelectedAnalysis,
    recovery: &ValidatedRecoveryClassifications,
    rows: &ImmediateRows<'_>,
) -> Result<(Vec<FunctionLiteralFold>, SelectedInstructionPlan), LiteralFoldError> {
    let mut output = selected.selected_plan().clone();
    let mut functions = Vec::with_capacity(output.functions.len());
    for function_index in 0..output.functions.len() {
        let source = &selected.selected_plan().functions[function_index];
        let recovery_function = recovery.plan().functions.get(function_index).ok_or(
            LiteralFoldError::FunctionMismatch {
                function: function_index,
            },
        )?;
        if source.machine != recovery_function.machine {
            return Err(LiteralFoldError::FunctionMismatch {
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
        functions.push(FunctionLiteralFold {
            machine: source.machine,
            action,
        });
    }
    Ok((functions, output))
}

fn action_from_classification(
    function_index: usize,
    function: &SelectedFunction,
    candidate: &crate::PressureRecoveryClassification,
    rows: &ImmediateRows<'_>,
) -> Result<LiteralFoldAction, LiteralFoldError> {
    if candidate.role != RecoveryVictimRole::Incoming {
        return Err(LiteralFoldError::UnsupportedVictimRole {
            function: function_index,
        });
    }
    let RecoveryClassification::ImmediateU64RematerializationCandidate {
        defining_instruction,
        value: IntegerValue::Unsigned(value),
        provenance,
        future_uses,
        ..
    } = &candidate.classification
    else {
        return Err(LiteralFoldError::ClassificationNotAdmitted {
            function: function_index,
        });
    };
    let immediate = u64::try_from(*value)
        .ok()
        .filter(|value| *value <= 4095)
        .ok_or(LiteralFoldError::UnsupportedImmediate {
            function: function_index,
        })?;
    let [future_use] = future_uses.as_slice() else {
        return Err(LiteralFoldError::FutureUseMismatch {
            function: function_index,
        });
    };
    if future_use.operand != 1 || future_use.block != candidate.block {
        return Err(LiteralFoldError::FutureUseMismatch {
            function: function_index,
        });
    }
    let block = function
        .blocks
        .iter()
        .find(|block| block.id == candidate.block)
        .ok_or(LiteralFoldError::LiteralMismatch {
            function: function_index,
        })?;
    let literal_index = block
        .instructions
        .iter()
        .position(|instruction| instruction.id == *defining_instruction)
        .ok_or(LiteralFoldError::LiteralMismatch {
            function: function_index,
        })?;
    let literal = &block.instructions[literal_index];
    let consumer = block
        .instructions
        .get(literal_index + 1)
        .filter(|instruction| instruction.id == future_use.instruction)
        .ok_or(LiteralFoldError::ConsumerMismatch {
            function: function_index,
        })?;
    if literal.kind
        != (SelectedInstructionKind::MaterializeI64 {
            value: IntegerValue::Unsigned(*value),
        })
        || literal.provenance != *provenance
        || literal.operands.len() != 1
        || literal.operands[0].virtual_register != candidate.victim
        || literal.operands[0].access != RegisterOperandAccess::Def
    {
        return Err(LiteralFoldError::LiteralMismatch {
            function: function_index,
        });
    }
    let row = match consumer.kind {
        SelectedInstructionKind::ExactAddI64 { .. } => rows.add,
        SelectedInstructionKind::ExactSubtractI64 { .. } => rows.subtract,
        _ => None,
    }
    .ok_or(LiteralFoldError::ConsumerMismatch {
        function: function_index,
    })?;
    let [left, right, result] = consumer.operands.as_slice() else {
        return Err(LiteralFoldError::ConsumerMismatch {
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
        return Err(LiteralFoldError::ConsumerMismatch {
            function: function_index,
        });
    }
    Ok(LiteralFoldAction {
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
    function: &SelectedFunction,
) -> Result<(), LiteralFoldError> {
    if function
        .virtual_registers
        .iter()
        .enumerate()
        .any(|(index, register)| usize::try_from(register.id.0) != Ok(index))
    {
        return Err(LiteralFoldError::FunctionMismatch {
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
                    SelectedTerminator::ConditionalBranch { instruction, .. }
                    | SelectedTerminator::Return { instruction, .. } => instruction.id.0,
                }))
        })
        .collect::<Vec<_>>();
    ids.sort_unstable();
    let count = u32::try_from(ids.len()).map_err(|_| LiteralFoldError::WorkOverflow)?;
    if ids != (0..count).collect::<Vec<_>>() {
        return Err(LiteralFoldError::FunctionMismatch {
            function: function_index,
        });
    }
    Ok(())
}

fn apply_action(
    function_index: usize,
    function: &mut SelectedFunction,
    action: LiteralFoldAction,
    rows: &ImmediateRows<'_>,
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
    let literal = block.instructions.remove(literal_index);
    let consumer = block
        .instructions
        .get_mut(literal_index)
        .filter(|instruction| instruction.id == action.consumer_instruction)
        .ok_or(LiteralFoldError::DecisionMismatch {
            function: function_index,
        })?;
    let (row, kind) = match consumer.kind {
        SelectedInstructionKind::ExactAddI64 {
            obligation,
            accepted_fact,
        } => (
            rows.add,
            SelectedInstructionKind::ExactAddI64Immediate {
                immediate: IntegerValue::Unsigned(u128::from(action.immediate)),
                obligation,
                accepted_fact,
            },
        ),
        SelectedInstructionKind::ExactSubtractI64 {
            obligation,
            accepted_fact,
        } => (
            rows.subtract,
            SelectedInstructionKind::ExactSubtractI64Immediate {
                immediate: IntegerValue::Unsigned(u128::from(action.immediate)),
                obligation,
                accepted_fact,
            },
        ),
        _ => (None, consumer.kind),
    };
    let row = row
        .filter(|row| row.key == action.immediate_constraint)
        .ok_or(LiteralFoldError::ConsumerMismatch {
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
    for register in &mut function.virtual_registers {
        register.id = lower_register(function_index, register.id, removed_register)?;
        match &mut register.origin {
            VirtualRegisterOrigin::InstructionResult { instruction, .. }
            | VirtualRegisterOrigin::LegalizationTemporary { instruction, .. } => {
                *instruction =
                    lower_instruction(function_index, *instruction, removed_instruction)?;
            }
            VirtualRegisterOrigin::EntryParameter { .. } => {}
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
            SelectedTerminator::ConditionalBranch { instruction, .. }
            | SelectedTerminator::Return { instruction, .. } => {
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
    constraint: &omega_register_model::RegisterOperandConstraint,
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

pub(crate) fn ensure_budget(
    usage: OptimizationWorkUsage,
    budget: OptimizationWorkBudget,
) -> Result<(), LiteralFoldError> {
    if usage.within(budget) {
        Ok(())
    } else {
        Err(LiteralFoldError::BudgetExceeded {
            required: usage,
            budget,
        })
    }
}
