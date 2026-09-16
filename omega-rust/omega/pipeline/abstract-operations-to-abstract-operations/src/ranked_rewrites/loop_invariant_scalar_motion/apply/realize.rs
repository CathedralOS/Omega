//! Optimizer module role: application leaf. Canonical node movement and derived-coordinate refresh.

use super::super::{
    EffectLink, LoopInvariantScalarMotionError, LoopInvariantScalarNode, NodeLocation, OperationId,
    PsiOptimizationFunction, PsiOptimizationUnit, PsiProvenance, ValueDefinitionSite,
    recompute_psi_optimization_unit_identity,
};
use std::collections::BTreeMap;

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
    for (offset, node) in nodes.into_iter().enumerate() {
        preheader.nodes.insert(insertion + offset, node);
    }
    refresh_coordinates_effects_and_facts(function)?;
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
