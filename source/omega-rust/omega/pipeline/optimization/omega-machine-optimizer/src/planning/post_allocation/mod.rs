//! Deterministic post-allocation machine-plan construction and replay.

mod codec;
mod compute;
mod identity;
mod model;
mod validate;

pub use codec::PostAllocationMachineDecodeError;
pub use identity::post_allocation_machine_identity;
pub use model::*;
pub use validate::validate_post_allocation_machine_plan;

/// Join one validated selected CFG, its pre-allocation machine effects, and
/// its independently validated physical homes. Target applicability must
/// identify exactly one legal alternative for every instruction.
#[allow(clippy::too_many_arguments)]
pub fn analyze_post_allocation_machine_plan<S: omega_regalloc::ValidatedSelectedAnalysis>(
    selected: &S,
    effects: &crate::ValidatedPreAllocationMachineEffects,
    ranges: &omega_regalloc::ValidatedLiveRanges,
    legality: &omega_regalloc::ValidatedAllocationLegality,
    homes: &omega_regalloc::ValidatedRegisterHomes,
    manifest: &omega_regalloc::ValidatedPostAllocationOptimizationManifest,
    register_environment: omega_register_model::TargetRegisterEnvironmentIdentity,
    physical: &omega_register_model::ValidatedPhysicalRegisterModel,
    constraints: &omega_register_model::ValidatedRegisterConstraintCatalog,
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
