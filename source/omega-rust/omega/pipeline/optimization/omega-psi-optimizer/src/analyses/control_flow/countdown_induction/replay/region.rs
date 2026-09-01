//! Optimizer module role: validation leaf. Independent reducible-region reconstruction.

use std::collections::{BTreeMap, BTreeSet};

use super::super::*;

pub(super) fn reconstruct(
    function: &PsiOptimizationFunction,
    component: &OptimizerCycleComponent,
    certificate: &OptimizerUnsignedCountdownRankingCertificate,
) -> Result<LoopRegion, CountedLoopAnalysisError> {
    let blocks = function
        .blocks
        .iter()
        .map(|block| block.id)
        .collect::<BTreeSet<_>>();
    if blocks.len() != function.blocks.len()
        || component.members.is_empty()
        || !component.members.windows(2).all(|pair| pair[0] < pair[1])
        || !component.members.iter().all(|block| blocks.contains(block))
        || !component.members.contains(&certificate.header)
    {
        return Err(shape(function.machine));
    }

    let edges = current_edges(function);
    if edges.iter().any(|edge| !blocks.contains(&edge.target)) {
        return Err(shape(function.machine));
    }
    let members = component.members.iter().copied().collect::<BTreeSet<_>>();
    let internal = edges
        .iter()
        .copied()
        .filter(|edge| members.contains(&edge.source) && members.contains(&edge.target))
        .collect::<Vec<_>>();
    let entries = edges
        .iter()
        .copied()
        .filter(|edge| !members.contains(&edge.source) && members.contains(&edge.target))
        .collect::<Vec<_>>();
    let exits = edges
        .iter()
        .copied()
        .filter(|edge| members.contains(&edge.source) && !members.contains(&edge.target))
        .collect::<Vec<_>>();
    if internal != component.id.internal_edges
        || entries != component.entries
        || exits != component.exits
        || entries.as_slice().first().map(|edge| edge.target) != Some(certificate.header)
        || entries.len() != 1
    {
        return Err(shape(function.machine));
    }

    let successors = successors(&blocks, &edges);
    if !reachable(function.entry, certificate.header, &successors, None)
        || component.members.iter().copied().any(|member| {
            member != certificate.header
                && reachable(
                    function.entry,
                    member,
                    &successors,
                    Some(certificate.header),
                )
        })
    {
        return Err(shape(function.machine));
    }

    Ok(LoopRegion {
        header: Some(certificate.header),
        blocks: component.members.clone(),
        irreducible: false,
    })
}

fn current_edges(function: &PsiOptimizationFunction) -> Vec<CycleComponentEdge> {
    let mut edges = Vec::new();
    for block in &function.blocks {
        let Some(operation) = block.nodes.last().map(|node| &node.operation) else {
            continue;
        };
        match operation {
            O::Jump {
                psi_edge, target, ..
            } => edges.push(CycleComponentEdge {
                edge: *psi_edge,
                source: block.id,
                target: *target,
            }),
            O::Conditional {
                when_true,
                when_false,
                ..
            } => edges.extend([when_true, when_false].map(|successor| CycleComponentEdge {
                edge: successor.psi_edge,
                source: block.id,
                target: successor.target,
            })),
            _ => {}
        }
    }
    edges.sort_unstable();
    edges
}

fn successors(
    blocks: &BTreeSet<BlockId>,
    edges: &[CycleComponentEdge],
) -> BTreeMap<BlockId, Vec<BlockId>> {
    let mut successors = blocks
        .iter()
        .copied()
        .map(|block| (block, Vec::new()))
        .collect::<BTreeMap<_, _>>();
    for edge in edges {
        successors
            .get_mut(&edge.source)
            .expect("every edge source came from a function block")
            .push(edge.target);
    }
    for targets in successors.values_mut() {
        targets.sort_unstable();
        targets.dedup();
    }
    successors
}

fn reachable(
    entry: BlockId,
    target: BlockId,
    successors: &BTreeMap<BlockId, Vec<BlockId>>,
    forbidden: Option<BlockId>,
) -> bool {
    if forbidden == Some(entry) {
        return false;
    }
    let mut visited = BTreeSet::new();
    let mut pending = vec![entry];
    while let Some(block) = pending.pop() {
        if block == target {
            return true;
        }
        if visited.insert(block) {
            pending.extend(
                successors
                    .get(&block)
                    .into_iter()
                    .flatten()
                    .copied()
                    .filter(|successor| Some(*successor) != forbidden),
            );
        }
    }
    false
}

fn shape(machine: MachineId) -> CountedLoopAnalysisError {
    CountedLoopAnalysisError::UnsupportedCountdownShape { machine }
}
