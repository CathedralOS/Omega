//! Optimizer module role: stage group. Independent CFG rewrite validation by exact graph transformation.

use crate::OptimizationUnitValidationError;
use abstract_operations::{AbstractOperation as O, ValueBinding};
use optimization_unit::{
    OptimizationNode, PsiOptimizationFunction, PsiRewriteCandidate, ScalarSubstitution,
    ValueDefinition,
};
use semantic_vocabulary::{BlockId, EdgeId};

mod block_merging;
mod constant_conditionals;
mod empty_block_threading;
mod shared_jump_fusion;
mod unreachable_private_machines;

pub(crate) use block_merging::{
    validate_adjacent_block_merge_candidate, validate_non_adjacent_block_merge_candidate,
};
pub(crate) use constant_conditionals::validate_constant_conditional_candidate;
pub(crate) use empty_block_threading::{
    validate_linear_empty_block_candidate, validate_path_qualified_empty_block_candidate,
};
pub(crate) use shared_jump_fusion::validate_shared_jump_fusion_candidate;
pub(crate) use unreachable_private_machines::validate_unreachable_private_machines_candidate;

/// The outgoing value bindings of the jump at `node` when that jump realizes
/// the patch exactly: the row carries no cleanup actions and names the patched
/// edge and target. Every family's replay admits its terminator through this
/// one rule, so a jump that also cleans up or leaves toward another edge can
/// never be read as the patch's own exit.
pub(super) fn patched_jump_bindings<'node>(
    node: &'node OptimizationNode,
    edge: EdgeId,
    target: BlockId,
) -> Result<&'node [ValueBinding], OptimizationUnitValidationError> {
    let O::Jump {
        psi_edge,
        target: jump_target,
        bindings,
        cleanup_actions,
        ..
    } = &node.operation
    else {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    };
    if !cleanup_actions.is_empty() || *psi_edge != edge || *jump_target != target {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    Ok(bindings)
}

/// These rewrite families substitute scalar bindings only: a function whose
/// blocks declare structural parameters is outside all of them, so its
/// candidate can never be replayed as one.
pub(super) fn require_scalar_bindings(
    function: &PsiOptimizationFunction,
) -> Result<(), OptimizationUnitValidationError> {
    if function
        .blocks
        .iter()
        .any(|block| !block.structural_parameters.is_empty())
    {
        return Err(OptimizationUnitValidationError::CandidateIncomingBindingMismatch);
    }
    Ok(())
}

/// The candidate's scalar substitution roster for one jump: every destination
/// parameter row must consume its own binding, `admits` carries the rule's own
/// obligation on that pair, and the candidate's claim must match the
/// canonically ordered roster exactly.
pub(super) fn checked_scalar_substitutions<'bindings>(
    candidate: &PsiRewriteCandidate,
    parameters: &[ValueDefinition],
    bindings: &'bindings [ValueBinding],
    admits: impl Fn(&ValueDefinition, &'bindings ValueBinding) -> bool,
) -> Result<Vec<ScalarSubstitution>, OptimizationUnitValidationError> {
    if parameters.len() != bindings.len() {
        return Err(OptimizationUnitValidationError::CandidateIncomingBindingMismatch);
    }
    let mut substitutions = parameters
        .iter()
        .zip(bindings)
        .map(|(parameter, binding)| {
            (binding.parameter == parameter.value
                && binding.scalar_type == parameter.scalar_type
                && admits(parameter, binding))
            .then_some(ScalarSubstitution {
                from: parameter.value,
                to: binding.argument,
                scalar_type: parameter.scalar_type,
            })
        })
        .collect::<Option<Vec<_>>>()
        .ok_or(OptimizationUnitValidationError::CandidateIncomingBindingMismatch)?;
    substitutions.sort();
    if candidate.substitutions() != substitutions {
        return Err(OptimizationUnitValidationError::CandidateSubstitutionMismatch);
    }
    Ok(substitutions)
}
