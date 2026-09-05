//! Optimizer module role: stage group.
use abstract_operations::AbstractOperation as O;
use abstract_operations::{
    AbstractBlockEntry, AbstractFunction, AbstractFunctionResult, AbstractOperation,
    AbstractOperationPlan, AbstractParameter, AbstractResult, AbstractSuccessor, ValueBinding,
};
use optimization::{PsiOptimization, PsiOptimizationSelections};
use optimization_core::{
    AnalysisKind, Optimization, OptimizationFactReference, OptimizationPassIdentity,
    OptimizationRuleContract, OptimizationRuleIdentity, OptimizationSafetyClass,
    OptimizationSelections, OptimizationValidatorIdentity, ScalarConstantFactIdentity,
};
use optimization_unit::{
    AcceptedObligationFact, BooleanConstantRewrite, ConstantConditionalRewrite,
    DominatingScalarCommonSubexpressionRewrite, IntegerConstantRewrite, IntegerEvaluationWitness,
    NodeLocation, OptimizationFact, OptimizationNode, OwnershipFrontierFact, OwnershipFrontierSite,
    OwnershipFrontierSnapshot, OwnershipFrontierWitness, ProofCertifiedScalarIdentityKind,
    ProofCertifiedScalarIdentityRewrite, ProofQuestion, ProofQuestionClass, ProofQuestionOwner,
    ProvenanceDisposition, ProvenanceRewrite, PrunedMachineCustody, PsiOptimizationUnit,
    PsiProvenance, PsiRealizationSite, PsiRewriteCandidate, PsiRewriteCandidateError,
    PsiRewritePatch, RedundantBlockParameterWitness, ScalarSubstitution, TotalScalarIdentityKind,
    attach_accepted_obligation_facts, attach_proof_questions,
    recompute_psi_optimization_unit_identity, reconstruct_psi_optimization_unit_seed,
};
use optimization_validation::{
    OptimizationUnitValidationError, validate_adjacent_block_merge_candidate,
    validate_boolean_evaluation_candidate, validate_constant_conditional_candidate,
    validate_dead_scalar_node_candidate, validate_dominating_scalar_common_subexpression_candidate,
    validate_integer_evaluation_candidate, validate_linear_empty_block_candidate,
    validate_local_scalar_common_subexpression_candidate,
    validate_non_adjacent_block_merge_candidate, validate_path_qualified_empty_block_candidate,
    validate_phi_translated_scalar_common_subexpression_candidate,
    validate_proof_certified_exact_integer_self_subtract_candidate,
    validate_proof_certified_integer_remainder_by_one_candidate,
    validate_proof_certified_integer_self_divide_candidate,
    validate_proof_certified_integer_self_remainder_candidate,
    validate_proof_certified_scalar_identity_candidate,
    validate_proof_certified_signed_integer_remainder_by_negative_one_candidate,
    validate_psi_optimization_unit, validate_redundant_block_parameter_candidate,
    validate_shared_jump_fusion_candidate, validate_total_scalar_identity_candidate,
    validate_unreachable_private_machines_candidate,
};
use semantic_vocabulary::{
    BlockId, BoundaryMachineId, EdgeId, FuelScheduleIdentity, IntegerSign, IntegerType,
    IntegerValue, MachineId, ObligationId, OperationId, PlaceId, ScalarType, ServiceId,
    StructuralTypeId, ValueId,
};
use terminal_psi::{
    SemanticFingerprint, ServiceDeclaration, TerminalPsiIdentity, VocabularyMarker,
};

use super::control_flow_cleanup::rule_unreachable_private_machine_complement;
use super::global_value_numbering::{
    compatible_policy_scalar_leader, compatible_policy_scalar_redundant,
    proof_certified_scalar_expression,
};
use super::proof_check_elision::{integer_one, integer_zero};
use super::*;
use crate::rules::catalog::{
    BuiltInRuleRegistration, assemble_built_in_registry, built_in_rule_registrations,
    registry_for_optimization,
};
use crate::rules::{
    ORDERED_PSI_PASSES, PSI_PASS_CATALOG, built_in_psi_registries, built_in_psi_registry,
    built_in_psi_registry_for_selections,
};
use crate::{
    AnalysisProduct, OrderedRuleRegistry, PsiOptimizationRule, RuleAnalysisView, RuleProposalError,
    RuleRegistryError, compute_analysis,
};

mod fixtures;

pub(crate) use super::{IntegerRangeComparisonKind, IntegerRangePairComparisonKind};
pub(crate) use fixtures::*;

mod catalog;
mod control_flow_cleanup;
mod copy_propagation;
mod dead_scalar_elimination;
mod global_value_numbering;
mod proof_check_elision;
mod sparse_conditional_constant_propagation;
