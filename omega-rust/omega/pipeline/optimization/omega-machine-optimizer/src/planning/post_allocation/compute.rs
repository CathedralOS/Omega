//! Post-allocation plan construction coordination.
//!
//! Root custody is admitted first, ordinary and structural functions descend
//! through separate builders, and this file performs the sole final plan
//! assembly and identity assignment.

mod alternative;
mod instruction;
mod ordinary;
mod roots;
mod structural;
#[cfg(test)]
mod tests;

use omega_regalloc::{
    ValidatedAllocationLegality, ValidatedLiveRanges, ValidatedPostAllocationOptimizationManifest,
    ValidatedRegisterHomes, ValidatedSelectedAnalysis,
};
use omega_register_model::{
    TargetRegisterEnvironmentIdentity, ValidatedPhysicalRegisterModel,
    ValidatedRegisterConstraintCatalog,
};

use crate::{
    MachineAlternativeChoiceRule, PostAllocationMachineError, PostAllocationMachineIdentity,
    PostAllocationMachinePlan, ValidatedPreAllocationMachineEffects,
    post_allocation_machine_identity,
};

#[allow(clippy::too_many_arguments)]
pub(crate) fn compute_terminal_post_allocation_machine_plan<S: ValidatedSelectedAnalysis>(
    selected: &S,
    effects: &ValidatedPreAllocationMachineEffects,
    ranges: &ValidatedLiveRanges,
    legality: &ValidatedAllocationLegality,
    homes: &ValidatedRegisterHomes,
    manifest: &ValidatedPostAllocationOptimizationManifest,
    register_environment: TargetRegisterEnvironmentIdentity,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
) -> Result<PostAllocationMachinePlan, PostAllocationMachineError> {
    roots::validate(
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
    let selected_plan = selected.selected_plan();
    let functions = ordinary::build_functions(selected_plan, effects, homes, physical)?;
    let structural_unit_functions =
        structural::build_functions(selected_plan, effects, homes, physical)?;
    let mut plan = PostAllocationMachinePlan {
        identity: PostAllocationMachineIdentity::from_bytes([0; 32]),
        selected: selected.selected_identity(),
        effects: effects.receipt().identity(),
        ranges: ranges.receipt().identity(),
        legality: legality.receipt().identity(),
        homes: homes.receipt().identity(),
        post_allocation_manifest: manifest.record().identity,
        target: selected_plan.target,
        register_environment,
        physical_register_model: physical.identity(),
        register_constraints: effects.plan().register_constraints,
        machine_effect_catalog: effects.plan().machine_effect_catalog,
        choice_rule: MachineAlternativeChoiceRule::UniqueApplicableInCatalogOrderV1,
        functions,
        structural_unit_functions,
    };
    plan.identity = post_allocation_machine_identity(&plan);
    Ok(plan)
}
