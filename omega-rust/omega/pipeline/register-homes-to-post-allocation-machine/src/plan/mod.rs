//! Optimizer module role: executable entrance. Deterministic post-allocation machine-plan construction and replay.

mod compute;
mod model;
mod validate;

pub use ::physical_instructions::*;
pub use model::*;
pub use validate::validate_post_allocation_machine_plan;

/// Join one validated selected CFG, its pre-allocation machine effects, and
/// its independently validated physical homes. Target applicability must
/// identify exactly one legal alternative for every instruction.
#[allow(clippy::too_many_arguments)]
pub fn analyze_post_allocation_machine_plan<
    S: selected_instructions_to_register_homes::ValidatedSelectedAnalysis,
>(
    selected: &S,
    effects: &selected_instructions_to_register_homes::ValidatedPreAllocationMachineEffects,
    ranges: &selected_instructions_to_register_homes::ValidatedLiveRanges,
    legality: &selected_instructions_to_register_homes::ValidatedAllocationLegality,
    homes: &selected_instructions_to_register_homes::ValidatedRegisterHomes,
    manifest: &selected_instructions_to_register_homes::ValidatedPostAllocationOptimizationManifest,
    register_environment: register_model::TargetRegisterEnvironmentIdentity,
    physical: &register_model::ValidatedPhysicalRegisterModel,
    constraints: &register_model::ValidatedRegisterConstraintCatalog,
) -> Result<ValidatedPostAllocationMachinePlan, PostAllocationMachineError> {
    let plan = compute::compute_terminal_post_allocation_machine_plan(
        selected,
        effects,
        ranges,
        legality,
        homes,
        manifest,
        register_environment,
        physical,
        constraints,
    )?;
    validate_post_allocation_machine_plan(
        selected,
        effects,
        ranges,
        legality,
        homes,
        manifest,
        register_environment,
        physical,
        constraints,
        plan,
    )
}
