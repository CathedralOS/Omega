use omega_register_model::{
    TargetRegisterEnvironmentConstraintKeys, TargetRegisterEnvironmentIdentity,
    ValidatedRegisterConstraintCatalog,
};
use omega_terminal_target_operations_to_selected_instructions::terminal_selected_instruction_plan_identity;

use crate::{
    TerminalLiteralFoldError, TerminalLiteralFoldPlan, TerminalLiteralFoldPolicy,
    TerminalLiteralFoldValidationReceipt, ValidatedTerminalAllocationLegality,
    ValidatedTerminalAllocatorAvailability, ValidatedTerminalLiteralFold,
    ValidatedTerminalLiveRanges, ValidatedTerminalRecoveryClassifications,
    ValidatedTerminalSelectedAnalysis, ValidatedTerminalSpillChoices,
    literal_fold_transform::{
        ensure_budget, fold_usage, immediate_row, replay_actions, validate_literal_fold_roots,
    },
    terminal_literal_fold_identity,
};

#[allow(clippy::too_many_arguments)]
pub fn validate_terminal_literal_fold<S: ValidatedTerminalSelectedAnalysis>(
    selected: &S,
    ranges: &ValidatedTerminalLiveRanges,
    legality: &ValidatedTerminalAllocationLegality,
    spill_choices: &ValidatedTerminalSpillChoices,
    recovery: &ValidatedTerminalRecoveryClassifications,
    availability: &ValidatedTerminalAllocatorAvailability,
    register_environment: TargetRegisterEnvironmentIdentity,
    constraints: &ValidatedRegisterConstraintCatalog,
    selected_keys: TargetRegisterEnvironmentConstraintKeys,
    plan: TerminalLiteralFoldPlan,
) -> Result<ValidatedTerminalLiteralFold, TerminalLiteralFoldError> {
    validate_literal_fold_roots(
        selected,
        ranges,
        legality,
        spill_choices,
        recovery,
        availability,
        register_environment,
    )?;
    if plan.source_selected != selected.selected_identity()
        || plan.spill_choices != spill_choices.receipt().identity()
        || plan.recovery_classifications != recovery.receipt().identity()
        || plan.ranges != ranges.receipt().identity()
        || plan.legality != legality.receipt().identity()
        || plan.register_environment != register_environment
        || plan.allocator_availability != availability.receipt().identity()
        || plan.optimization_unit != selected.optimization_unit_identity()
        || plan.fuel_schedule != selected.fuel_schedule_identity()
    {
        return Err(TerminalLiteralFoldError::RootMismatch);
    }
    if plan.policy != TerminalLiteralFoldPolicy::SelectedIncomingU12ExactAddImmediateV1 {
        return Err(TerminalLiteralFoldError::UnsupportedPolicy);
    }
    let row = immediate_row(constraints, selected_keys)?;
    let (expected_functions, transformed) = replay_actions(selected, recovery, row)?;
    if plan.functions != expected_functions {
        return Err(TerminalLiteralFoldError::DecisionMismatch { function: 0 });
    }
    let applied_count = expected_functions
        .iter()
        .filter(|function| function.action.is_some())
        .count();
    let usage = fold_usage(selected, applied_count)?;
    if plan.usage != usage {
        return Err(TerminalLiteralFoldError::UsageMismatch);
    }
    ensure_budget(plan.usage, plan.budget)?;
    let transformed_selected = terminal_selected_instruction_plan_identity(&transformed);
    if plan.transformed_selected != transformed_selected {
        return Err(TerminalLiteralFoldError::TransformedIdentityMismatch);
    }
    let receipt = TerminalLiteralFoldValidationReceipt {
        identity: terminal_literal_fold_identity(&plan),
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
        applied_count,
    };
    Ok(ValidatedTerminalLiteralFold {
        plan,
        transformed,
        receipt,
    })
}
