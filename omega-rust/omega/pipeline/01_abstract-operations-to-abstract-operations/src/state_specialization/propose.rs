//! Optimizer module role: proposal leaf. Constant-proof-derived exact specialization plans.
//!
//! A dispatch state is eligible only in a machine the authenticated Terminal
//! component roster leaves acyclic — cyclic machines are frozen byte-exact
//! under the immutable-body custody rules, so no edge there may be retargeted;
//! the rule applies that freeze before asking for a plan. The state argument
//! must not already be a proven global constant: when every incoming edge
//! agrees on one value the dispatch belongs to constant conditional folding,
//! not this family. An incoming edge qualifies when its bound argument is
//! proven constant either by the sparse lattice directly or — the result
//! specialization — by resolving through single-predecessor forwarding-block
//! parameters to an in-function `Call` result whose single-return callee's
//! lattice proves that result constant.

use super::{
    BlockId, PsiOptimizationFunction, PsiOptimizationUnit, ScalarConstantAnalysis,
    StateArgumentSpecializationRewrite, admission,
};

/// Independently derived specialization plan for one block, or `None` when the
/// block is not an eligible dispatch state or nothing enters it. An admissible
/// plan carries at most the constant-supplied incoming edges — unconditional
/// `Jump` successors and `Conditional` predecessor arms — sorted by edge
/// identity; when every incoming edge qualifies, fusing them all would orphan
/// the dispatch state, so the plan is reported with no edges.
pub(crate) fn plan(
    unit: &PsiOptimizationUnit,
    function: &PsiOptimizationFunction,
    dispatch: BlockId,
    constants: &ScalarConstantAnalysis,
) -> Option<StateArgumentSpecializationRewrite> {
    let evidence = admission::dispatch_evidence(unit, function, dispatch, constants)?;
    let incoming = admission::incoming_edges(function, dispatch);
    if incoming.is_empty() {
        return None;
    }
    let mut edges: Vec<_> = incoming
        .iter()
        .filter_map(|(owner_block, node_index, owner_node, edge)| {
            admission::admit_incoming_edge(
                &evidence,
                *owner_block,
                *node_index,
                owner_node,
                edge,
                constants,
            )
        })
        .collect();
    edges.sort_by_key(|row| row.incoming_edge);
    // Specializing every incoming edge would leave the dispatch state
    // unreachable; at least one unfused edge must remain.
    if edges.len() == incoming.len() {
        edges.clear();
    }
    Some(StateArgumentSpecializationRewrite {
        machine: evidence.machine,
        dispatch,
        edges,
    })
}
