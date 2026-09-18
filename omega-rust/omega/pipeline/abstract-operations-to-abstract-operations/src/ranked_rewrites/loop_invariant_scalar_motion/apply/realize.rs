//! Optimizer module role: application leaf. Canonical node movement and derived-coordinate refresh.

use super::super::{
    EffectLink, LoopInvariantScalarMotionError, LoopInvariantScalarNode, NodeLocation, OperationId,
    PsiOptimizationFunction, PsiOptimizationUnit, PsiProvenance, ValueDefinitionSite,
    recompute_psi_optimization_unit_identity,
};
use abstract_operations::AbstractOperation as O;
use semantic_vocabulary::PlaceId;
use std::collections::{BTreeMap, BTreeSet};

/// Remove every planned scalar node at its exact source location, rebind any
/// invariant-parameter operands to their entry representatives, and insert the
/// run ahead of the component preheader's entry terminator, before any
/// already-relocated countdown-certificate constants.
pub(crate) fn realize(
    unit: &PsiOptimizationUnit,
    component: &optimization_unit::OptimizerCycleComponent,
    nodes: &[LoopInvariantScalarNode],
    certificate_tail: usize,
) -> Result<PsiOptimizationUnit, LoopInvariantScalarMotionError> {
    let mut output = unit.clone();
    let machine = component.id.machine;
    let function = output
        .functions
        .iter_mut()
        .find(|function| function.machine == machine)
        .ok_or(LoopInvariantScalarMotionError::UnknownComponent)?;
    // Remove in descending node order inside each block so earlier removals do
    // not shift later coordinates, then reassemble the run in plan order.
    let mut requests = nodes
        .iter()
        .map(|node| (node.location, node.psi_operation))
        .collect::<Vec<_>>();
    requests.sort_by_key(|(location, _)| (location.block, std::cmp::Reverse(location.node)));
    let mut removed = BTreeMap::new();
    for (location, operation) in requests {
        if location.machine != machine {
            return Err(LoopInvariantScalarMotionError::MissingNode {
                machine: location.machine,
                block: location.block,
                node: location.node,
            });
        }
        let block = function
            .blocks
            .iter_mut()
            .find(|block| block.id == location.block)
            .ok_or(LoopInvariantScalarMotionError::MissingNode {
                machine,
                block: location.block,
                node: location.node,
            })?;
        let index = usize::try_from(location.node)
            .map_err(|_| LoopInvariantScalarMotionError::CoordinateOverflow)?;
        let node = block
            .nodes
            .get(index)
            .filter(|node| node.provenance.first() == Some(&PsiProvenance::Operation(operation)));
        if node.is_none() {
            return Err(LoopInvariantScalarMotionError::MissingNode {
                machine,
                block: location.block,
                node: location.node,
            });
        }
        if removed
            .insert(operation, block.nodes.remove(index))
            .is_some()
        {
            return Err(LoopInvariantScalarMotionError::CandidateMismatch);
        }
    }
    let nodes = nodes
        .iter()
        .map(|planned| {
            let mut node = removed
                .remove(&planned.psi_operation)
                .ok_or(LoopInvariantScalarMotionError::CandidateMismatch)?;
            let substitution = planned
                .operand_rewrites
                .iter()
                .copied()
                .collect::<BTreeMap<_, _>>();
            crate::validation::substitute_invariant_scalar_operands(
                &mut node.operation,
                &substitution,
            );
            for value_use in &mut node.uses {
                if let Some(representative) = substitution.get(&value_use.value) {
                    value_use.value = *representative;
                }
            }
            if let Some((parameter, representative)) = planned.root_rewrite
                && !crate::validation::substitute_invariant_place_root(
                    &mut node.operation,
                    parameter,
                    representative,
                )
            {
                return Err(LoopInvariantScalarMotionError::CandidateMismatch);
            }
            if !planned.argument_rewrites.is_empty() {
                let rewrites = planned
                    .argument_rewrites
                    .iter()
                    .copied()
                    .collect::<BTreeMap<_, _>>();
                if !crate::validation::substitute_invariant_call_roots(
                    &mut node.operation,
                    &rewrites,
                ) {
                    return Err(LoopInvariantScalarMotionError::CandidateMismatch);
                }
            }
            Ok(node)
        })
        .collect::<Result<Vec<_>, LoopInvariantScalarMotionError>>()?;
    let Some(preheader_source) = crate::validation::shared_entry_source(component) else {
        return Err(LoopInvariantScalarMotionError::CandidateMismatch);
    };
    let preheader = function
        .blocks
        .iter_mut()
        .find(|block| block.id == preheader_source)
        .ok_or(LoopInvariantScalarMotionError::MissingNode {
            machine,
            block: preheader_source,
            node: 0,
        })?;
    let insertion = preheader
        .nodes
        .len()
        .checked_sub(1)
        .and_then(|jump| jump.checked_sub(certificate_tail))
        .ok_or(LoopInvariantScalarMotionError::CandidateMismatch)?;
    // An affine scalar-case result or structural-call result the run
    // relocates keeps one persistent preheader place live through the whole
    // component where the source established — and discarded — a fresh place
    // at every traversal's dispatch.
    let case_results: BTreeSet<PlaceId> = nodes
        .iter()
        .filter_map(|node| match &node.operation {
            O::EstablishScalarCase { result, .. } | O::CallStructural { result, .. }
                if result.multiplicity == terminal_psi::StructuralMultiplicity::Affine =>
            {
                Some(result.place)
            }
            _ => None,
        })
        .collect();
    for (offset, node) in nodes.into_iter().enumerate() {
        preheader.nodes.insert(insertion + offset, node);
    }
    if !case_results.is_empty() {
        // Rebuild the retained member nodes' edge and cleanup custody for the
        // relocated results: member-internal edges keep the persistent place
        // live while exit edges and member returns dispose it — the same
        // rewrite the relocation freeze replays against the seed.
        let members: BTreeSet<semantic_vocabulary::BlockId> =
            component.members.iter().copied().collect();
        let structural_places = function.structural_places.clone();
        for member in &component.members {
            let block = function
                .blocks
                .iter_mut()
                .find(|block| block.id == *member)
                .ok_or(LoopInvariantScalarMotionError::MissingNode {
                    machine,
                    block: *member,
                    node: 0,
                })?;
            for node in &mut block.nodes {
                crate::validation::rewrite_scalar_case_custody(
                    &structural_places,
                    &members,
                    &case_results,
                    node,
                );
            }
        }
    }
    refresh_coordinates_effects_and_facts(function)?;
    if !case_results.is_empty() {
        // Re-express each relocated result's verifier-frontier membership
        // through the transformed custody: production at the establishing
        // operation's exit, must-own joins across the component, and removal
        // at each disposal the custody rewrite spelled. The context
        // projection re-derives this same delta when it checks the attached
        // catalog, so the rewritten facts are evidence, not authority.
        let relocated = BTreeMap::from([(machine, case_results)]);
        let mut facts = std::mem::take(&mut output.ownership_frontier_facts);
        crate::validation::rewrite_relocated_case_result_frontiers(&mut facts, &output, &relocated);
        output.ownership_frontier_facts = facts;
    }
    output.identity = recompute_psi_optimization_unit_identity(&output);
    Ok(output)
}

pub(crate) fn operation_location(
    unit: &PsiOptimizationUnit,
    operation: OperationId,
) -> Option<NodeLocation> {
    let mut matches = unit.functions.iter().flat_map(|function| {
        function.blocks.iter().flat_map(move |block| {
            block
                .nodes
                .iter()
                .enumerate()
                .filter_map(move |(node, value)| {
                    (value.provenance.first() == Some(&PsiProvenance::Operation(operation)))
                        .then_some(NodeLocation {
                            machine: function.machine,
                            block: block.id,
                            node: u32::try_from(node).ok()?,
                        })
                })
        })
    });
    let location = matches.next()?;
    matches.next().is_none().then_some(location)
}

fn refresh_coordinates_effects_and_facts(
    function: &mut PsiOptimizationFunction,
) -> Result<(), LoopInvariantScalarMotionError> {
    let mut effect = 0u64;
    for block in &mut function.blocks {
        for (node_index, node) in block.nodes.iter_mut().enumerate() {
            let node_index = u32::try_from(node_index)
                .map_err(|_| LoopInvariantScalarMotionError::CoordinateOverflow)?;
            for definition in &mut node.definitions {
                definition.site = ValueDefinitionSite::Node {
                    block: block.id,
                    node: node_index,
                };
            }
            for value_use in &mut node.uses {
                value_use.block = block.id;
                value_use.node = node_index;
            }
            node.effect = EffectLink {
                input: effect,
                output: effect
                    .checked_add(1)
                    .ok_or(LoopInvariantScalarMotionError::CoordinateOverflow)?,
            };
            effect = node.effect.output;
        }
    }
    let operation_order = function
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .enumerate()
        .filter_map(|(position, node)| match node.provenance.first() {
            Some(PsiProvenance::Operation(operation)) => Some((*operation, position)),
            _ => None,
        })
        .collect::<BTreeMap<_, _>>();
    function.facts.sort_by_key(|fact| {
        let support = match fact {
            optimization_unit::OptimizationFact::OperationObligationReference {
                support, ..
            }
            | optimization_unit::OptimizationFact::BooleanConstant { support, .. }
            | optimization_unit::OptimizationFact::IntegerConstant { support, .. } => support,
        };
        operation_order.get(support).copied()
    });
    Ok(())
}
