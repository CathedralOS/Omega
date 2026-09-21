//! Independent state-argument specialization replay mechanics.
//!
//! Validation never trusts the candidate's specialization rows: it re-derives
//! the dispatch plan from a locally reconstructed cyclic-machine roster and an
//! independently recomputed sparse-constant lattice, requires the claimed rows
//! to equal the replayed rows exactly, rebuilds the output itself, and
//! reconstructs the exact edge custody the fused traversals carry.

use crate::BTreeMap;
use crate::BTreeSet;
use crate::BlockId;
use crate::EdgeId;
use crate::MachineId;
use crate::NodeLocation;
use crate::O;
use crate::OptimizationEdge;
use crate::OptimizationNode;
use crate::OptimizationUnitValidationError;
use crate::OptimizationValidatorIdentity;
use crate::ProvenanceDisposition;
use crate::ProvenanceRewrite;
use crate::PsiOptimizationFunction;
use crate::PsiOptimizationUnit;
use crate::PsiProvenance;
use crate::PsiRealizationSite;
use crate::PsiRewriteCandidate;
use crate::PsiRewritePatch;
use crate::ScalarConstantValue;
use crate::SpecializedStateEdgeRow;
use crate::ValidatedPsiRewrite;
use crate::ValueId;
use crate::ValueUse;
use crate::recompute_psi_optimization_unit_identity;
use crate::validate_psi_optimization_unit;
use crate::validator_scalar_constant_facts;

/// Machines holding a cyclic component are frozen byte-exact for this family;
/// the roster is reconstructed privately over each function's canonical block
/// projection so no session or proposal authority is consulted.
fn cyclic_machines(unit: &PsiOptimizationUnit) -> BTreeSet<MachineId> {
    unit.functions
        .iter()
        .filter_map(|function| {
            let graph = function
                .blocks
                .iter()
                .map(|block| {
                    (
                        block.id,
                        block
                            .nodes
                            .iter()
                            .flat_map(|node| node.successors.iter().map(|edge| edge.target))
                            .collect::<Vec<_>>(),
                    )
                })
                .collect::<BTreeMap<_, _>>();
            has_cycle(&graph).then_some(function.machine)
        })
        .collect()
}

/// Depth-first back-edge detection over one function's block projection. A
/// back edge is exactly a nonempty cyclic component: a grey successor on the
/// current stack is either part of a multi-block SCC or a self loop.
fn has_cycle(graph: &BTreeMap<BlockId, Vec<BlockId>>) -> bool {
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum Mark {
        Visiting,
        Done,
    }
    let mut marks = BTreeMap::<BlockId, Mark>::new();
    for &root in graph.keys() {
        if marks.contains_key(&root) {
            continue;
        }
        let mut stack = vec![(root, 0usize)];
        marks.insert(root, Mark::Visiting);
        while let Some(&(block, index)) = stack.last() {
            let successors = graph.get(&block).cloned().unwrap_or_default();
            if index >= successors.len() {
                marks.insert(block, Mark::Done);
                stack.pop();
                continue;
            }
            let successor = successors[index];
            if let Some(top) = stack.last_mut() {
                top.1 = index + 1;
            }
            match marks.get(&successor) {
                Some(Mark::Visiting) => return true,
                Some(Mark::Done) => {}
                None => {
                    marks.insert(successor, Mark::Visiting);
                    stack.push((successor, 0));
                }
            }
        }
    }
    false
}

/// Independently derived specialization rows for one dispatch block, or `None`
/// when the block is not an eligible dispatch state. Mirrors the proposal's
/// exact admission: non-entry, structurally clean, single-`Conditional` block
/// whose condition is one of its own scalar parameters, no globally proven
/// Boolean for that parameter, and only constant-supplied incoming edges with
/// empty affine and structural custody — an unconditional `Jump` successor or
/// one arm of a `Conditional` predecessor. When every incoming edge qualifies
/// the dispatch would be orphaned, so the plan reports no rows.
fn plan_dispatch(
    function: &PsiOptimizationFunction,
    dispatch: BlockId,
    constants: &BTreeMap<ValueId, bool>,
) -> Option<Vec<SpecializedStateEdgeRow>> {
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
    let parameter = block
        .parameters
        .iter()
        .find(|parameter| parameter.value == *condition)?;
    if constants.contains_key(&parameter.value) {
        return None;
    }
    let arm_edge = |edge: EdgeId| {
        node.successors
            .iter()
            .find(|successor| successor.psi_edge == edge)
    };
    let when_true_edge = arm_edge(when_true.psi_edge)?;
    let when_false_edge = arm_edge(when_false.psi_edge)?;

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
        match &owner_node.operation {
            O::Jump {
                psi_edge,
                target,
                structural_bindings,
                trivial_affine_discards,
                residual_affine_discards,
                ..
            } => {
                if *psi_edge != edge.psi_edge
                    || *target != dispatch
                    || !structural_bindings.is_empty()
                    || !trivial_affine_discards.is_empty()
                    || !residual_affine_discards.is_empty()
                {
                    continue;
                }
            }
            O::Conditional {
                when_true,
                when_false,
                ..
            } => {
                // The admitted edge is exactly one arm of the conditional;
                // the sibling arm is not part of this traversal and keeps
                // its own custody untouched. A conditional successor carries
                // no residual affine discards, so only the arm's own fields
                // can refuse.
                let Some(arm) = [when_true, when_false]
                    .into_iter()
                    .find(|arm| arm.psi_edge == edge.psi_edge)
                else {
                    continue;
                };
                if arm.target != dispatch
                    || !arm.structural_bindings.is_empty()
                    || !arm.trivial_affine_discards.is_empty()
                {
                    continue;
                }
            }
            _ => continue,
        }
        if !edge.structural_bindings.is_empty()
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
        let Some(&constant) = constants.get(&binding.argument) else {
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
        edges.push(SpecializedStateEdgeRow {
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
    if edges.len() == incoming.len() {
        edges.clear();
    }
    Some(edges)
}

/// The fused node a specialization admits at one predecessor site: every
/// admitted row naming this site applied to a single reconstruction — the
/// retargeted `Jump` and its fused successor edge, or the `Conditional`
/// whose admitted arms each carry their own fused successor edge while the
/// unadmitted arms stay byte-exact — plus the derived use roster. The fused
/// edge keeps the incoming edge's own Psi identity first in provenance and
/// appends the resolved arm edge's complete custody.
fn fused_node(
    site_rows: &[&SpecializedStateEdgeRow],
    dispatch: BlockId,
    function: &PsiOptimizationFunction,
) -> Result<OptimizationNode, OptimizationUnitValidationError> {
    let Some(first) = site_rows.first() else {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    };
    // A site fold is exact only when every row names this same predecessor;
    // a row drifted onto a different site would silently fuse the wrong arm.
    if site_rows
        .iter()
        .any(|row| row.predecessor != first.predecessor)
    {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    let block = function
        .blocks
        .iter()
        .find(|block| block.id == first.predecessor.block)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let predecessor = block
        .nodes
        .get(
            usize::try_from(first.predecessor.node)
                .map_err(|_| OptimizationUnitValidationError::CandidateLocationMissing)?,
        )
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let dispatch_block = function
        .blocks
        .iter()
        .find(|block| block.id == dispatch)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let [dispatch_node] = dispatch_block.nodes.as_slice() else {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
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
                return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
            }
            if first.predecessor.machine != function.machine
                || *psi_edge != first.incoming_edge
                || *target != dispatch
                || !structural_bindings.is_empty()
                || !trivial_affine_discards.is_empty()
                || !residual_affine_discards.is_empty()
            {
                return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
            }
            let incoming = predecessor
                .successors
                .iter()
                .find(|edge| edge.psi_edge == first.incoming_edge)
                .ok_or(OptimizationUnitValidationError::CandidatePatchMismatch)?;
            if incoming.target != dispatch
                || !incoming.structural_bindings.is_empty()
                || !incoming.trivial_affine_discards.is_empty()
                || !incoming.residual_affine_discards.is_empty()
            {
                return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
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
                return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
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
                    return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
                };
                if arm.target != dispatch
                    || !arm.structural_bindings.is_empty()
                    || !arm.trivial_affine_discards.is_empty()
                {
                    return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
                }
                let Some(index) = successors
                    .iter()
                    .position(|edge| edge.psi_edge == row.incoming_edge)
                else {
                    return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
                };
                let incoming = &successors[index];
                if incoming.target != dispatch
                    || !incoming.structural_bindings.is_empty()
                    || !incoming.trivial_affine_discards.is_empty()
                    || !incoming.residual_affine_discards.is_empty()
                {
                    return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
                }
                let fused = fused_edge(row, incoming, dispatch_node)?;
                *arm = abstract_operations::AbstractSuccessor {
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
        _ => Err(OptimizationUnitValidationError::CandidatePatchMismatch),
    }
}

/// The fused edge one admitted row admits: the incoming edge's own Psi
/// identity and custody first, the resolved arm edge's complete custody
/// appended, and the resolved arm's bindings composed through the incoming
/// edge's bindings — each resolved-arm argument naming a dispatch parameter
/// maps to the argument this edge bound to that parameter; every other
/// argument keeps its value.
fn fused_edge(
    row: &SpecializedStateEdgeRow,
    incoming: &OptimizationEdge,
    dispatch_node: &OptimizationNode,
) -> Result<OptimizationEdge, OptimizationUnitValidationError> {
    let resolved = dispatch_node
        .successors
        .iter()
        .find(|edge| edge.psi_edge == row.taken_edge)
        .ok_or(OptimizationUnitValidationError::CandidatePatchMismatch)?;
    if resolved.target != row.resolved_target
        || row.rejected_edge == row.taken_edge
        || !dispatch_node
            .successors
            .iter()
            .any(|edge| edge.psi_edge == row.rejected_edge)
    {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
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
            abstract_operations::ValueBinding {
                parameter: binding.parameter,
                argument,
                scalar_type: binding.scalar_type,
            }
        })
        .collect::<Vec<_>>();
    let mut provenance = incoming.provenance.clone();
    provenance.extend_from_slice(&resolved.provenance);
    if provenance.first() != Some(&PsiProvenance::Edge(incoming.psi_edge)) {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
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
fn node_uses(
    operation: &abstract_operations::AbstractOperation,
    location: NodeLocation,
) -> Vec<ValueUse> {
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

/// The accepted custody ledger: each fused edge records the incoming edge's
/// retained occurrence and the resolved arm edge's fan-out onto the fused
/// edge, and each resolved arm edge records its surviving dispatch occurrence
/// once — two incoming edges may legitimately resolve to the same dispatch
/// arm, so that occurrence is a per-edge row, not a per-specialization row.
fn accepted_provenance(
    function: &PsiOptimizationFunction,
    machine: MachineId,
    rows: &[SpecializedStateEdgeRow],
) -> Result<Vec<ProvenanceRewrite>, OptimizationUnitValidationError> {
    let find_edge = |edge: EdgeId| {
        function
            .blocks
            .iter()
            .flat_map(|block| &block.nodes)
            .flat_map(|node| &node.successors)
            .find(|candidate| candidate.psi_edge == edge)
            .ok_or(OptimizationUnitValidationError::CandidateProvenanceMismatch)
    };
    let mut provenance = Vec::new();
    let mut resolved_edges = BTreeSet::new();
    for row in rows {
        let incoming = find_edge(row.incoming_edge)?;
        let resolved = find_edge(row.taken_edge)?;
        let incoming_site = PsiRealizationSite::Edge {
            machine,
            edge: row.incoming_edge,
        };
        let resolved_site = PsiRealizationSite::Edge {
            machine,
            edge: row.taken_edge,
        };
        provenance.push(ProvenanceRewrite {
            input: incoming_site,
            disposition: ProvenanceDisposition::RealizedAt(incoming_site),
            sources: incoming.provenance.clone(),
            fuel: incoming.fuel.clone(),
        });
        provenance.push(ProvenanceRewrite {
            input: resolved_site,
            disposition: ProvenanceDisposition::RealizedAt(incoming_site),
            sources: resolved.provenance.clone(),
            fuel: resolved.fuel.clone(),
        });
        resolved_edges.insert(row.taken_edge);
    }
    for taken_edge in resolved_edges {
        let resolved = find_edge(taken_edge)?;
        let resolved_site = PsiRealizationSite::Edge {
            machine,
            edge: taken_edge,
        };
        provenance.push(ProvenanceRewrite {
            input: resolved_site,
            disposition: ProvenanceDisposition::RealizedAt(resolved_site),
            sources: resolved.provenance.clone(),
            fuel: resolved.fuel.clone(),
        });
    }
    provenance.sort_by_key(|row| {
        (
            row.input,
            row.disposition.canonical_tag(),
            row.disposition.site(),
        )
    });
    Ok(provenance)
}

pub(super) fn validate(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
    let PsiRewritePatch::SpecializeStateArgument(patch) = candidate.patch() else {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    };
    if candidate.node_decision_point()
        != Some(NodeLocation {
            machine: patch.machine,
            block: patch.dispatch,
            node: 0,
        })
    {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    if patch.edges.is_empty()
        || candidate.predicted_cost_delta()
            != -i64::try_from(patch.edges.len())
                .map_err(|_| OptimizationUnitValidationError::CandidatePatchMismatch)?
    {
        return Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch);
    }
    // A machine holding a cyclic component is frozen: none of its edges may
    // move, so any candidate naming it is rejected before plan replay.
    if cyclic_machines(input).contains(&patch.machine) {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    let function = input
        .functions
        .iter()
        .find(|function| function.machine == patch.machine)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let constants = validator_scalar_constant_facts(input.identity, function)
        .into_iter()
        .filter_map(|(value, constant, _)| match constant {
            ScalarConstantValue::Boolean(constant) => Some((value, constant)),
            ScalarConstantValue::Integer(_) => None,
        })
        .collect::<BTreeMap<ValueId, bool>>();
    let replayed = plan_dispatch(function, patch.dispatch, &constants)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    if replayed.is_empty() {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    if patch.edges != replayed {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    let mut expected_blocks = vec![patch.dispatch];
    expected_blocks.extend(patch.edges.iter().map(|row| row.predecessor.block));
    expected_blocks.sort_unstable();
    expected_blocks.dedup();
    let expected_provenance = accepted_provenance(function, patch.machine, &patch.edges)?;
    if candidate.affected_blocks() != expected_blocks
        || candidate.provenance() != expected_provenance
    {
        return Err(OptimizationUnitValidationError::CandidateProvenanceMismatch);
    }

    let input_function = function;
    // Rows sharing one predecessor site — both constant arms of one
    // conditional — fold into a single node reconstruction; applied
    // independently, the later row would overwrite the earlier arm's fusion
    // while the candidate still claims both.
    let mut sites = BTreeMap::<NodeLocation, Vec<&SpecializedStateEdgeRow>>::new();
    for row in &patch.edges {
        sites.entry(row.predecessor).or_default().push(row);
    }
    let fused = sites
        .into_iter()
        .map(|(location, rows)| {
            fused_node(&rows, patch.dispatch, input_function).map(|node| (location, node))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut output = input.clone();
    let output_function = output
        .functions
        .iter_mut()
        .find(|function| function.machine == patch.machine)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    for (location, node) in fused {
        let index = usize::try_from(location.node)
            .map_err(|_| OptimizationUnitValidationError::CandidateLocationMissing)?;
        let Some(slot) = output_function
            .blocks
            .iter_mut()
            .find(|block| block.id == location.block)
            .and_then(|block| block.nodes.get_mut(index))
        else {
            return Err(OptimizationUnitValidationError::CandidateLocationMissing);
        };
        *slot = node;
    }
    output.identity = recompute_psi_optimization_unit_identity(&output);
    validate_psi_optimization_unit(&output)?;
    let output_function = output
        .functions
        .iter()
        .find(|function| function.machine == patch.machine)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    for input_block in &input_function.blocks {
        if !expected_blocks.contains(&input_block.id)
            && output_function
                .blocks
                .iter()
                .find(|block| block.id == input_block.id)
                != Some(input_block)
        {
            return Err(OptimizationUnitValidationError::CandidateOutsideRegionMismatch);
        }
    }
    for input_function_other in &input.functions {
        if input_function_other.machine != patch.machine
            && !output.functions.contains(input_function_other)
        {
            return Err(OptimizationUnitValidationError::CandidateOutsideRegionMismatch);
        }
    }
    Ok(ValidatedPsiRewrite {
        unit: output,
        candidate: candidate.identity(),
        validator: OptimizationValidatorIdentity::from_canonical_bytes(
            b"omega.validator.state-argument-specialization.v1",
        ),
        provenance: expected_provenance,
    })
}
