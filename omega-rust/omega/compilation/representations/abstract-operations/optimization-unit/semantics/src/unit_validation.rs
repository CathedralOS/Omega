//! Optimizer module role: executable entrance. Independent validation of one
//! complete Psi optimization unit and its retained context.
//!
//! `validate_psi_optimization_unit` is the entry; acceptance proceeds in the
//! order `validate_psi_optimization_unit_with_control_cycles` shows: the
//! canonical identity and accepted fact indexes (`identity_indexes`), the
//! machine rosters and structural/service catalogs with every function
//! validated in place (`catalogs`, which drives `function_structure`), the
//! retained edge-cleanup and hidden-establishment affine authority
//! (`affine_authority`), then the final frontier, entry and service
//! authorities (`catalogs` again). The remaining modules are the invariant
//! families those steps descend into: `structural_catalog` indexes types,
//! domains and provider specializations; `services` the service catalog and
//! root reach; `references` the reference-carrier paths; `derived_metadata`
//! the places, claims, dominance, edges and provenance a function must
//! reproduce; `operation_contracts` the per-node value and binding
//! contracts. Callers retain Terminal admission and supply any independently
//! admitted cycle roster; structural success alone grants no execution or
//! publication authority.

use crate::OptimizationUnitValidationError;
use optimization_unit::{
    PsiOptimizationFunction, PsiOptimizationUnit, recompute_psi_optimization_unit_identity,
};
use semantic_vocabulary::MachineId;

mod activation_storage;
pub(crate) mod affine_authority;
pub(crate) mod arrival_storage;

/// The catalog-mismatch rejection for `function`, named once per check site.
pub(crate) fn catalog_mismatch(
    function: &PsiOptimizationFunction,
) -> OptimizationUnitValidationError {
    OptimizationUnitValidationError::StructuralCatalogMismatch {
        machine: Some(function.machine),
    }
}
mod catalogs;
pub(crate) mod derived_metadata;
pub(crate) mod function_structure;
mod identity_indexes;
pub(crate) mod operation_contracts;
pub(crate) mod references;
mod runtime_evidence;
pub(crate) mod services;
pub(crate) mod structural_catalog;

pub(crate) fn validate_psi_optimization_unit(
    unit: &PsiOptimizationUnit,
) -> Result<(), OptimizationUnitValidationError> {
    validate_psi_optimization_unit_with_control_cycles(
        unit,
        &function_structure::ControlCyclePolicy::default(),
    )
}

/// The output-tail every rewrite candidate replays after mutating a function:
/// rebuild the function's derived fact index, declared places and root
/// service reach, refresh the unit identity, re-validate the whole unit, and
/// hand the rebuilt function back for the caller's remaining checks. Every
/// surface here is derived metadata — the rewrite re-derives it from the
/// operations it landed, exactly as unit validation requires, instead of
/// trusting a stale copy from before the mutation.
pub(crate) fn revalidate_rewrite_output(
    output: &mut PsiOptimizationUnit,
    machine: MachineId,
) -> Result<&PsiOptimizationFunction, OptimizationUnitValidationError> {
    let output_function = output
        .function_mut(machine)
        .expect("output function exists");
    output_function.facts = function_structure::reconstruct_fact_index(output_function);
    output_function.declared_places =
        derived_metadata::reconstruct_declared_places(output_function)?;
    services::refresh_root_service_reach(output)?;
    output.identity = recompute_psi_optimization_unit_identity(output);
    validate_psi_optimization_unit(output)?;
    Ok(output.function(machine).expect("output function exists"))
}

/// Check unit structure after a caller has independently admitted the named
/// machines' control cycles. This checks no ranking evidence and grants no
/// cycle-admission authority; the stage must retain and replay that evidence.
pub fn validate_psi_optimization_unit_with_admitted_cycle_machines(
    unit: &PsiOptimizationUnit,
    admitted_machines: &[MachineId],
) -> Result<(), OptimizationUnitValidationError> {
    let mut policy = function_structure::ControlCyclePolicy::default();
    for &machine in admitted_machines {
        policy.admit(machine);
    }
    validate_psi_optimization_unit_with_control_cycles(unit, &policy)
}

/// Reconstruct the current ownership frontier from the executable optimization
/// unit itself. The returned facts carry no verifier snapshot or side channel.
pub fn reconstruct_ownership_frontier_facts_with_admitted_cycle_machines(
    unit: &PsiOptimizationUnit,
    admitted_machines: &[MachineId],
) -> Result<Vec<optimization_unit::OwnershipFrontierFact>, OptimizationUnitValidationError> {
    identity_indexes::validate_identity_and_fact_indexes(unit)?;
    let mut policy = function_structure::ControlCyclePolicy::default();
    for &machine in admitted_machines {
        policy.admit(machine);
    }
    let mut facts = catalogs::index_and_validate_unit_catalogs(unit, &policy)?.ownership_frontiers;
    facts.sort_by_key(|fact| (fact.machine, fact.site));
    Ok(facts)
}

pub(crate) fn validate_psi_optimization_unit_with_control_cycles(
    unit: &PsiOptimizationUnit,
    cycle_policy: &function_structure::ControlCyclePolicy,
) -> Result<(), OptimizationUnitValidationError> {
    identity_indexes::validate_identity_and_fact_indexes(unit)?;
    let indexes = catalogs::index_and_validate_unit_catalogs(unit, cycle_policy)?;
    runtime_evidence::validate(unit)?;
    affine_authority::validate_retained_ownership_authority(unit)?;
    catalogs::validate_final_authorities(unit, &indexes)
}
