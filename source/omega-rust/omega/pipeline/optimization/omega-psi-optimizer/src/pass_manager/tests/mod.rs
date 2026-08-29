//! Pass-manager tests organized by execution responsibility.
//!
//! Shared imports and fixtures enter here; leaves own fixed-point execution,
//! budget/invalidation fences, public entry points, and external replay.

use std::{collections::BTreeMap, sync::Arc};

use omega_abstract_operations::AbstractOperation;
use omega_optimization_core::{
    AnalysisSet, Optimization, OptimizationCandidateIdentity, OptimizationCandidateVerdict,
    OptimizationFactReference, OptimizationPassManifestRecord, OptimizationReasonCode,
    OptimizationRuleIdentity, OptimizationRuleSetIdentity, OptimizationSelections,
    OptimizationUnitIdentity, OptimizationWorkBudget,
};
use omega_optimization_policy::{
    BaselineDecisionOutcome, ExternalCandidateFeatures, ExternalDecisionAction,
    ExternalDecisionContext, ExternalDecisionLog, ExternalDecisionPoint,
    ExternalDecisionSchemaError, ValidatedCandidateSummary,
    external_psi_decision_schema_v2_identity, psi_target_neutral_decision_target_v2_identity,
};
use omega_optimization_unit::{PsiOptimizationUnit, PsiRewritePatch, PsiTransformationLedger};
use omega_optimization_validation::{
    OptimizationUnitValidationError, validate_psi_rewrite_candidate,
};
use omega_psi_to_abstract_operations::VerifiedPsiOptimizationUnit;

use super::*;
use crate::{
    AnalysisManager, AnalysisProduct, ExactIntegerAddConstantsRule,
    ExactIntegerSubtractConstantsRule, OrderedRuleRegistry, PsiOptimizationRule, RuleAnalysisView,
    RuleProposalError, built_in_psi_registries, built_in_psi_registry,
    rules::tests::{
        SelfDividePolicy, SelfRemainderPolicy, WrappingNeutralOperation, boolean_unit,
        compatible_policy_local_cse_unit, compatible_policy_phi_translated_gvn_unit,
        constant_conditional_same_target_unit, dead_exact_add_unit, dead_wrapping_add_unit,
        dependent_exact_chain_unit, diamond_dominator_gvn_unit, dominator_gvn_unit, exact_add_unit,
        linear_empty_block_unit, live_divide_by_one_unit, live_exact_multiply_by_zero_unit,
        live_exact_self_subtract_unit, live_exact_signed_negative_one_shift_right_unit,
        live_exact_zero_value_shift_unit, live_remainder_by_one_unit, live_self_divide_unit,
        live_self_remainder_unit, live_signed_remainder_by_negative_one_unit, local_cse_unit,
        non_adjacent_merge_unit, phi_translated_gvn_unit, proof_certified_dominator_gvn_unit,
        proof_certified_local_cse_unit, proof_certified_phi_translated_gvn_unit,
        propagated_block_parameter_unit, randomized_built_in_registries,
        redundant_block_parameter_unit, wrapping_add_unit, wrapping_neutral_identity_unit,
    },
};

mod budget_and_invalidation;
mod execution;
mod fixtures;
mod replay;
mod support;
mod synthetic_rules;

use fixtures::*;
use support::*;
use synthetic_rules::*;
