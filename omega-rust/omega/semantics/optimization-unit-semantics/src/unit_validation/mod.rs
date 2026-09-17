//! Optimizer module role: stage group. Independent validation of complete Psi optimization units and retained context.

mod core;
mod derived_metadata;
mod function_structure;
mod operation_contracts;
mod references;
mod services;
mod structural_catalog;

pub use core::{
    validate_psi_optimization_unit, validate_psi_optimization_unit_with_admitted_cycle_machines,
};

#[cfg(test)]
pub(crate) use core::{valid_edge_affine_transition, valid_hidden_affine_establishment};
pub(crate) use derived_metadata::*;
pub(crate) use function_structure::*;
pub(crate) use operation_contracts::*;
pub(crate) use references::*;
pub(crate) use services::*;
pub(crate) use structural_catalog::*;
