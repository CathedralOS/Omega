//! Optimizer module role: stage group. Independent validation of complete Psi optimization units and retained context.

use super::*;

mod context;
mod core;
mod derived_metadata;
mod function_structure;
mod operation_contracts;
mod services;
mod structural_catalog;

pub use context::{
    CycleComponentEdge, CycleComponentId, OptimizerCycleComponent, OptimizerCycleComponentSnapshot,
    ValidatedOptimizerCycleComponents, validate_psi_cycle_component_snapshot,
    validate_transformed_psi_cycle_components, validate_transformed_psi_optimization_unit,
    validate_verified_psi_cycle_components, validate_verified_psi_optimization_unit,
};
pub use core::validate_psi_optimization_unit;

#[cfg(test)]
pub(crate) use core::{valid_edge_affine_transition, valid_hidden_affine_establishment};
pub(crate) use derived_metadata::*;
pub(crate) use function_structure::*;
pub(crate) use operation_contracts::*;
pub(crate) use services::*;
pub(crate) use structural_catalog::*;
