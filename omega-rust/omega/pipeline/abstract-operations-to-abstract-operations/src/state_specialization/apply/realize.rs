//! Optimizer module role: application leaf. Exact edge fusion and derived-coordinate refresh.
//!
//! For each admitted specialization row, the incoming edge is retargeted to
//! the resolved dispatch arm's target. The fused edge keeps its own Psi edge
//! identity first in provenance — the shape every edge must keep — and
//! appends the resolved arm edge's complete custody, so one taken traversal
//! charges both sources exactly once. Scalar bindings compose through the
//! incoming edge: any resolved-arm argument naming a dispatch parameter maps
//! to the argument this edge bound to that parameter, and any other argument
//! keeps its value — such a value is either a function parameter or defined
//! in a block dominating the dispatch state, and that block also dominates
//! this edge's owner, so the use stays def-ordered.
//!
//! The incoming edge is either the single successor of an unconditional
//! `Jump` or one arm of a `Conditional` predecessor. A conditional keeps its
//! sibling arm byte-exact — it is not part of the fused traversal — and both
//! arms of one conditional may fuse in one plan when each supplies its own
//! proven constant; rows sharing one predecessor site therefore fold into a
//! single node reconstruction rather than overwriting each other.

use super::super::{
    BlockId, DispatchSpecializationPlan, NodeLocation, O, OptimizationEdge, OptimizationNode,
    PsiOptimizationUnit, PsiProvenance, SpecializedStateEdge, StateArgumentSpecializationError,
    ValueBinding, ValueId, ValueUse, recompute_psi_optimization_unit_identity,
};
use abstract_operations::AbstractSuccessor;
use optimization_unit::PsiOptimizationFunction;
use std::collections::BTreeMap;

/// The fused node shape a specialization admits at one predecessor site:
/// every admitted row naming this site applied to a single reconstruction —
/// the retargeted `Jump` and its fused successor edge, or the `Conditional`
/// whose admitted arms each carry their own fused successor edge while the
/// unadmitted arms stay byte-exact — plus the derived use roster.
/// Application and the independent custody walk share this construction so
/// both name exactly the same admitted difference.
pub(crate) fn fused_node(
    site_rows: &[&SpecializedStateEdge],
    dispatch: BlockId,
    function: &PsiOptimizationFunction,
) -> Result<OptimizationNode, StateArgumentSpecializationError> {
    let Some(first) = site_rows.first() else {
        return Err(StateArgumentSpecializationError::CandidateMismatch);
    };
    // A site fold is exact only when every row names this same predecessor;
    // a row drifted onto a different site would silently fuse the wrong arm.
    if site_rows
        .iter()
        .any(|row| row.predecessor != first.predecessor)
    {
        return Err(StateArgumentSpecializationError::CandidateMismatch);
    }
    let predecessor = node_at(function, first)?;
    let dispatch_block = function
        .blocks
        .iter()
        .find(|block| block.id == dispatch)
        .ok_or(StateArgumentSpecializationError::MissingSite {
            machine: function.machine,
            block: dispatch,
            node: 0,
        })?;
    let [dispatch_node] = dispatch_block.nodes.as_slice() else {
        return Err(StateArgumentSpecializationError::CandidateMismatch);
    };
    match &predecessor.operation {
        O::Jump {
            psi_edge,
            target,
            structural_bindings,
            trivial_affine_discards,
            residual_affine_discards,
            ..
        } => {
            // A jump owns exactly one edge, so exactly one row may land here.
            if site_rows.len() != 1 {
                return Err(StateArgumentSpecializationError::CandidateMismatch);
            }
            if first.predecessor.machine != function.machine
                || *psi_edge != first.incoming_edge
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
                .find(|edge| edge.psi_edge == first.incoming_edge)
                .ok_or(StateArgumentSpecializationError::CandidateMismatch)?;
            if incoming.target != dispatch
                || !incoming.structural_bindings.is_empty()
                || !incoming.trivial_affine_discards.is_empty()
                || !incoming.residual_affine_discards.is_empty()
            {
                return Err(StateArgumentSpecializationError::CandidateMismatch);
            }
            let fused_edge = fused_edge(first, incoming, dispatch_node)?;
            let operation = O::Jump {
                psi_edge: incoming.psi_edge,
                target: fused_edge.target,
                bindings: fused_edge.bindings.clone(),
                structural_bindings: structural_bindings.clone(),
                trivial_affine_discards: trivial_affine_discards.clone(),
                residual_affine_discards: residual_affine_discards.clone(),
            };
            Ok(OptimizationNode {
                uses: node_uses(&operation, first.predecessor),
                operation,
                provenance: predecessor.provenance.clone(),
                fuel: predecessor.fuel.clone(),
                effect: predecessor.effect,
                definitions: predecessor.definitions.clone(),
                successors: vec![fused_edge],
                ownership: predecessor.ownership.clone(),
            })
        }
        O::Conditional {
            condition,
            when_true,
            when_false,
        } => {
            if first.predecessor.machine != function.machine {
                return Err(StateArgumentSpecializationError::CandidateMismatch);
            }
            // Each admitted row fuses one arm of this conditional: the arm's
            // successor record and its successor edge retarget together while
            // the sibling arm and edge stay byte-exact. A repeated row reuses
            // the fused arm's kept identity but misses the dispatch target
            // check, so a duplicated incoming edge still fails.
            let mut fused_when_true = when_true.clone();
            let mut fused_when_false = when_false.clone();
            let mut successors = predecessor.successors.clone();
            for row in site_rows {
                let arm = if fused_when_true.psi_edge == row.incoming_edge {
                    &mut fused_when_true
                } else if fused_when_false.psi_edge == row.incoming_edge {
                    &mut fused_when_false
                } else {
                    return Err(StateArgumentSpecializationError::CandidateMismatch);
                };
                if arm.target != dispatch
                    || !arm.structural_bindings.is_empty()
                    || !arm.trivial_affine_discards.is_empty()
                {
                    return Err(StateArgumentSpecializationError::CandidateMismatch);
                }
                let Some(index) = successors
                    .iter()
                    .position(|edge| edge.psi_edge == row.incoming_edge)
                else {
                    return Err(StateArgumentSpecializationError::CandidateMismatch);
                };
                let incoming = &successors[index];
                if incoming.target != dispatch
                    || !incoming.structural_bindings.is_empty()
                    || !incoming.trivial_affine_discards.is_empty()
                    || !incoming.residual_affine_discards.is_empty()
                {
                    return Err(StateArgumentSpecializationError::CandidateMismatch);
                }
                let fused = fused_edge(row, incoming, dispatch_node)?;
                *arm = AbstractSuccessor {
                    psi_edge: fused.psi_edge,
                    target: fused.target,
                    bindings: fused.bindings.clone(),
                    structural_bindings: fused.structural_bindings.clone(),
                    trivial_affine_discards: fused.trivial_affine_discards.clone(),
                };
                successors[index] = fused;
            }
            let operation = O::Conditional {
                condition: *condition,
                when_true: fused_when_true,
                when_false: fused_when_false,
            };
            Ok(OptimizationNode {
                uses: node_uses(&operation, first.predecessor),
                operation,
                provenance: predecessor.provenance.clone(),
                fuel: predecessor.fuel.clone(),
                effect: predecessor.effect,
                definitions: predecessor.definitions.clone(),
                successors,
                ownership: predecessor.ownership.clone(),
            })
        }
        _ => Err(StateArgumentSpecializationError::CandidateMismatch),
    }
}

/// The fused edge one admitted row admits: the incoming edge's own Psi
/// identity and custody first, the resolved arm edge's complete custody
/// appended, and the resolved arm's bindings composed through the incoming
/// edge's bindings — each resolved-arm argument naming a dispatch parameter
/// maps to the argument this edge bound to that parameter; every other
/// argument keeps its value.
fn fused_edge(
    row: &SpecializedStateEdge,
    incoming: &OptimizationEdge,
    dispatch_node: &OptimizationNode,
) -> Result<OptimizationEdge, StateArgumentSpecializationError> {
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
    Ok(OptimizationEdge {
        psi_edge: incoming.psi_edge,
        target: resolved.target,
        bindings,
        structural_bindings: Vec::new(),
        trivial_affine_discards: Vec::new(),
        residual_affine_discards: Vec::new(),
        provenance,
        fuel,
    })
}

/// The use roster of a fused control node, in the same order unit
/// construction derives it from the operation — condition first, then
/// `when_true` and `when_false` arm arguments for a conditional, or the jump
/// bindings for an unconditional edge — so the reconstructed node carries
/// exactly the metadata an independent rebuild recomputes.
fn node_uses(operation: &O, location: NodeLocation) -> Vec<ValueUse> {
    let arguments: Vec<ValueId> = match operation {
        O::Jump { bindings, .. } => bindings.iter().map(|binding| binding.argument).collect(),
        O::Conditional {
            condition,
            when_true,
            when_false,
        } => std::iter::once(*condition)
            .chain(when_true.bindings.iter().map(|binding| binding.argument))
            .chain(when_false.bindings.iter().map(|binding| binding.argument))
            .collect(),
        _ => Vec::new(),
    };
    arguments
        .into_iter()
        .map(|value| ValueUse {
            value,
            block: location.block,
            node: location.node,
        })
        .collect()
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
    // row cannot leave a partially rewritten machine. Rows sharing one
    // predecessor site — both constant-supplied arms of one conditional —
    // fold into a single node reconstruction; applying them independently
    // would let the later row overwrite the earlier arm's fusion while the
    // custody ledger still claims both.
    let mut sites = BTreeMap::<NodeLocation, Vec<&SpecializedStateEdge>>::new();
    for row in &plan.edges {
        sites.entry(row.predecessor).or_default().push(row);
    }
    let fused = sites
        .into_iter()
        .map(|(location, rows)| {
            fused_node(&rows, plan.dispatch, input_function).map(|node| (location, node))
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
