use omega_optimization_core::OptimizationWorkUsage;
use omega_register_model::{
    RegisterInstructionConstraint, RegisterOperandAccess, TargetRegisterEnvironmentConstraintKeys,
    TargetRegisterEnvironmentIdentity, ValidatedPhysicalRegisterModel,
    ValidatedRegisterConstraintCatalog, ValidatedRegisterReservationProfile,
    target_register_environment_identity,
};
use omega_terminal_selected_instructions::{
    TerminalSelectedFunction, TerminalSelectedInstruction, TerminalSelectedInstructionId,
    TerminalSelectedInstructionKind, TerminalSelectedInstructionProvenance,
    TerminalSelectedOperand, TerminalSelectedTerminator, TerminalVirtualRegister,
    TerminalVirtualRegisterId, TerminalVirtualRegisterOrigin,
};
use omega_terminal_target_operations_to_selected_instructions::terminal_selected_instruction_plan_identity;

use crate::*;

/// Independently authenticates and replays the plain rematerialization recipe.
/// It does not call the proposal builder or accept a decoded artifact as proof.
#[allow(clippy::too_many_arguments)]
pub fn validate_terminal_pressure_rematerialization<S: ValidatedTerminalSelectedAnalysis>(
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
    plan: TerminalPressureRematerializationPlan,
) -> Result<ValidatedTerminalPressureRematerialization, TerminalPressureRematerializationError> {
    if plan.policy != TerminalPressureRematerializationPolicy::SelectedActiveResidentImmediateU64BeforeSingleFutureFlexibleUseV1 {
        return Err(TerminalPressureRematerializationError::UnsupportedPolicy);
    }
    if plan.source_selected != selected.selected_identity()
        || plan.spill_choices != spill_choices.receipt().identity()
        || plan.recovery_classifications != recovery.receipt().identity()
        || plan.ranges != ranges.receipt().identity()
        || plan.legality != legality.receipt().identity()
        || plan.register_environment != register_environment
        || plan.allocator_availability != availability.receipt().identity()
        || plan.optimization_unit != selected.optimization_unit_identity()
        || plan.fuel_schedule != selected.fuel_schedule_identity()
        || ranges.receipt().selected() != selected.selected_identity()
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
        || plan.functions.len() != selected.selected_plan().functions.len()
        || ranges.plan().functions.len() != plan.functions.len()
        || recovery.plan().functions.len() != plan.functions.len()
    {
        return Err(TerminalPressureRematerializationError::RootMismatch);
    }

    let row = constraints
        .catalog()
        .constraints
        .iter()
        .find(|row| row.key == selected_keys.materialize_i64)
        .ok_or(TerminalPressureRematerializationError::MaterializeConstraintMismatch)?;
    validate_materialize_row(row)?;
    let mut transformed = selected.selected_plan().clone();
    let mut applied = 0usize;
    for index in 0..plan.functions.len() {
        let source = &selected.selected_plan().functions[index];
        let function_plan = &plan.functions[index];
        let range_function = &ranges.plan().functions[index];
        let recovery_function = &recovery.plan().functions[index];
        if source.machine != function_plan.machine
            || source.machine != range_function.machine
            || source.machine != recovery_function.machine
        {
            return Err(TerminalPressureRematerializationError::FunctionMismatch {
                function: index,
            });
        }
        validate_dense(index, source)?;
        match (&recovery_function.classification, function_plan.action) {
            (None, None) => {}
            (Some(candidate), Some(action)) => {
                validate_action(index, source, range_function, candidate, action, row)?;
                replay_action(index, &mut transformed.functions[index], action, row)?;
                applied = applied
                    .checked_add(1)
                    .ok_or(TerminalPressureRematerializationError::WorkOverflow)?;
            }
            _ => {
                return Err(TerminalPressureRematerializationError::DecisionMismatch {
                    function: index,
                });
            }
        }
    }
    if applied == 0 {
        return Err(TerminalPressureRematerializationError::NoAction);
    }
    let usage = independent_usage(selected, applied)?;
    if plan.usage != usage {
        return Err(TerminalPressureRematerializationError::UsageMismatch);
    }
    if !plan.usage.within(plan.budget) {
        return Err(TerminalPressureRematerializationError::BudgetExceeded {
            required: plan.usage,
            budget: plan.budget,
        });
    }
    let transformed_selected = terminal_selected_instruction_plan_identity(&transformed);
    if plan.transformed_selected != transformed_selected {
        return Err(TerminalPressureRematerializationError::TransformedIdentityMismatch);
    }
    let receipt = TerminalPressureRematerializationValidationReceipt {
        identity: terminal_pressure_rematerialization_identity(&plan),
        source_selected: plan.source_selected,
        spill_choices: plan.spill_choices,
        recovery_classifications: plan.recovery_classifications,
        ranges: plan.ranges,
        legality: plan.legality,
        register_environment: plan.register_environment,
        allocator_availability: plan.allocator_availability,
        optimization_unit: plan.optimization_unit,
        fuel_schedule: plan.fuel_schedule,
        transformed_selected,
        policy: plan.policy,
        usage: plan.usage,
        function_count: transformed.functions.len(),
        applied_count: applied,
    };
    Ok(ValidatedTerminalPressureRematerialization {
        plan,
        transformed,
        receipt,
    })
}

fn validate_materialize_row(
    row: &RegisterInstructionConstraint,
) -> Result<(), TerminalPressureRematerializationError> {
    let [result] = row.operands.as_slice() else {
        return Err(TerminalPressureRematerializationError::MaterializeConstraintMismatch);
    };
    if result.operand != 0
        || result.access != RegisterOperandAccess::Def
        || result.fixed_view.is_some()
        || result.tied_to.is_some()
        || result.early_clobber
        || !row.implicit_uses.is_empty()
        || !row.implicit_defs.is_empty()
        || !row.clobbers.is_empty()
    {
        return Err(TerminalPressureRematerializationError::MaterializeConstraintMismatch);
    }
    Ok(())
}

fn validate_action(
    index: usize,
    function: &TerminalSelectedFunction,
    ranges: &TerminalFunctionLiveRanges,
    candidate: &TerminalPressureRecoveryClassification,
    action: TerminalPressureRematerializationAction,
    row: &RegisterInstructionConstraint,
) -> Result<(), TerminalPressureRematerializationError> {
    let TerminalRecoveryVictimRole::ActiveResident {
        current_view,
        reclaimed_view,
    } = candidate.role
    else {
        return Err(
            TerminalPressureRematerializationError::UnsupportedVictimRole { function: index },
        );
    };
    let TerminalRecoveryClassification::ImmediateU64RematerializationCandidate {
        defining_instruction,
        source_value,
        value,
        provenance,
        future_uses,
    } = &candidate.classification
    else {
        return Err(
            TerminalPressureRematerializationError::ClassificationNotAdmitted { function: index },
        );
    };
    let [future] = future_uses.as_slice() else {
        return Err(TerminalPressureRematerializationError::FutureUseMismatch { function: index });
    };
    let expected_instruction = TerminalSelectedInstructionId(instruction_count(index, function)?);
    let expected_register =
        TerminalVirtualRegisterId(u32::try_from(function.virtual_registers.len()).map_err(
            |_| TerminalPressureRematerializationError::IdentifierOverflow { function: index },
        )?);
    if action.block != candidate.block
        || action.pressure_point != candidate.point
        || action.victim != candidate.victim
        || action.current_view != current_view
        || action.reclaimed_view != reclaimed_view
        || action.original_materialize != *defining_instruction
        || action.source_value != *source_value
        || action.value != *value
        || action.future_point != future.point
        || action.future_instruction != future.instruction
        || action.future_operand != future.operand
        || action.fresh_materialize != expected_instruction
        || action.result_virtual_register != expected_register
        || action.materialize_constraint != row.key
        || future.block != candidate.block
        || future.point <= candidate.point
    {
        return Err(TerminalPressureRematerializationError::DecisionMismatch { function: index });
    }
    let victim = function
        .virtual_registers
        .iter()
        .find(|register| register.id == candidate.victim)
        .ok_or(TerminalPressureRematerializationError::MaterializeMismatch { function: index })?;
    if victim.scalar_type != candidate.scalar_type
        || victim.class != candidate.class
        || victim.origin != candidate.origin
        || victim.definition_site != candidate.definition_site
        || victim.entry_fixed_view.is_some()
        || row.operands[0].class != victim.class
    {
        return Err(
            TerminalPressureRematerializationError::MaterializeMismatch { function: index },
        );
    }
    let block = function
        .blocks
        .iter()
        .find(|block| block.id == candidate.block)
        .ok_or(TerminalPressureRematerializationError::MaterializeMismatch { function: index })?;
    let original = block
        .instructions
        .iter()
        .find(|instruction| instruction.id == *defining_instruction)
        .ok_or(TerminalPressureRematerializationError::MaterializeMismatch { function: index })?;
    if original.kind != (TerminalSelectedInstructionKind::MaterializeI64 { value: *value })
        || original.constraint != row.key
        || original.provenance != *provenance
        || original.operands.as_slice() != [make_operand(&row.operands[0], candidate.victim)]
    {
        return Err(
            TerminalPressureRematerializationError::MaterializeMismatch { function: index },
        );
    }
    let victim_range = ranges
        .virtual_registers
        .iter()
        .find(|range| range.virtual_register == candidate.victim)
        .ok_or(TerminalPressureRematerializationError::MaterializeMismatch { function: index })?;
    if !victim_range.occurrences.iter().any(|occurrence| {
        occurrence.instruction == *defining_instruction
            && occurrence.access == RegisterOperandAccess::Def
            && occurrence.point < candidate.point
    }) {
        return Err(
            TerminalPressureRematerializationError::MaterializeMismatch { function: index },
        );
    }
    let future_instruction = lookup_instruction(block, future.instruction)
        .ok_or(TerminalPressureRematerializationError::FutureUseMismatch { function: index })?;
    let future_operand = future_instruction
        .operands
        .iter()
        .find(|operand| operand.operand == future.operand)
        .ok_or(TerminalPressureRematerializationError::FutureUseMismatch { function: index })?;
    if future_operand.virtual_register != candidate.victim
        || future_operand.access != RegisterOperandAccess::Use
        || future_operand.fixed_view.is_some()
        || future_operand.class != candidate.class
    {
        return Err(TerminalPressureRematerializationError::FutureUseMismatch { function: index });
    }
    Ok(())
}

fn replay_action(
    index: usize,
    function: &mut TerminalSelectedFunction,
    action: TerminalPressureRematerializationAction,
    row: &RegisterInstructionConstraint,
) -> Result<(), TerminalPressureRematerializationError> {
    let source = function
        .virtual_registers
        .iter()
        .find(|register| register.id == action.victim)
        .cloned()
        .ok_or(TerminalPressureRematerializationError::DecisionMismatch { function: index })?;
    function.virtual_registers.push(TerminalVirtualRegister {
        id: action.result_virtual_register,
        scalar_type: source.scalar_type,
        class: source.class,
        origin: TerminalVirtualRegisterOrigin::InstructionResult {
            instruction: action.fresh_materialize,
            source_value: action.source_value,
        },
        definition_site: source.definition_site,
        entry_fixed_view: None,
    });
    let inserted = TerminalSelectedInstruction {
        id: action.fresh_materialize,
        kind: TerminalSelectedInstructionKind::MaterializeI64 {
            value: action.value,
        },
        constraint: row.key,
        operands: vec![make_operand(
            &row.operands[0],
            action.result_virtual_register,
        )],
        implicit_uses: Vec::new(),
        implicit_defs: Vec::new(),
        clobbers: Vec::new(),
        provenance: TerminalSelectedInstructionProvenance {
            values: vec![action.source_value],
            ..Default::default()
        },
    };
    let block = function
        .blocks
        .iter_mut()
        .find(|block| block.id == action.block)
        .ok_or(TerminalPressureRematerializationError::DecisionMismatch { function: index })?;
    if let Some(position) = block
        .instructions
        .iter()
        .position(|instruction| instruction.id == action.future_instruction)
    {
        rewrite(index, &mut block.instructions[position], action)?;
        block.instructions.insert(position, inserted);
    } else {
        let terminator = match &mut block.terminator {
            TerminalSelectedTerminator::ConditionalBranch { instruction, .. }
            | TerminalSelectedTerminator::Return { instruction, .. } => instruction,
        };
        if terminator.id != action.future_instruction {
            return Err(TerminalPressureRematerializationError::DecisionMismatch {
                function: index,
            });
        }
        rewrite(index, terminator, action)?;
        block.instructions.push(inserted);
    }
    Ok(())
}

fn rewrite(
    index: usize,
    instruction: &mut TerminalSelectedInstruction,
    action: TerminalPressureRematerializationAction,
) -> Result<(), TerminalPressureRematerializationError> {
    let operand = instruction
        .operands
        .iter_mut()
        .find(|operand| {
            operand.operand == action.future_operand
                && operand.virtual_register == action.victim
                && operand.access == RegisterOperandAccess::Use
                && operand.fixed_view.is_none()
        })
        .ok_or(TerminalPressureRematerializationError::DecisionMismatch { function: index })?;
    operand.virtual_register = action.result_virtual_register;
    Ok(())
}

fn make_operand(
    row: &omega_register_model::RegisterOperandConstraint,
    register: TerminalVirtualRegisterId,
) -> TerminalSelectedOperand {
    TerminalSelectedOperand {
        operand: row.operand,
        virtual_register: register,
        access: row.access,
        class: row.class,
        fixed_view: row.fixed_view,
        tied_to: row.tied_to,
        early_clobber: row.early_clobber,
    }
}

fn lookup_instruction(
    block: &omega_terminal_selected_instructions::TerminalSelectedBlock,
    id: TerminalSelectedInstructionId,
) -> Option<&TerminalSelectedInstruction> {
    block
        .instructions
        .iter()
        .find(|instruction| instruction.id == id)
        .or_else(|| match &block.terminator {
            TerminalSelectedTerminator::ConditionalBranch { instruction, .. }
            | TerminalSelectedTerminator::Return { instruction, .. }
                if instruction.id == id =>
            {
                Some(instruction)
            }
            _ => None,
        })
}

fn validate_dense(
    index: usize,
    function: &TerminalSelectedFunction,
) -> Result<(), TerminalPressureRematerializationError> {
    if function
        .virtual_registers
        .iter()
        .enumerate()
        .any(|(position, register)| usize::try_from(register.id.0) != Ok(position))
    {
        return Err(TerminalPressureRematerializationError::FunctionMismatch { function: index });
    }
    let count = instruction_count(index, function)?;
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
    if ids != (0..count).collect::<Vec<_>>() {
        return Err(TerminalPressureRematerializationError::FunctionMismatch { function: index });
    }
    Ok(())
}

fn instruction_count(
    index: usize,
    function: &TerminalSelectedFunction,
) -> Result<u32, TerminalPressureRematerializationError> {
    let count = function
        .blocks
        .iter()
        .try_fold(0usize, |total, block| {
            total.checked_add(block.instructions.len().checked_add(1)?)
        })
        .ok_or(TerminalPressureRematerializationError::IdentifierOverflow { function: index })?;
    u32::try_from(count)
        .map_err(|_| TerminalPressureRematerializationError::IdentifierOverflow { function: index })
}

fn independent_usage(
    selected: &impl ValidatedTerminalSelectedAnalysis,
    applied: usize,
) -> Result<OptimizationWorkUsage, TerminalPressureRematerializationError> {
    let rule_evaluations = u64::try_from(selected.selected_plan().functions.len())
        .map_err(|_| TerminalPressureRematerializationError::WorkOverflow)?;
    let validation_steps = selected
        .selected_plan()
        .functions
        .iter()
        .try_fold(0u64, |total, function| {
            let instructions = function.blocks.iter().try_fold(0u64, |count, block| {
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
        .ok_or(TerminalPressureRematerializationError::WorkOverflow)?;
    let applied =
        u64::try_from(applied).map_err(|_| TerminalPressureRematerializationError::WorkOverflow)?;
    Ok(OptimizationWorkUsage {
        rule_evaluations,
        candidates: applied,
        validation_steps,
        commits: applied,
        iterations: 1,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn independent_replay_matches_proposal_and_rejects_recipe_corruption() {
        let (selected, ranges, recovery, row) =
            crate::pressure_rematerialization_compute::tests::fixture();
        let candidate = recovery.functions[0].classification.as_ref().unwrap();
        let (functions, proposed) = crate::pressure_rematerialization_compute::build_functions(
            &selected, &ranges, &recovery, &row,
        )
        .unwrap();
        let action = functions[0].action.unwrap();
        validate_action(
            0,
            &selected.functions[0],
            &ranges.functions[0],
            candidate,
            action,
            &row,
        )
        .unwrap();
        let mut replayed = selected.clone();
        replay_action(0, &mut replayed.functions[0], action, &row).unwrap();
        assert_eq!(replayed, proposed);

        let mut corrupt = action;
        corrupt.future_operand = 1;
        assert_eq!(
            validate_action(
                0,
                &selected.functions[0],
                &ranges.functions[0],
                candidate,
                corrupt,
                &row,
            ),
            Err(TerminalPressureRematerializationError::DecisionMismatch { function: 0 })
        );
    }
}
