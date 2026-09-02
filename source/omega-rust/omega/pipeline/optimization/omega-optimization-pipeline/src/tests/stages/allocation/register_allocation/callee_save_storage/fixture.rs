use crate::tests::*;

pub(super) fn call_requirements(
    target: NativeTarget,
) -> (
    ValidatedAllocatedCalleeSavedRequirements,
    ValidatedTargetRegisterEnvironment,
) {
    requirements(register_homes(staged_scalar_call_unit(target)))
}

pub(super) fn ordinary_requirements(
    target: NativeTarget,
) -> (
    ValidatedAllocatedCalleeSavedRequirements,
    ValidatedTargetRegisterEnvironment,
) {
    requirements(register_homes(staged_conditional(target)))
}

fn register_homes(selected: StagedOptimizedSelectedInstructions) -> StagedOptimizedRegisterHomes {
    stage_optimized_register_homes(
        stage_optimized_allocation_legality(
            stage_optimized_live_ranges(stage_optimized_liveness(selected).unwrap()).unwrap(),
        )
        .unwrap(),
    )
    .unwrap()
}

fn requirements(
    homes: StagedOptimizedRegisterHomes,
) -> (
    ValidatedAllocatedCalleeSavedRequirements,
    ValidatedTargetRegisterEnvironment,
) {
    let environment = homes
        .legality_stage()
        .live_range_stage()
        .liveness_stage()
        .selected_stage()
        .register_environment()
        .clone();
    let requirements = stage_allocated_callee_saved_requirements(
        &homes,
        AllocatedCalleeSavedRequirementPolicy::AllocatedSelectedWritesIntersectAbiPreservationV1,
        wide_budget(),
    )
    .unwrap();
    (requirements, environment)
}

pub(super) fn stage(
    requirements: &ValidatedAllocatedCalleeSavedRequirements,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<ValidatedNonAuthoritativeCalleeSaveStorage, NonAuthoritativeCalleeSaveStorageError> {
    stage_non_authoritative_callee_save_storage(
        requirements,
        environment,
        NonAuthoritativeCalleeSaveStoragePolicy::CanonicalTargetPreservationGroupsV1,
        budget,
    )
}

pub(super) fn wide_budget() -> OptimizationWorkBudget {
    OptimizationWorkBudget::new(1_000_000, 1_000_000, 1_000_000, 1_000_000, 1_000_000).unwrap()
}

pub(super) fn exact_budget(usage: OptimizationWorkUsage) -> OptimizationWorkBudget {
    OptimizationWorkBudget::new(
        usage.rule_evaluations,
        usage.candidates,
        usage.validation_steps,
        usage.commits,
        usage.iterations,
    )
    .unwrap()
}
