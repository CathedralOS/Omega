//! Reconstructible optimization-unit validation coordination.
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
    identity_indexes::validate_identity_and_fact_indexes(unit)?;
    let indexes = catalogs::index_and_validate_unit_catalogs(unit)?;
    affine_authority::validate_retained_ownership_authority(unit)?;
    catalogs::validate_final_authorities(unit, &indexes)
}
