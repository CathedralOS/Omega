//! Post-allocation plan construction coordination.
//!
//! Root custody is admitted first, each ordinary function is resolved, and this
//! file performs the sole final plan assembly and identity assignment.

mod alternative;
mod homes;
mod instruction;
mod ordinary;
mod roots;
#[cfg(test)]
mod tests;

use register_model::{
    TargetRegisterEnvironmentIdentity, ValidatedPhysicalRegisterModel,
    ValidatedRegisterConstraintCatalog,
};
use selected_instructions_to_register_homes::{
    ValidatedAllocationLegality, ValidatedLiveRanges, ValidatedPostAllocationOptimizationManifest,
    ValidatedRegisterHomes, ValidatedSelectedAnalysis,
};

use crate::PostAllocationMachineError;
use physical_instructions::{
    MachineAlternativeChoiceRule, PostAllocationMachineIdentity, PostAllocationMachinePlan,
    post_allocation_machine_identity,
};
use selected_instructions_to_register_homes::ValidatedPreAllocationMachineEffects;

/// The validated sources one plan construction reads, named once. Every layer
/// of the construction carries this owner instead of re-declaring the argument
/// tuple, and admission reads the same fields the assembly does.
pub(crate) struct PlanSources<'a, S: ValidatedSelectedAnalysis> {
    pub(crate) selected: &'a S,
    pub(crate) effects: &'a ValidatedPreAllocationMachineEffects,
    pub(crate) ranges: &'a ValidatedLiveRanges,
    pub(crate) legality: &'a ValidatedAllocationLegality,
    pub(crate) homes: &'a ValidatedRegisterHomes,
    pub(crate) manifest: &'a ValidatedPostAllocationOptimizationManifest,
    pub(crate) register_environment: TargetRegisterEnvironmentIdentity,
    pub(crate) physical: &'a ValidatedPhysicalRegisterModel,
    pub(crate) constraints: &'a ValidatedRegisterConstraintCatalog,
}

/// Join one validated selected CFG, its pre-allocation machine effects, and its
/// independently validated physical homes into the unsealed baseline plan.
/// Target applicability must identify exactly one legal alternative for every
/// instruction; optimizer-selected alternatives wait for the phase.
pub(crate) fn compute_post_allocation_machine_plan<S: ValidatedSelectedAnalysis>(
    sources: &PlanSources<'_, S>,
) -> Result<PostAllocationMachinePlan, PostAllocationMachineError> {
    roots::validate(sources)?;
    let selected_plan = sources.selected.selected_plan();
    let functions = ordinary::build_functions(
        selected_plan,
        sources.effects,
        sources.homes,
        sources.physical,
    )?;
    let mut plan = PostAllocationMachinePlan {
        identity: PostAllocationMachineIdentity::from_bytes([0; 32]),
        selected: sources.selected.selected_identity(),
        effects: sources.effects.receipt().identity(),
        ranges: sources.ranges.receipt().identity(),
        legality: sources.legality.receipt().identity(),
        homes: sources.homes.receipt().identity(),
        post_allocation_manifest: sources.manifest.record().identity,
        target: selected_plan.target,
        register_environment: sources.register_environment,
        physical_register_model: sources.physical.identity(),
        register_constraints: sources.effects.plan().register_constraints,
        machine_effect_catalog: sources.effects.plan().machine_effect_catalog,
        choice_rule: MachineAlternativeChoiceRule::UniqueApplicableInCatalogOrderV1,
        functions,
    };
    plan.identity = post_allocation_machine_identity(&plan);
    Ok(plan)
}
