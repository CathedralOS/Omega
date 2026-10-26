//! Optimizer module role: stage group. Independent CFG rewrite validation by exact graph transformation.

use crate::OptimizationUnitValidationError;
use abstract_operations::{AbstractOperation as O, ValueBinding};
use optimization_unit::OptimizationNode;
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
