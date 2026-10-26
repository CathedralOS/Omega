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

pub(crate) use scalar_case_frontiers::rewrite_relocated_case_result_frontiers;

pub(crate) use context::{
    ValidatedOptimizerCycleComponents, ValidatedOptimizerRankingCertificates,
    validate_transformed_psi_context, validate_verified_psi_context,
};
pub use prephysical_manifest::ValidatedPrePhysicalOptimizationManifest;
pub(crate) use prephysical_manifest::{
    PrePhysicalOptimizationManifestError, project_pre_physical_optimization_manifest,
};
pub(crate) use projection::{
    ValidatedOptimizedAbstractPlanProjection, seal_optimized_abstract_plan_projection,
};
