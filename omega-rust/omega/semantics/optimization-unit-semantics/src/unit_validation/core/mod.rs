//! Optimizer module role: executable entrance. Reconstructible optimization-unit validation coordination.
//!
//! Acceptance proceeds through canonical identity/fact indexes, unit catalogs,
//! retained affine authority, and final frontier/entry/service checks.

use super::*;

mod affine_authority;
mod catalogs;
mod identity_indexes;

#[cfg(test)]
pub(crate) use affine_authority::{
    valid_edge_affine_transition, valid_hidden_affine_establishment,
};

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
