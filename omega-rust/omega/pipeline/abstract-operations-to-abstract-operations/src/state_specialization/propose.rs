//! Optimizer module role: proposal leaf. Constant-fact-derived exact specialization candidates.
//!
//! A dispatch state is eligible only in a machine the authenticated Terminal
//! component roster leaves acyclic — cyclic machines are frozen byte-exact
//! under the immutable-body custody rules, so no edge there may be retargeted.
//! The state argument must not already be a proven global constant: when every
//! incoming edge agrees on one value the dispatch belongs to constant
//! conditional folding, not this family.

use super::{
    AnalysisProduct, BlockId, DispatchSpecializationPlan, EdgeId, NodeLocation, O,
    PsiOptimizationFunction, PsiOptimizationUnit, ScalarConstant, ScalarConstantAnalysis,
    SpecializedStateEdge, StateArgumentSpecializationCandidate, StateArgumentSpecializationError,
    VerifiedPsiOptimizationSession, apply, candidate_identity, compute_analysis,
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
/// the constant-supplied unconditional incoming edges; when every incoming
/// edge qualifies, fusing them all would orphan the dispatch state, so the
/// plan is reported with no edges.
pub(crate) fn plan(
    unit: &PsiOptimizationUnit,
    function: &PsiOptimizationFunction,
    dispatch: BlockId,
    constants: &ScalarConstantAnalysis,
) -> Option<DispatchSpecializationPlan> {
    let machine = function.machine;
    let block = function.blocks.iter().find(|block| block.id == dispatch)?;
    if block.id == function.entry || !block.structural_parameters.is_empty() {
        return None;
    }
    let [node] = block.nodes.as_slice() else {
        return None;
    };
    let O::Conditional {
        condition,
        when_true,
        when_false,
    } = &node.operation
    else {
        return None;
    };
    // The dispatch reads a state argument directly: the condition must be one
    // of this block's own scalar parameters.
    let parameter = block
        .parameters
        .iter()
        .find(|parameter| parameter.value == *condition)?;
    // A globally constant state argument is constant-conditional-fold custody,
    // not specialization. This family owns the edge-exact case where the
    // function-wide lattice cannot prove the parameter.
    if constants.facts.iter().any(|fact| {
        fact.valid_in.machine == machine
            && fact.valid_in.revision == unit.identity
            && fact.value == parameter.value
            && matches!(fact.constant, ScalarConstant::Boolean(_))
    }) {
        return None;
    }
    let arm_edge = |edge: EdgeId| {
        node.successors
            .iter()
            .find(|successor| successor.psi_edge == edge)
    };
    let when_true_edge = arm_edge(when_true.psi_edge)?;
    let when_false_edge = arm_edge(when_false.psi_edge)?;

    // Every edge entering the dispatch state, with its owner coordinates.
    let mut incoming = Vec::new();
    for owner_block in &function.blocks {
        for (node_index, owner_node) in owner_block.nodes.iter().enumerate() {
            for edge in &owner_node.successors {
                if edge.target == dispatch {
                    incoming.push((owner_block.id, node_index, owner_node, edge));
                }
            }
        }
    }
    if incoming.is_empty() {
        return None;
    }

    let mut edges = Vec::new();
    for (owner_block, node_index, owner_node, edge) in &incoming {
        let predecessor = NodeLocation {
            machine,
            block: *owner_block,
            node: u32::try_from(*node_index).ok()?,
        };
        // Only an unconditional traversal may fuse: the owner must be a Jump
        // whose single successor is this exact edge, and no affine or
        // structural custody may ride either side of the fused traversal.
        let O::Jump {
            psi_edge,
            target,
            structural_bindings,
            trivial_affine_discards,
            residual_affine_discards,
            ..
        } = &owner_node.operation
        else {
            continue;
        };
        if *psi_edge != edge.psi_edge
            || *target != dispatch
            || !structural_bindings.is_empty()
            || !trivial_affine_discards.is_empty()
            || !residual_affine_discards.is_empty()
            || !edge.structural_bindings.is_empty()
            || !edge.trivial_affine_discards.is_empty()
            || !edge.residual_affine_discards.is_empty()
        {
            continue;
        }
        let Some(binding) = edge
            .bindings
            .iter()
            .find(|binding| binding.parameter == parameter.value)
        else {
            continue;
        };
        let Some(fact) = constants.facts.iter().find(|fact| {
            fact.valid_in.machine == machine
                && fact.valid_in.revision == unit.identity
                && fact.value == binding.argument
        }) else {
            continue;
        };
        let ScalarConstant::Boolean(constant) = fact.constant else {
            continue;
        };
        let (resolved, rejected) = if constant {
            (when_true_edge, when_false_edge)
        } else {
            (when_false_edge, when_true_edge)
        };
        if !resolved.structural_bindings.is_empty()
            || !resolved.trivial_affine_discards.is_empty()
            || !resolved.residual_affine_discards.is_empty()
        {
            continue;
        }
        let Some(resolved_block) = function
            .blocks
            .iter()
            .find(|candidate| candidate.id == resolved.target)
        else {
            continue;
        };
        if !resolved_block.structural_parameters.is_empty() {
            continue;
        }
        edges.push(SpecializedStateEdge {
            incoming_edge: edge.psi_edge,
            predecessor,
            parameter: parameter.value,
            argument: binding.argument,
            constant,
            taken_edge: resolved.psi_edge,
            rejected_edge: rejected.psi_edge,
            resolved_target: resolved.target,
        });
    }
    edges.sort_by_key(|row| row.incoming_edge);
    // Specializing every incoming edge would leave the dispatch state
    // unreachable; at least one unfused edge must remain.
    if edges.len() == incoming.len() {
        edges.clear();
    }
    Some(DispatchSpecializationPlan {
        machine,
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
