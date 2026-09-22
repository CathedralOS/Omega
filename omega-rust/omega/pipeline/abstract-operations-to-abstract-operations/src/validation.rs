//! Optimizer module role: stage group. Abstract-optimization admission and
//! publication replay.
//!
//! Independent unit and rewrite meaning lives in optimization-unit-semantics.
//! These checks additionally consume the preceding stage's sealed Terminal
//! input. `context` admits verified and transformed units and their cycle
//! components; `projection` validates the plan a run projects and
//! `prephysical_manifest` the manifest published from it;
//! `scalar_case_frontiers` rewrites relocated scalar-case result frontiers.
//! The loop-invariant scalar-motion boundary, which both the proposal and the
//! relocation freeze replay consult so neither trusts a plan, is owned by
//! `member_blocks` (which member blocks and parameters are loop-invariant),
//! `invariant_operations` and `invariant_calls` (which operations may leave
//! the component), `place_observations` (whether what they observe is
//! invariant) and `relocation_rewrites` (the rewrites a relocation performs).

use abstract_operations::AbstractOperation as O;
use optimization_unit::*;
use optimization_unit_semantics::*;
use semantic_vocabulary::*;
use std::collections::{BTreeMap, BTreeSet};

mod context;
pub(crate) mod invariant_calls;
pub(crate) mod invariant_operations;
pub(crate) mod member_blocks;
pub(crate) mod place_observations;
mod prephysical_manifest;
mod projection;
pub(crate) mod relocation_rewrites;
mod scalar_case_frontiers;

pub(crate) use scalar_case_frontiers::{
    relocated_scalar_case_result_places, rewrite_relocated_case_result_frontiers,
};

pub use context::{
    ValidatedOptimizerCycleComponents, ValidatedOptimizerRankingCertificates,
    validate_psi_cycle_component_snapshot, validate_psi_ranking_certificate_snapshot,
    validate_transformed_psi_cycle_components, validate_transformed_psi_optimization_unit,
    validate_verified_psi_cycle_components, validate_verified_psi_optimization_unit,
};
pub use prephysical_manifest::{
    PrePhysicalOptimizationManifestError, ValidatedPrePhysicalOptimizationManifest,
    project_pre_physical_optimization_manifest, validate_pre_physical_optimization_manifest,
};
pub use projection::{
    OptimizedAbstractPlanProjectionError, ValidatedOptimizedAbstractPlanProjection,
    validate_optimized_abstract_plan_projection,
};
