//! Optimizer module role: admission leaf. Dispatch-shape and incoming-edge
//! predicates the producer enumeration and the independent validator share.
//!
//! Proposal decides *which* edges enter a plan; validation must never rerun
//! that decision through the producer's own plan — a matcher cannot attest to
//! itself. Both sides share only these predicates: the dispatch's block-shape
//! evidence and the per-edge admissibility that turns one qualifying incoming
//! traversal into its fused `SpecializedStateEdge` row.

use super::{
    BlockId, NodeLocation, O, OptimizationEdge, OptimizationNode, PsiOptimizationFunction,
    PsiOptimizationUnit, ScalarConstant, ScalarConstantAnalysis, SpecializedStateEdge,
};
use semantic_vocabulary::{EdgeId, MachineId, ValueId};

/// The dispatch context every admitted edge resolves against: the owning
/// function, the block's own state-argument parameter, and the conditional's
/// two arm edges. `None` from [`dispatch_evidence`] when `dispatch` is not an
/// eligible dispatch state — an entry block, a block with structural
/// parameters, a block whose sole node is not a `Conditional` on one of its
/// own scalar parameters, a conditional with an unresolved arm edge, or a
/// state argument the sparse constant analysis already proves globally
/// Boolean (constant-conditional-fold custody, not specialization).
pub(super) struct DispatchEvidence<'a> {
    pub(super) unit: &'a PsiOptimizationUnit,
    pub(super) function: &'a PsiOptimizationFunction,
    pub(super) machine: MachineId,
    pub(super) dispatch: BlockId,
    pub(super) parameter: ValueId,
    pub(super) when_true: &'a OptimizationEdge,
    pub(super) when_false: &'a OptimizationEdge,
}

/// The dispatch context for one block, or `None` when it is not an
/// admissible dispatch state.
pub(super) fn dispatch_evidence<'a>(
    unit: &'a PsiOptimizationUnit,
    function: &'a PsiOptimizationFunction,
    dispatch: BlockId,
    constants: &ScalarConstantAnalysis,
) -> Option<DispatchEvidence<'a>> {
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
    Some(DispatchEvidence {
        unit,
        function,
        machine,
        dispatch,
        parameter: parameter.value,
        when_true: arm_edge(when_true.psi_edge)?,
        when_false: arm_edge(when_false.psi_edge)?,
    })
}

/// Every edge entering `dispatch`, with its owner coordinates — the owner
/// block, the node index inside it, the node, and the edge itself. Proposal
/// enumerates this roster to select candidates; validation counts it to
/// reject a declared set that would fuse every incoming edge and leave the
/// dispatch state unreachable.
pub(super) fn incoming_edges(
    function: &PsiOptimizationFunction,
    dispatch: BlockId,
) -> Vec<(BlockId, usize, &OptimizationNode, &OptimizationEdge)> {
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
    incoming
}

/// The admissibility of one incoming edge under `evidence`, or `None` when
/// the edge may not specialize. Only an unconditional traversal may fuse:
/// the owner must be a `Jump` whose single successor is this exact edge, no
/// affine or structural custody may ride either side of the fused traversal,
/// the edge must bind the state argument to a value the sparse constant
/// analysis proves is one exact Boolean, and the arm that constant resolves
/// must carry no custody and reach a block without structural parameters.
pub(super) fn admit_incoming_edge(
    evidence: &DispatchEvidence<'_>,
    owner_block: BlockId,
    node_index: usize,
    owner_node: &OptimizationNode,
    edge: &OptimizationEdge,
    constants: &ScalarConstantAnalysis,
) -> Option<SpecializedStateEdge> {
    let predecessor = NodeLocation {
        machine: evidence.machine,
        block: owner_block,
        node: u32::try_from(node_index).ok()?,
    };
    let O::Jump {
        psi_edge,
        target,
        structural_bindings,
        trivial_affine_discards,
        residual_affine_discards,
        ..
    } = &owner_node.operation
    else {
        return None;
    };
    if *psi_edge != edge.psi_edge
        || *target != evidence.dispatch
        || !structural_bindings.is_empty()
        || !trivial_affine_discards.is_empty()
        || !residual_affine_discards.is_empty()
        || !edge.structural_bindings.is_empty()
        || !edge.trivial_affine_discards.is_empty()
        || !edge.residual_affine_discards.is_empty()
    {
        return None;
    }
    let binding = edge
        .bindings
        .iter()
        .find(|binding| binding.parameter == evidence.parameter)?;
    let fact = constants.facts.iter().find(|fact| {
        fact.valid_in.machine == evidence.machine
            && fact.valid_in.revision == evidence.unit.identity
            && fact.value == binding.argument
    })?;
    let ScalarConstant::Boolean(constant) = fact.constant else {
        return None;
    };
    let (resolved, rejected) = if constant {
        (evidence.when_true, evidence.when_false)
    } else {
        (evidence.when_false, evidence.when_true)
    };
    if !resolved.structural_bindings.is_empty()
        || !resolved.trivial_affine_discards.is_empty()
        || !resolved.residual_affine_discards.is_empty()
    {
        return None;
    }
    let resolved_block = evidence
        .function
        .blocks
        .iter()
        .find(|candidate| candidate.id == resolved.target)?;
    if !resolved_block.structural_parameters.is_empty() {
        return None;
    }
    Some(SpecializedStateEdge {
        incoming_edge: edge.psi_edge,
        predecessor,
        parameter: evidence.parameter,
        argument: binding.argument,
        constant,
        taken_edge: resolved.psi_edge,
        rejected_edge: rejected.psi_edge,
        resolved_target: resolved.target,
    })
}
