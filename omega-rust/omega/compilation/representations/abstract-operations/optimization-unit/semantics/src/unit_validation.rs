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
use optimization_unit::PsiOptimizationUnit;
use semantic_vocabulary::MachineId;

pub(crate) mod affine_authority;
mod catalogs;
pub(crate) mod derived_metadata;
pub(crate) mod function_structure;
mod identity_indexes;
pub(crate) mod operation_contracts;
pub(crate) mod references;
pub(crate) mod services;
pub(crate) mod structural_catalog;

pub fn validate_psi_optimization_unit(
    unit: &PsiOptimizationUnit,
) -> Result<(), OptimizationUnitValidationError> {
    validate_psi_optimization_unit_with_control_cycles(
        unit,
        &function_structure::ControlCyclePolicy::default(),
    )
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

pub(crate) fn validate_psi_optimization_unit_with_control_cycles(
    unit: &PsiOptimizationUnit,
    cycle_policy: &function_structure::ControlCyclePolicy,
) -> Result<(), OptimizationUnitValidationError> {
    identity_indexes::validate_identity_and_fact_indexes(unit)?;
    let indexes = catalogs::index_and_validate_unit_catalogs(unit, cycle_policy)?;
    affine_authority::validate_retained_ownership_authority(unit)?;
    catalogs::validate_final_authorities(unit, &indexes)
}
