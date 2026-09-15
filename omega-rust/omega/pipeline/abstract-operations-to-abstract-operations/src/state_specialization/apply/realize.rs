//! Optimizer module role: application leaf. Exact edge fusion and derived-coordinate refresh.
//!
//! For each admitted specialization row, the unconditional incoming edge is
//! retargeted to the resolved dispatch arm's target. The fused edge keeps its
//! own Psi edge identity first in provenance — the shape every edge must keep
//! — and appends the resolved arm edge's complete custody, so one taken
//! traversal charges both sources exactly once. Scalar bindings compose
//! through the incoming edge: any resolved-arm argument naming a dispatch
//! parameter maps to the argument this edge bound to that parameter, and any
//! other argument keeps its value — such a value is either a function
//! parameter or defined in a block dominating the dispatch state, and that
//! block also dominates this edge's owner, so the use stays def-ordered.

use super::super::{
    DispatchSpecializationPlan, O, OptimizationEdge, PsiOptimizationUnit, PsiProvenance,
    SpecializedStateEdge, StateArgumentSpecializationError, ValueBinding, ValueUse,
    recompute_psi_optimization_unit_identity,
};
use optimization_unit::{OptimizationNode, PsiOptimizationFunction};

/// The fused node shape a specialization admits at its predecessor site: the
/// retargeted `Jump`, its single fused successor edge, and the derived use
/// roster. Application and the independent custody walk share this
/// construction so both name exactly the same admitted difference.
pub(crate) fn fused_node(
    row: &SpecializedStateEdge,
    dispatch: semantic_vocabulary::BlockId,
    function: &PsiOptimizationFunction,
) -> Result<OptimizationNode, StateArgumentSpecializationError> {
    let predecessor = node_at(function, row)?;
    let O::Jump {
        psi_edge,
        target,
        bindings: _,
        structural_bindings,
        trivial_affine_discards,
        residual_affine_discards,
    } = &predecessor.operation
    else {
        return Err(StateArgumentSpecializationError::CandidateMismatch);
    };
    if row.predecessor.machine != function.machine
        || *psi_edge != row.incoming_edge
        || *target != dispatch
        || !structural_bindings.is_empty()
        || !trivial_affine_discards.is_empty()
        || !residual_affine_discards.is_empty()
    {
        return Err(StateArgumentSpecializationError::CandidateMismatch);
    }
    let incoming = predecessor
        .successors
        .iter()
        .find(|edge| edge.psi_edge == row.incoming_edge)
        .ok_or(StateArgumentSpecializationError::CandidateMismatch)?;
    let dispatch = function
        .blocks
        .iter()
        .find(|block| block.id == dispatch)
        .ok_or(StateArgumentSpecializationError::MissingSite {
            machine: function.machine,
            block: dispatch,
            node: 0,
        })?;
    let [dispatch_node] = dispatch.nodes.as_slice() else {
        return Err(StateArgumentSpecializationError::CandidateMismatch);
    };
    let resolved = dispatch_node
        .successors
        .iter()
        .find(|edge| edge.psi_edge == row.taken_edge)
        .ok_or(StateArgumentSpecializationError::CandidateMismatch)?;
    if resolved.target != row.resolved_target
        || row.rejected_edge == row.taken_edge
        || !dispatch_node
            .successors
            .iter()
            .any(|edge| edge.psi_edge == row.rejected_edge)
    {
        return Err(StateArgumentSpecializationError::CandidateMismatch);
    }
    // Each resolved-arm argument naming a dispatch parameter is substituted by
    // the argument this incoming edge bound to that parameter. Every other
    // argument keeps its value.
    let bindings = resolved
        .bindings
        .iter()
        .map(|binding| {
            let argument = incoming
                .bindings
                .iter()
                .find(|incoming_binding| incoming_binding.parameter == binding.argument)
                .map(|incoming_binding| incoming_binding.argument)
                .unwrap_or(binding.argument);
            ValueBinding {
                parameter: binding.parameter,
                argument,
                scalar_type: binding.scalar_type,
            }
        })
        .collect::<Vec<_>>();
    // The fused traversal charges the incoming edge's source first — its own
    // edge identity — then the resolved arm edge's complete source custody.
    let mut provenance = incoming.provenance.clone();
    provenance.extend_from_slice(&resolved.provenance);
    if provenance.first() != Some(&PsiProvenance::Edge(incoming.psi_edge)) {
        return Err(StateArgumentSpecializationError::CandidateMismatch);
    }
    let mut fuel = incoming.fuel.clone();
    fuel.extend_from_slice(&resolved.fuel);
    let fused_edge = OptimizationEdge {
        psi_edge: incoming.psi_edge,
        target: resolved.target,
        bindings,
        structural_bindings: Vec::new(),
        trivial_affine_discards: Vec::new(),
        residual_affine_discards: Vec::new(),
        provenance,
        fuel,
    };
    let operation = O::Jump {
        psi_edge: incoming.psi_edge,
        target: resolved.target,
        bindings: fused_edge.bindings.clone(),
        structural_bindings: structural_bindings.clone(),
        trivial_affine_discards: trivial_affine_discards.clone(),
        residual_affine_discards: residual_affine_discards.clone(),
    };
    let uses = fused_edge
        .bindings
        .iter()
        .map(|binding| ValueUse {
            value: binding.argument,
            block: row.predecessor.block,
            node: row.predecessor.node,
        })
        .collect();
    Ok(OptimizationNode {
        operation,
        provenance: predecessor.provenance.clone(),
        fuel: predecessor.fuel.clone(),
        effect: predecessor.effect,
        definitions: predecessor.definitions.clone(),
        uses,
        successors: vec![fused_edge],
        ownership: predecessor.ownership.clone(),
    })
}

fn node_at<'a>(
    function: &'a PsiOptimizationFunction,
    row: &SpecializedStateEdge,
) -> Result<&'a OptimizationNode, StateArgumentSpecializationError> {
    let block = function
        .blocks
        .iter()
        .find(|block| block.id == row.predecessor.block)
        .ok_or(StateArgumentSpecializationError::MissingSite {
            machine: function.machine,
            block: row.predecessor.block,
            node: row.predecessor.node,
        })?;
    block
        .nodes
        .get(
            usize::try_from(row.predecessor.node)
                .map_err(|_| StateArgumentSpecializationError::CoordinateOverflow)?,
        )
        .ok_or(StateArgumentSpecializationError::MissingSite {
            machine: function.machine,
            block: row.predecessor.block,
            node: row.predecessor.node,
        })
}

pub(crate) fn realize(
    unit: &PsiOptimizationUnit,
    plan: &DispatchSpecializationPlan,
) -> Result<PsiOptimizationUnit, StateArgumentSpecializationError> {
    let input_function = unit
        .functions
        .iter()
        .find(|function| function.machine == plan.machine)
        .ok_or(StateArgumentSpecializationError::UnknownDispatch)?;
    // Derive every fused node from the input before mutating, so a malformed
    // row cannot leave a partially rewritten machine.
    let fused = plan
        .edges
        .iter()
        .map(|row| {
            fused_node(row, plan.dispatch, input_function).map(|node| (row.predecessor, node))
        })
        .collect::<Result<Vec<_>, StateArgumentSpecializationError>>()?;
    let mut output = unit.clone();
    let function = output
        .functions
        .iter_mut()
        .find(|function| function.machine == plan.machine)
        .ok_or(StateArgumentSpecializationError::UnknownDispatch)?;
    for (location, node) in fused {
        let index = usize::try_from(location.node)
            .map_err(|_| StateArgumentSpecializationError::CoordinateOverflow)?;
        let Some(slot) = function
            .blocks
            .iter_mut()
            .find(|block| block.id == location.block)
            .and_then(|block| block.nodes.get_mut(index))
        else {
            return Err(StateArgumentSpecializationError::MissingSite {
                machine: plan.machine,
                block: location.block,
                node: location.node,
            });
        };
        *slot = node;
    }
    output.identity = recompute_psi_optimization_unit_identity(&output);
    Ok(output)
}
