//! Optimizer module role: application leaf. Canonical pair movement and derived-coordinate refresh.

use std::collections::BTreeMap;

use super::super::*;

pub(crate) fn realize(
    unit: &PsiOptimizationUnit,
    placement: &UnsignedCountdownInvariantConstantPlacements,
) -> Result<PsiOptimizationUnit, CountdownInvariantConstantRelocationError> {
    let mut output = unit.clone();
    let machine = placement.component.machine;
    let function = output
        .functions
        .iter_mut()
        .find(|function| function.machine == machine)
        .ok_or(CountdownInvariantConstantRelocationError::UnknownComponent)?;
    let mut requests = placement
        .placements
        .iter()
        .map(|row| {
            (
                row.constant.role,
                row.constant.location,
                row.constant.psi_operation,
            )
        })
        .collect::<Vec<_>>();
    requests.sort_by_key(|(_, location, _)| (location.block, std::cmp::Reverse(location.node)));
    let mut nodes = BTreeMap::new();
    for (role, location, operation) in requests {
        if location.machine != machine {
            return Err(CountdownInvariantConstantRelocationError::MissingNode {
                machine: location.machine,
                block: location.block,
                node: location.node,
            });
        }
        let block = function
            .blocks
            .iter_mut()
            .find(|block| block.id == location.block)
            .ok_or(CountdownInvariantConstantRelocationError::MissingNode {
                machine,
                block: location.block,
                node: location.node,
            })?;
        let index = usize::try_from(location.node)
            .map_err(|_| CountdownInvariantConstantRelocationError::CoordinateOverflow)?;
        let node = block.nodes.get(index).filter(|node| {
            node.provenance.first() == Some(&optimization_unit::PsiProvenance::Operation(operation))
        });
        if node.is_none() {
            return Err(CountdownInvariantConstantRelocationError::MissingNode {
                machine,
                block: location.block,
                node: location.node,
            });
        }
        let node = block.nodes.remove(index);
        if nodes.insert(role, node).is_some() {
            return Err(CountdownInvariantConstantRelocationError::CandidateMismatch);
        }
    }
    let [zero, one] = [
        CountdownInvariantConstantRole::PositiveGuardZero,
        CountdownInvariantConstantRole::BackedgeDecrementOne,
    ]
    .map(|role| nodes.remove(&role));
    let (Some(zero), Some(one)) = (zero, one) else {
        return Err(CountdownInvariantConstantRelocationError::CandidateMismatch);
    };
    if !nodes.is_empty() {
        return Err(CountdownInvariantConstantRelocationError::CandidateMismatch);
    }
    let destination = placement
        .placements
        .first()
        .ok_or(CountdownInvariantConstantRelocationError::CandidateMismatch)?
        .destination
        .before;
    let preheader = function
        .blocks
        .iter_mut()
        .find(|block| block.id == destination.block)
        .ok_or(CountdownInvariantConstantRelocationError::MissingNode {
            machine,
            block: destination.block,
            node: destination.node,
        })?;
    let jump = preheader
        .nodes
        .len()
        .checked_sub(1)
        .ok_or(CountdownInvariantConstantRelocationError::CandidateMismatch)?;
    preheader.nodes.insert(jump, zero);
    preheader.nodes.insert(jump + 1, one);
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
                    (value.provenance.first()
                        == Some(&optimization_unit::PsiProvenance::Operation(operation)))
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
) -> Result<(), CountdownInvariantConstantRelocationError> {
    let mut effect = 0u64;
    for block in &mut function.blocks {
        for (node_index, node) in block.nodes.iter_mut().enumerate() {
            let node_index = u32::try_from(node_index)
                .map_err(|_| CountdownInvariantConstantRelocationError::CoordinateOverflow)?;
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
                    .ok_or(CountdownInvariantConstantRelocationError::CoordinateOverflow)?,
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
            Some(optimization_unit::PsiProvenance::Operation(operation)) => {
                Some((*operation, position))
            }
            _ => None,
        })
        .collect::<BTreeMap<_, _>>();
    function.facts.sort_by_key(|fact| {
        let support = match fact {
            OptimizationFact::OperationObligationReference { support, .. }
            | OptimizationFact::BooleanConstant { support, .. }
            | OptimizationFact::IntegerConstant { support, .. } => support,
        };
        operation_order.get(support).copied()
    });
    Ok(())
}
