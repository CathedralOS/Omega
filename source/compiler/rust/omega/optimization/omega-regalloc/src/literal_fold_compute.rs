use omega_optimization_core::OptimizationWorkBudget;
use omega_register_model::{
    TargetRegisterEnvironmentConstraintKeys, TargetRegisterEnvironmentIdentity,
    ValidatedRegisterConstraintCatalog,
};
use omega_terminal_target_operations_to_selected_instructions::terminal_selected_instruction_plan_identity;

use crate::{
    TerminalLiteralFoldError, TerminalLiteralFoldPlan, TerminalLiteralFoldPolicy,
    ValidatedTerminalAllocationLegality, ValidatedTerminalAllocatorAvailability,
    ValidatedTerminalLiveRanges, ValidatedTerminalRecoveryClassifications,
    ValidatedTerminalSelectedAnalysis, ValidatedTerminalSpillChoices,
    literal_fold_transform::{
        ensure_budget, fold_usage, immediate_row, replay_actions, validate_literal_fold_roots,
    },
};

#[allow(clippy::too_many_arguments)]
pub(crate) fn compute_terminal_literal_fold<S: ValidatedTerminalSelectedAnalysis>(
    selected: &S,
    ranges: &ValidatedTerminalLiveRanges,
    legality: &ValidatedTerminalAllocationLegality,
    spill_choices: &ValidatedTerminalSpillChoices,
    recovery: &ValidatedTerminalRecoveryClassifications,
    availability: &ValidatedTerminalAllocatorAvailability,
    register_environment: TargetRegisterEnvironmentIdentity,
    constraints: &ValidatedRegisterConstraintCatalog,
    selected_keys: TargetRegisterEnvironmentConstraintKeys,
    policy: TerminalLiteralFoldPolicy,
    budget: OptimizationWorkBudget,
) -> Result<TerminalLiteralFoldPlan, TerminalLiteralFoldError> {
    validate_literal_fold_roots(
        selected,
        ranges,
        legality,
        spill_choices,
        recovery,
        availability,
        register_environment,
    )?;
    if policy != TerminalLiteralFoldPolicy::SelectedIncomingU12ExactAddImmediateV1 {
        return Err(TerminalLiteralFoldError::UnsupportedPolicy);
    }
    let row = immediate_row(constraints, selected_keys)?;
    let (functions, transformed) = replay_actions(selected, recovery, row)?;
    let usage = fold_usage(
        selected,
        functions
            .iter()
            .filter(|function| function.action.is_some())
            .count(),
    )?;
    ensure_budget(usage, budget)?;
    Ok(TerminalLiteralFoldPlan {
        source_selected: selected.selected_identity(),
        spill_choices: spill_choices.receipt().identity(),
        recovery_classifications: recovery.receipt().identity(),
        ranges: ranges.receipt().identity(),
        legality: legality.receipt().identity(),
        register_environment,
        allocator_availability: availability.receipt().identity(),
        optimization_unit: selected.optimization_unit_identity(),
        fuel_schedule: selected.fuel_schedule_identity(),
        policy,
        budget,
        usage,
        functions,
        transformed_selected: terminal_selected_instruction_plan_identity(&transformed),
    })
}
