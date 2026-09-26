//! Optimizer module role: stage group. Pass-manager tests organized by execution responsibility.
//!
//! Shared imports and fixtures enter here; leaves own fixed-point execution,
//! budget/invalidation fences, public entry points, and external replay.

use std::collections::BTreeMap;
use std::sync::Arc;

use crate::rules::built_in_psi_registries;
use crate::rules::tests::fixtures::control_flow_cleanup::{
    constant_conditional_same_target_unit, linear_empty_block_unit, non_adjacent_merge_unit,
    propagated_block_parameter_unit, redundant_block_parameter_unit,
};
use crate::rules::tests::fixtures::dead_scalar_elimination::{
    dead_exact_add_unit, dead_scalar_literals_unit, dead_wrapping_add_unit,
};
use crate::rules::tests::fixtures::global_value_numbering::{
    WrappingNeutralOperation, compatible_policy_local_cse_unit,
    compatible_policy_phi_translated_gvn_unit, diamond_dominator_gvn_unit, dominator_gvn_unit,
    local_cse_unit, phi_translated_gvn_unit, proof_certified_dominator_gvn_unit,
    proof_certified_local_cse_unit, proof_certified_phi_translated_gvn_unit,
    wrapping_neutral_identity_unit,
};
use crate::rules::tests::fixtures::proof_check_elision::{
    SelfDividePolicy, SelfRemainderPolicy, dependent_exact_chain_unit, exact_add_unit,
    live_divide_by_one_unit, live_exact_add_zero_unit, live_exact_multiply_by_zero_unit,
    live_exact_self_subtract_unit, live_exact_signed_negative_one_shift_right_unit,
    live_exact_zero_value_shift_unit, live_remainder_by_one_unit, live_self_divide_unit,
    live_self_remainder_unit, live_signed_remainder_by_negative_one_unit, live_zero_dividend_unit,
};
use crate::rules::tests::fixtures::randomized_built_in_registries;
use crate::rules::tests::fixtures::sparse_conditional_constant_propagation::{
    boolean_unit, wrapping_add_unit,
};
use crate::rules::{ExactIntegerAddConstantsRule, ExactIntegerSubtractConstantsRule};
use crate::{
    AnalysisManager, AnalysisProduct, OrderedRuleRegistry, PsiOptimizationRule, RuleAnalysisView,
    RuleProposalError, built_in_psi_registry,
};
use abstract_operations::AbstractOperation;
use optimization_core::{
    AnalysisSet, BaselineDecisionOutcome, ExternalCandidateFeatures, ExternalDecisionAction,
    ExternalDecisionContext, ExternalDecisionLog, ExternalDecisionPoint,
    ExternalDecisionSchemaError, Optimization, OptimizationCandidateIdentity,
    OptimizationCandidateVerdict, OptimizationFactReference, OptimizationPassManifestRecord,
    OptimizationReasonCode, OptimizationRuleIdentity, OptimizationRuleSetIdentity,
    OptimizationSelections, OptimizationUnitIdentity, OptimizationWorkBudget,
    ValidatedCandidateSummary, external_psi_decision_schema_v2_identity,
    psi_target_neutral_decision_target_v2_identity,
};
use optimization_unit::{PsiOptimizationUnit, PsiRewritePatch, PsiTransformationLedger};
use optimization_unit_semantics::{
    OptimizationUnitValidationError, validate_psi_rewrite_candidate,
};
use terminal_psi_to_abstract_operations::VerifiedPsiOptimizationUnit;

mod budget_and_invalidation;
mod cycle_component_custody;
mod evidence_matrix;
mod execution;
mod fixtures;
mod replay;
mod support;
mod synthetic_rules;

use fixtures::*;
use support::*;
use synthetic_rules::*;
