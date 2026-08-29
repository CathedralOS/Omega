use omega_optimization_core::OptimizationWorkUsage;
use omega_register_model::{
    RegisterInstructionConstraint, RegisterOperandAccess, TargetRegisterEnvironmentConstraintKeys,
    TargetRegisterEnvironmentIdentity, ValidatedPhysicalRegisterModel,
    ValidatedRegisterConstraintCatalog, ValidatedRegisterReservationProfile,
    target_register_environment_identity,
};
use omega_selected_instructions::{
    SelectedFunction, SelectedInstruction, SelectedInstructionId, SelectedInstructionKind,
    SelectedInstructionProvenance, SelectedOperand, SelectedTerminator, VirtualRegister,
    VirtualRegisterId, VirtualRegisterOrigin,
};
use omega_target_operations_to_selected_instructions::selected_instruction_plan_identity;

use crate::*;

/// Independently authenticates and replays the plain rematerialization recipe.
/// It does not call the proposal builder or accept a decoded artifact as proof.
#[allow(clippy::too_many_arguments)]
pub fn validate_pressure_rematerialization<S: ValidatedSelectedAnalysis>(
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
    plan: PressureRematerializationPlan,
) -> Result<ValidatedPressureRematerialization, PressureRematerializationError> {
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
        return Err(PressureRematerializationError::RootMismatch);
    }

    let row = constraints
        .catalog()
        .constraints
        .iter()
        .find(|row| row.key == selected_keys.materialize_i64)
        .ok_or(PressureRematerializationError::MaterializeConstraintMismatch)?;
    validate_materialize_row(row)?;
    let mut transformed = selected.selected_plan().clone();
    let mut applied = 0usize;
    let mut rewritten_uses = 0usize;
    for index in 0..plan.functions.len() {
        let source = &selected.selected_plan().functions[index];
        let function_plan = &plan.functions[index];
        let range_function = &ranges.plan().functions[index];
        let recovery_function = &recovery.plan().functions[index];
        if source.machine != function_plan.machine
            || source.machine != range_function.machine
            || source.machine != recovery_function.machine
        {
            return Err(PressureRematerializationError::FunctionMismatch { function: index });
        }
        validate_dense(index, source)?;
        match (
            &recovery_function.classification,
            function_plan.action.as_ref(),
        ) {
            (None, None) => {}
            (Some(candidate), Some(action)) => {
                validate_action(
                    index,
                    source,
                    range_function,
                    candidate,
                    action,
                    row,
                    plan.policy,
                )?;
                replay_action(index, &mut transformed.functions[index], action, row)?;
                applied = applied
                    .checked_add(1)
                    .ok_or(PressureRematerializationError::WorkOverflow)?;
                rewritten_uses = rewritten_uses
                    .checked_add(action.rewrites.len())
                    .ok_or(PressureRematerializationError::WorkOverflow)?;
            }
            _ => {
                return Err(PressureRematerializationError::DecisionMismatch { function: index });
            }
        }
    }
    if applied == 0 {
        return Err(PressureRematerializationError::NoAction);
    }
    let usage = independent_usage(selected, applied, rewritten_uses)?;
    if plan.usage != usage {
        return Err(PressureRematerializationError::UsageMismatch);
    }
    if !plan.usage.within(plan.budget) {
        return Err(PressureRematerializationError::BudgetExceeded {
            required: plan.usage,
            budget: plan.budget,
        });
    }
    let transformed_selected = selected_instruction_plan_identity(&transformed);
    if plan.transformed_selected != transformed_selected {
        return Err(PressureRematerializationError::TransformedIdentityMismatch);
    }
    let receipt = PressureRematerializationValidationReceipt {
        identity: pressure_rematerialization_identity(&plan),
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
        rewritten_use_count: rewritten_uses,
    };
    Ok(ValidatedPressureRematerialization {
        plan,
        transformed,
        receipt,
    })
}

fn validate_materialize_row(
    row: &RegisterInstructionConstraint,
) -> Result<(), PressureRematerializationError> {
    let [result] = row.operands.as_slice() else {
        return Err(PressureRematerializationError::MaterializeConstraintMismatch);
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
        return Err(PressureRematerializationError::MaterializeConstraintMismatch);
    }
    Ok(())
}

fn validate_action(
    index: usize,
    function: &SelectedFunction,
    ranges: &FunctionLiveRanges,
    candidate: &PressureRecoveryClassification,
    action: &PressureRematerializationAction,
    row: &RegisterInstructionConstraint,
    policy: PressureRematerializationPolicy,
) -> Result<(), PressureRematerializationError> {
    let RecoveryVictimRole::ActiveResident {
        current_view,
        reclaimed_view,
    } = candidate.role
    else {
        return Err(PressureRematerializationError::UnsupportedVictimRole { function: index });
    };
    let RecoveryClassification::ImmediateU64RematerializationCandidate {
        defining_instruction,
        source_value,
        value,
        provenance,
        future_uses,
    } = &candidate.classification
    else {
        return Err(PressureRematerializationError::ClassificationNotAdmitted { function: index });
    };
    let valid_arity = match policy {
        PressureRematerializationPolicy::SelectedActiveResidentImmediateU64BeforeSingleFutureFlexibleUseV1 => future_uses.len() == 1,
        PressureRematerializationPolicy::SelectedActiveResidentImmediateU64BeforeFirstOfMultipleFutureFlexibleUsesV1 => future_uses.len() >= 2,
    };
    if !valid_arity
        || future_uses.windows(2).any(|pair| pair[0] >= pair[1])
        || future_uses
            .iter()
            .any(|future| future.block != candidate.block || future.point <= candidate.point)
    {
        return Err(PressureRematerializationError::FutureUseMismatch { function: index });
    }
    let expected_instruction = SelectedInstructionId(instruction_count(index, function)?);
    let expected_register = VirtualRegisterId(
        u32::try_from(function.virtual_registers.len())
            .map_err(|_| PressureRematerializationError::IdentifierOverflow { function: index })?,
    );
    if action.block != candidate.block
        || action.pressure_point != candidate.point
        || action.victim != candidate.victim
        || action.current_view != current_view
        || action.reclaimed_view != reclaimed_view
        || action.original_materialize != *defining_instruction
        || action.source_value != *source_value
        || action.value != *value
        || action.rewrites.len() != future_uses.len()
        || !action
            .rewrites
            .iter()
            .zip(future_uses)
            .all(|(rewrite, future)| {
                rewrite.point == future.point
                    && rewrite.instruction == future.instruction
                    && rewrite.operand == future.operand
            })
        || action.fresh_materialize != expected_instruction
        || action.result_virtual_register != expected_register
        || action.materialize_constraint != row.key
    {
        return Err(PressureRematerializationError::DecisionMismatch { function: index });
    }
    let victim = function
        .virtual_registers
        .iter()
        .find(|register| register.id == candidate.victim)
        .ok_or(PressureRematerializationError::MaterializeMismatch { function: index })?;
    if victim.scalar_type != candidate.scalar_type
        || victim.class != candidate.class
        || victim.origin != candidate.origin
        || victim.definition_site != candidate.definition_site
        || victim.entry_fixed_view.is_some()
        || row.operands[0].class != victim.class
    {
        return Err(PressureRematerializationError::MaterializeMismatch { function: index });
    }
    let block = function
        .blocks
        .iter()
        .find(|block| block.id == candidate.block)
        .ok_or(PressureRematerializationError::MaterializeMismatch { function: index })?;
    let original = block
        .instructions
        .iter()
        .find(|instruction| instruction.id == *defining_instruction)
        .ok_or(PressureRematerializationError::MaterializeMismatch { function: index })?;
    if original.kind != (SelectedInstructionKind::MaterializeI64 { value: *value })
        || original.constraint != row.key
        || original.provenance != *provenance
        || original.operands.as_slice() != [make_operand(&row.operands[0], candidate.victim)]
    {
        return Err(PressureRematerializationError::MaterializeMismatch { function: index });
    }
    let victim_range = ranges
        .virtual_registers
        .iter()
        .find(|range| range.virtual_register == candidate.victim)
        .ok_or(PressureRematerializationError::MaterializeMismatch { function: index })?;
    if !victim_range.occurrences.iter().any(|occurrence| {
        occurrence.instruction == *defining_instruction
            && occurrence.access == RegisterOperandAccess::Def
            && occurrence.point < candidate.point
    }) {
        return Err(PressureRematerializationError::MaterializeMismatch { function: index });
    }
    for future in future_uses {
        let instruction = lookup_instruction(block, future.instruction)
            .ok_or(PressureRematerializationError::FutureUseMismatch { function: index })?;
        let matching = instruction
            .operands
            .iter()
            .filter(|operand| operand.operand == future.operand)
            .collect::<Vec<_>>();
        if matching.len() != 1
            || matching[0].virtual_register != candidate.victim
            || matching[0].access != RegisterOperandAccess::Use
            || matching[0].fixed_view.is_some()
            || matching[0].class != candidate.class
        {
            return Err(PressureRematerializationError::FutureUseMismatch { function: index });
        }
    }
    Ok(())
}

fn replay_action(
    index: usize,
    function: &mut SelectedFunction,
    action: &PressureRematerializationAction,
    row: &RegisterInstructionConstraint,
) -> Result<(), PressureRematerializationError> {
    let source = function
        .virtual_registers
        .iter()
        .find(|register| register.id == action.victim)
        .cloned()
        .ok_or(PressureRematerializationError::DecisionMismatch { function: index })?;
    function.virtual_registers.push(VirtualRegister {
        id: action.result_virtual_register,
        scalar_type: source.scalar_type,
        class: source.class,
        origin: VirtualRegisterOrigin::InstructionResult {
            instruction: action.fresh_materialize,
            source_value: action.source_value,
        },
        definition_site: source.definition_site,
        entry_fixed_view: None,
    });
    let inserted = SelectedInstruction {
        id: action.fresh_materialize,
        kind: SelectedInstructionKind::MaterializeI64 {
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
        provenance: SelectedInstructionProvenance {
            values: vec![action.source_value],
            ..Default::default()
        },
    };
    let block = function
        .blocks
        .iter_mut()
        .find(|block| block.id == action.block)
        .ok_or(PressureRematerializationError::DecisionMismatch { function: index })?;
    for rewrite_row in &action.rewrites {
        let mut matched = 0usize;
        for instruction in &mut block.instructions {
            if instruction.id == rewrite_row.instruction {
                rewrite(
                    index,
                    instruction,
                    action.victim,
                    action.result_virtual_register,
                    *rewrite_row,
                )?;
                matched += 1;
            }
        }
        let terminator = match &mut block.terminator {
            SelectedTerminator::ConditionalBranch { instruction, .. }
            | SelectedTerminator::Return { instruction, .. } => instruction,
        };
        if terminator.id == rewrite_row.instruction {
            rewrite(
                index,
                terminator,
                action.victim,
                action.result_virtual_register,
                *rewrite_row,
            )?;
            matched += 1;
        }
        if matched != 1 {
            return Err(PressureRematerializationError::DecisionMismatch { function: index });
        }
    }
    let first = action
        .rewrites
        .first()
        .ok_or(PressureRematerializationError::DecisionMismatch { function: index })?;
    if let Some(position) = block
        .instructions
        .iter()
        .position(|instruction| instruction.id == first.instruction)
    {
        block.instructions.insert(position, inserted);
    } else {
        let terminator = match &block.terminator {
            SelectedTerminator::ConditionalBranch { instruction, .. }
            | SelectedTerminator::Return { instruction, .. } => instruction,
        };
        if terminator.id != first.instruction {
            return Err(PressureRematerializationError::DecisionMismatch { function: index });
        }
        block.instructions.push(inserted);
    }
    Ok(())
}

fn rewrite(
    index: usize,
    instruction: &mut SelectedInstruction,
    victim: VirtualRegisterId,
    result: VirtualRegisterId,
    rewrite: PressureRematerializationRewrite,
) -> Result<(), PressureRematerializationError> {
    let operand = instruction
        .operands
        .iter_mut()
        .find(|operand| {
            operand.operand == rewrite.operand
                && operand.virtual_register == victim
                && operand.access == RegisterOperandAccess::Use
                && operand.fixed_view.is_none()
        })
        .ok_or(PressureRematerializationError::DecisionMismatch { function: index })?;
    operand.virtual_register = result;
    Ok(())
}

fn make_operand(
    row: &omega_register_model::RegisterOperandConstraint,
    register: VirtualRegisterId,
) -> SelectedOperand {
    SelectedOperand {
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
    block: &omega_selected_instructions::SelectedBlock,
    id: SelectedInstructionId,
) -> Option<&SelectedInstruction> {
    block
        .instructions
        .iter()
        .find(|instruction| instruction.id == id)
        .or_else(|| match &block.terminator {
            SelectedTerminator::ConditionalBranch { instruction, .. }
            | SelectedTerminator::Return { instruction, .. }
                if instruction.id == id =>
            {
                Some(instruction)
            }
            _ => None,
        })
}

fn validate_dense(
    index: usize,
    function: &SelectedFunction,
) -> Result<(), PressureRematerializationError> {
    if function
        .virtual_registers
        .iter()
        .enumerate()
        .any(|(position, register)| usize::try_from(register.id.0) != Ok(position))
    {
        return Err(PressureRematerializationError::FunctionMismatch { function: index });
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
                    SelectedTerminator::ConditionalBranch { instruction, .. }
                    | SelectedTerminator::Return { instruction, .. } => instruction.id.0,
                }))
        })
        .collect::<Vec<_>>();
    ids.sort_unstable();
    if ids != (0..count).collect::<Vec<_>>() {
        return Err(PressureRematerializationError::FunctionMismatch { function: index });
    }
    Ok(())
}

fn instruction_count(
    index: usize,
    function: &SelectedFunction,
) -> Result<u32, PressureRematerializationError> {
    let count = function
        .blocks
        .iter()
        .try_fold(0usize, |total, block| {
            total.checked_add(block.instructions.len().checked_add(1)?)
        })
        .ok_or(PressureRematerializationError::IdentifierOverflow { function: index })?;
    u32::try_from(count)
        .map_err(|_| PressureRematerializationError::IdentifierOverflow { function: index })
}

fn independent_usage(
    selected: &impl ValidatedSelectedAnalysis,
    applied: usize,
    rewritten_uses: usize,
) -> Result<OptimizationWorkUsage, PressureRematerializationError> {
    let rule_evaluations = u64::try_from(selected.selected_plan().functions.len())
        .map_err(|_| PressureRematerializationError::WorkOverflow)?;
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
        .ok_or(PressureRematerializationError::WorkOverflow)?
        .checked_add(
            u64::try_from(rewritten_uses)
                .map_err(|_| PressureRematerializationError::WorkOverflow)?,
        )
        .ok_or(PressureRematerializationError::WorkOverflow)?;
    let applied =
        u64::try_from(applied).map_err(|_| PressureRematerializationError::WorkOverflow)?;
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
            crate::rules::pressure_rematerialization::compute::tests::fixture();
        let candidate = recovery.functions[0].classification.as_ref().unwrap();
        let (functions, proposed) = crate::rules::pressure_rematerialization::compute::build_functions(
            &selected,
            &ranges,
            &recovery,
            &row,
            PressureRematerializationPolicy::SelectedActiveResidentImmediateU64BeforeSingleFutureFlexibleUseV1,
        )
        .unwrap();
        let action = functions[0].action.as_ref().unwrap();
        validate_action(
            0,
            &selected.functions[0],
            &ranges.functions[0],
            candidate,
            action,
            &row,
            PressureRematerializationPolicy::SelectedActiveResidentImmediateU64BeforeSingleFutureFlexibleUseV1,
        )
        .unwrap();
        let mut replayed = selected.clone();
        replay_action(0, &mut replayed.functions[0], action, &row).unwrap();
        assert_eq!(replayed, proposed);

        let mut corrupt = action.clone();
        corrupt.rewrites[0].operand = 1;
        assert_eq!(
            validate_action(
                0,
                &selected.functions[0],
                &ranges.functions[0],
                candidate,
                &corrupt,
                &row,
                PressureRematerializationPolicy::SelectedActiveResidentImmediateU64BeforeSingleFutureFlexibleUseV1,
            ),
            Err(PressureRematerializationError::DecisionMismatch { function: 0 })
        );
    }

    #[test]
    fn independent_replay_reconstructs_multiple_use_suffix_and_rejects_rewrite_corruption() {
        let (selected, ranges, recovery, row) =
            crate::rules::pressure_rematerialization::compute::tests::multiple_future_fixture();
        let candidate = recovery.functions[0].classification.as_ref().unwrap();
        let policy = PressureRematerializationPolicy::SelectedActiveResidentImmediateU64BeforeFirstOfMultipleFutureFlexibleUsesV1;
        let (functions, proposed) =
            crate::rules::pressure_rematerialization::compute::build_functions(
                &selected, &ranges, &recovery, &row, policy,
            )
            .unwrap();
        let action = functions[0].action.as_ref().unwrap();
        validate_action(
            0,
            &selected.functions[0],
            &ranges.functions[0],
            candidate,
            action,
            &row,
            policy,
        )
        .unwrap();
        let mut replayed = selected.clone();
        replay_action(0, &mut replayed.functions[0], action, &row).unwrap();
        assert_eq!(replayed, proposed);

        let mut removed = action.clone();
        removed.rewrites.pop();
        assert_eq!(
            validate_action(
                0,
                &selected.functions[0],
                &ranges.functions[0],
                candidate,
                &removed,
                &row,
                policy,
            ),
            Err(PressureRematerializationError::DecisionMismatch { function: 0 })
        );

        let mut reordered = action.clone();
        reordered.rewrites.swap(0, 1);
        assert_eq!(
            validate_action(
                0,
                &selected.functions[0],
                &ranges.functions[0],
                candidate,
                &reordered,
                &row,
                policy,
            ),
            Err(PressureRematerializationError::DecisionMismatch { function: 0 })
        );

        let mut corrupt_point = action.clone();
        corrupt_point.rewrites[1].point.0 += 1;
        assert_eq!(
            validate_action(
                0,
                &selected.functions[0],
                &ranges.functions[0],
                candidate,
                &corrupt_point,
                &row,
                policy,
            ),
            Err(PressureRematerializationError::DecisionMismatch { function: 0 })
        );
    }
}
