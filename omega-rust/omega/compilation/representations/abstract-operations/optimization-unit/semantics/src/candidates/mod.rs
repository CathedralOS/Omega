//! Optimizer module role: stage group. Independent candidate acceptance, organized by the producing pass family.
//!
//! `validate_psi_rewrite_candidate` is the entry: it routes one typed
//! candidate to the exact validator of the pass family that proposed it, and
//! each family validator replays the rewrite independently and returns the
//! `ValidatedPsiRewrite` it reconstructed. Proof-check elision is recognized
//! by rule before patch shape, because its rewrites reuse the scalar patch
//! shapes of other families. Each family module owns its validator and
//! replay; `rewrite_accounting` reconstructs provenance and custody for all of
//! them, `observation` the closed scalar observation boundaries, and
//! `structural_bindings` the block-parameter bindings replays may forward
//! through.

use optimization_core::{OptimizationCandidateIdentity, OptimizationValidatorIdentity};
use optimization_unit::{
    ProvenanceRewrite, PsiOptimizationFunction, PsiOptimizationUnit, PsiRewriteCandidate,
    PsiRewritePatch,
};

use crate::OptimizationUnitValidationError;

pub(crate) mod case_membership_specialization;
pub(crate) mod control_flow_cleanup;
pub(crate) mod copy_propagation;
pub(crate) mod dead_scalar_elimination;
pub(crate) mod field_value_specialization;
pub(crate) mod global_value_numbering;
pub(crate) mod observation;
mod outside_region;
pub(crate) mod proof_check_elision;
pub(crate) mod rewrite_accounting;
pub(crate) mod sparse_conditional_constant_propagation;
pub(crate) mod state_specialization;
mod structural_bindings;

use case_membership_specialization::validate_case_membership_specialization_candidate;
use control_flow_cleanup::{
    validate_adjacent_block_merge_candidate, validate_constant_conditional_candidate,
    validate_linear_empty_block_candidate, validate_non_adjacent_block_merge_candidate,
    validate_path_qualified_empty_block_candidate, validate_shared_jump_fusion_candidate,
    validate_unreachable_private_machines_candidate,
};
use copy_propagation::validate_redundant_block_parameter_candidate;
use dead_scalar_elimination::validate_dead_scalar_node_candidate;
use field_value_specialization::validate_field_value_specialization_candidate;
use global_value_numbering::{
    validate_dominating_scalar_common_subexpression_candidate,
    validate_local_scalar_common_subexpression_candidate,
    validate_phi_translated_scalar_common_subexpression_candidate,
    validate_total_scalar_identity_candidate,
};
use proof_check_elision::{is_proof_check_elision_rule, validate_proof_check_elision_candidate};
use sparse_conditional_constant_propagation::validate_scalar_evaluation_candidate;
use state_specialization::validate_state_argument_specialization_candidate;

pub fn validate_psi_rewrite_candidate(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
    if is_proof_check_elision_rule(candidate.rule()) {
        return validate_proof_check_elision_candidate(input, candidate);
    }
    match candidate.patch() {
        PsiRewritePatch::ReplaceIntegerOperationWithConstant(_)
        | PsiRewritePatch::ReplaceBooleanOperationWithConstant(_) => {
            validate_scalar_evaluation_candidate(input, candidate)
        }
        PsiRewritePatch::RemoveRedundantBlockParameter(_) => {
            validate_redundant_block_parameter_candidate(input, candidate)
        }
        PsiRewritePatch::FoldConstantConditional(_) => {
            validate_constant_conditional_candidate(input, candidate)
        }
        PsiRewritePatch::ThreadLinearEmptyBlock(_) => {
            validate_linear_empty_block_candidate(input, candidate)
        }
        PsiRewritePatch::ThreadPathQualifiedEmptyBlock(_) => {
            validate_path_qualified_empty_block_candidate(input, candidate)
        }
        PsiRewritePatch::MergeAdjacentBlock(_) => {
            validate_adjacent_block_merge_candidate(input, candidate)
        }
        PsiRewritePatch::MergeNonAdjacentBlock(_) => {
            validate_non_adjacent_block_merge_candidate(input, candidate)
        }
        PsiRewritePatch::FuseSharedTerminalJump(_) => {
            validate_shared_jump_fusion_candidate(input, candidate)
        }
        PsiRewritePatch::RemoveDeadScalarNode(_) => {
            validate_dead_scalar_node_candidate(input, candidate)
        }
        PsiRewritePatch::EliminateLocalScalarCommonSubexpression(_) => {
            validate_local_scalar_common_subexpression_candidate(input, candidate)
        }
        PsiRewritePatch::EliminateDominatedScalarCommonSubexpression(_) => {
            validate_dominating_scalar_common_subexpression_candidate(input, candidate)
        }
        PsiRewritePatch::EliminatePhiTranslatedScalarCommonSubexpression(_) => {
            validate_phi_translated_scalar_common_subexpression_candidate(input, candidate)
        }
        PsiRewritePatch::EliminateProofCertifiedScalarIdentity(_) => {
            validate_proof_check_elision_candidate(input, candidate)
        }
        PsiRewritePatch::EliminateTotalScalarIdentity(_) => {
            validate_total_scalar_identity_candidate(input, candidate)
        }
        PsiRewritePatch::PruneUnreachablePrivateMachines(_) => {
            validate_unreachable_private_machines_candidate(input, candidate)
        }
        PsiRewritePatch::SpecializeStateArgument(_) => {
            validate_state_argument_specialization_candidate(input, candidate)
        }
        PsiRewritePatch::SpecializeCaseMembership(_) => {
            validate_case_membership_specialization_candidate(input, candidate)
        }
        PsiRewritePatch::SpecializeFieldValue(_) => {
            validate_field_value_specialization_candidate(input, candidate)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedPsiRewrite {
    pub(in crate::candidates) unit: PsiOptimizationUnit,
    pub(in crate::candidates) candidate: OptimizationCandidateIdentity,
    pub(in crate::candidates) validator: OptimizationValidatorIdentity,
    pub(in crate::candidates) provenance: Vec<ProvenanceRewrite>,
}

impl ValidatedPsiRewrite {
    pub const fn unit(&self) -> &PsiOptimizationUnit {
        &self.unit
    }

    pub const fn candidate(&self) -> OptimizationCandidateIdentity {
        self.candidate
    }

    pub const fn validator(&self) -> OptimizationValidatorIdentity {
        self.validator
    }

    /// Validator-accepted source disposition and fuel accounting. Consumers
    /// must ledger this value rather than re-reading the proposal.
    pub fn provenance(&self) -> &[ProvenanceRewrite] {
        &self.provenance
    }

    pub fn into_unit(self) -> PsiOptimizationUnit {
        self.unit
    }
}

/// The unit's function at `machine`, or CandidateLocationMissing.
pub(crate) fn candidate_function(
    input: &PsiOptimizationUnit,
    machine: semantic_vocabulary::MachineId,
) -> Result<&PsiOptimizationFunction, OptimizationUnitValidationError> {
    input
        .function(machine)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)
}

/// Rebuild a node's derived definitions, uses, successors, and ownership from
/// its operation; `node_index` is the node's position inside `block`.
pub(crate) fn rebuild_node_metadata(
    node: &mut optimization_unit::OptimizationNode,
    block: semantic_vocabulary::BlockId,
    node_index: usize,
) -> Result<(), OptimizationUnitValidationError> {
    let node_index = u32::try_from(node_index)
        .map_err(|_| OptimizationUnitValidationError::CandidateLocationMissing)?;
    node.definitions = crate::unit_validation::derived_metadata::expected_definitions(
        &node.operation,
        block,
        node_index,
    );
    node.uses =
        crate::unit_validation::derived_metadata::expected_uses(&node.operation, block, node_index);
    node.successors = rewrite_accounting::preserve_edge_custody(node);
    node.ownership = crate::unit_validation::derived_metadata::expected_ownership(&node.operation);
    Ok(())
}

/// Stamp `node` with the effect link at `effect` and return the next offset.
pub(crate) fn set_node_effect_link(
    node: &mut optimization_unit::OptimizationNode,
    effect: u64,
) -> Result<u64, OptimizationUnitValidationError> {
    let output = effect
        .checked_add(1)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    node.effect = optimization_unit::EffectLink {
        input: effect,
        output,
    };
    Ok(output)
}

/// The block's node at `node`, or CandidateLocationMissing.
pub(crate) fn candidate_node(
    block: &optimization_unit::OptimizationBlock,
    node: u32,
) -> Result<&optimization_unit::OptimizationNode, OptimizationUnitValidationError> {
    let index = usize::try_from(node)
        .map_err(|_| OptimizationUnitValidationError::CandidateLocationMissing)?;
    block
        .nodes
        .get(index)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)
}

/// The function's block at `block`, or CandidateLocationMissing.
pub(crate) fn candidate_block(
    function: &PsiOptimizationFunction,
    block: semantic_vocabulary::BlockId,
) -> Result<&optimization_unit::OptimizationBlock, OptimizationUnitValidationError> {
    function
        .block(block)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)
}
