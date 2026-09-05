//! Optimizer module role: executable entrance. Independent post-allocation plan replay.
//!
//! Validation proceeds in visible rejection order: custody roots, ordinary
//! functions, structural-Unit functions, canonical identity, then receipt.
//! Each child reconstructs from validated inputs and never calls production
//! construction.

mod instruction;
mod ordinary;
mod roots;
mod structural;

use omega_register_model::{
    TargetRegisterEnvironmentIdentity, ValidatedPhysicalRegisterModel,
    ValidatedRegisterConstraintCatalog,
};
use omega_selected_instructions_to_register_homes::{
    ValidatedAllocationLegality, ValidatedLiveRanges, ValidatedPostAllocationOptimizationManifest,
    ValidatedRegisterHomes, ValidatedSelectedAnalysis,
};

use crate::{
    PostAllocationMachineError, PostAllocationMachinePlan, ValidatedPostAllocationMachinePlan,
    ValidatedPreAllocationMachineEffects, post_allocation_machine_identity,
    post_allocation_receipt,
};

/// Independently reconstruct and admit a proposed post-allocation plan.
#[allow(clippy::too_many_arguments)]
pub fn validate_post_allocation_machine_plan<S: ValidatedSelectedAnalysis>(
    selected: &S,
    effects: &ValidatedPreAllocationMachineEffects,
    ranges: &ValidatedLiveRanges,
    legality: &ValidatedAllocationLegality,
    homes: &ValidatedRegisterHomes,
    manifest: &ValidatedPostAllocationOptimizationManifest,
    register_environment: TargetRegisterEnvironmentIdentity,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
    plan: PostAllocationMachinePlan,
) -> Result<ValidatedPostAllocationMachinePlan, PostAllocationMachineError> {
    roots::validate_roots(
        selected,
        effects,
        ranges,
        legality,
        homes,
        manifest,
        register_environment,
        physical,
        constraints,
        &plan,
    )?;
    ordinary::validate_ordinary_functions(selected, effects, homes, physical, &plan)?;
    structural::validate_structural_functions(selected, effects, homes, physical, &plan)?;
    if post_allocation_machine_identity(&plan) != plan.identity {
        return Err(PostAllocationMachineError::IdentityMismatch);
    }
    let receipt = post_allocation_receipt(&plan)?;
    Ok(ValidatedPostAllocationMachinePlan::new(plan, receipt))
}
