//! Optimizer module role: proposal leaf. Constant-proof-derived exact specialization candidates.
//!
//! A dispatch state is eligible only in a machine the authenticated Terminal
//! component roster leaves acyclic — cyclic machines are frozen byte-exact
//! under the immutable-body custody rules, so no edge there may be retargeted.
//! The state argument must not already be a proven global constant: when every
//! incoming edge agrees on one value the dispatch belongs to constant
//! conditional folding, not this family. An incoming edge qualifies when its
//! bound argument is proven constant either by the sparse lattice directly
//! or — the result specialization — by resolving through single-predecessor
//! forwarding-block parameters to an in-function `Call` result whose
//! single-return callee's lattice proves that result constant.

use super::{
    AnalysisProduct, BlockId, DispatchSpecializationPlan, PsiOptimizationFunction,
    PsiOptimizationUnit, ScalarConstantAnalysis, StateArgumentSpecializationCandidate,
    StateArgumentSpecializationError, VerifiedPsiOptimizationSession, admission, apply,
    candidate_identity, compute_analysis,
};
use optimization_core::AnalysisKind;
use std::collections::BTreeSet;

pub(super) fn all(
    session: &VerifiedPsiOptimizationSession,
    candidate_limit: u64,
) -> Result<Vec<StateArgumentSpecializationCandidate>, StateArgumentSpecializationError> {
    let unit = session.unit();
    let Some(AnalysisProduct::ScalarConstants(constants)) =
        compute_analysis(unit, AnalysisKind::ScalarConstants)
    else {
        return Ok(Vec::new());
    };
    // Machines holding an authenticated cyclic component are frozen byte-exact
    // for this family; their entries/exits and internal edges cannot move.
    let frozen = session
        .cycle_components()
        .components()
        .iter()
        .map(|component| component.id.machine)
        .collect::<BTreeSet<_>>();
    let mut candidates = Vec::new();
    for function in &unit.functions {
        if frozen.contains(&function.machine) {
            continue;
        }
        for block in &function.blocks {
            let Some(plan) = plan(unit, function, block.id, &constants) else {
                continue;
            };
            if plan.edges.is_empty() {
                continue;
            }
            candidates.push(from_plan(unit, plan)?);
        }
    }
    let required = u64::try_from(candidates.len())
        .map_err(|_| StateArgumentSpecializationError::CoordinateOverflow)?;
    if required > candidate_limit {
        return Err(StateArgumentSpecializationError::CandidateBudgetExhausted {
            required,
            limit: candidate_limit,
        });
    }
    Ok(candidates)
}

/// Independently derived specialization plan for one block, or `None` when the
/// block is not an eligible dispatch state. An admissible plan carries at most
/// the constant-supplied incoming edges — unconditional `Jump` successors and
/// `Conditional` predecessor arms; when every incoming edge qualifies, fusing
/// them all would orphan the dispatch state, so the plan is reported with no
/// edges.
pub(crate) fn plan(
    unit: &PsiOptimizationUnit,
    function: &PsiOptimizationFunction,
    dispatch: BlockId,
    constants: &ScalarConstantAnalysis,
) -> Option<DispatchSpecializationPlan> {
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
    edges.sort_by_key(|row| row.incoming_edge());
    // Specializing every incoming edge would leave the dispatch state
    // unreachable; at least one unfused edge must remain.
    if edges.len() == incoming.len() {
        edges.clear();
    }
    Some(DispatchSpecializationPlan {
        machine: evidence.machine,
        dispatch,
        edges,
    })
}

pub(super) fn from_plan(
    unit: &PsiOptimizationUnit,
    plan: DispatchSpecializationPlan,
) -> Result<StateArgumentSpecializationCandidate, StateArgumentSpecializationError> {
    let output = apply::realize(unit, &plan)?;
    let identity = candidate_identity(
        unit.identity,
        output.identity,
        plan.machine,
        plan.dispatch,
        &plan.edges,
    );
    Ok(StateArgumentSpecializationCandidate {
        identity,
        input: unit.identity,
        output: output.identity,
        machine: plan.machine,
        dispatch: plan.dispatch,
        specializations: plan.edges,
    })
}
