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

use register_homes::AllocatedProgramFacts;
use register_model::{
    TargetRegisterEnvironmentIdentity, ValidatedPhysicalRegisterModel,
    ValidatedRegisterConstraintCatalog,
};

use crate::PostAllocationMachineError;
use physical_instructions::{
    MachineAlternativeChoiceRule, PostAllocationMachineIdentity, PostAllocationMachinePlan,
    post_allocation_machine_identity,
};

/// The sources one plan construction reads, named once. Every layer of the
/// construction carries this owner instead of re-declaring the argument tuple,
/// and admission reads the same fields the assembly does.
pub(crate) struct PlanSources<'a> {
    pub(crate) allocation: AllocatedProgramFacts<'a>,
    pub(crate) register_environment: TargetRegisterEnvironmentIdentity,
    pub(crate) physical: &'a ValidatedPhysicalRegisterModel,
    pub(crate) constraints: &'a ValidatedRegisterConstraintCatalog,
}

/// Join one validated selected CFG, its pre-allocation machine effects, and its
/// independently validated physical homes into the unsealed baseline plan.
/// Target applicability must identify exactly one legal alternative for every
/// instruction; optimizer-selected alternatives wait for the phase.
pub(crate) fn compute_post_allocation_machine_plan(
    sources: &PlanSources<'_>,
) -> Result<PostAllocationMachinePlan, PostAllocationMachineError> {
    roots::validate(sources)?;
    let allocation = sources.allocation;
    let selected_plan = allocation.selected.plan;
    let functions = ordinary::build_functions(
        selected_plan,
        allocation.effects,
        allocation.homes,
        sources.physical,
    )?;
    let mut plan = PostAllocationMachinePlan {
        identity: PostAllocationMachineIdentity::from_bytes([0; 32]),
        selected: allocation.selected.identity,
        effects: allocation.effects.identity,
        ranges: allocation.ranges_identity,
        legality: allocation.legality_identity,
        homes: allocation.homes_identity,
        post_allocation_manifest: allocation.manifest.identity,
        target: selected_plan.target,
        register_environment: sources.register_environment,
        physical_register_model: sources.physical.identity(),
        register_constraints: allocation.effects.register_constraints,
        machine_effect_catalog: allocation.effects.machine_effect_catalog,
        choice_rule: MachineAlternativeChoiceRule::UniqueApplicableInCatalogOrderV1,
        functions,
    };
    plan.identity = post_allocation_machine_identity(&plan);
    Ok(plan)
}
