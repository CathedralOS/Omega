//! Independent reachability and dominance reconstruction.

use super::*;

pub(crate) fn independent_reachable_dominators(
    function: &PsiOptimizationFunction,
) -> BTreeMap<BlockId, BTreeSet<BlockId>> {
    let successors = function
        .blocks
        .iter()
        .map(|block| {
            (
                block.id,
                block
                    .nodes
                    .last()
                    .map(|node| node.successors.iter().map(|edge| edge.target).collect())
                    .unwrap_or_default(),
            )
        })
        .collect::<BTreeMap<BlockId, Vec<BlockId>>>();
    let mut reachable = BTreeSet::from([function.entry]);
    let mut frontier = vec![function.entry];
    while let Some(block) = frontier.pop() {
        for successor in successors.get(&block).into_iter().flatten() {
            if reachable.insert(*successor) {
                frontier.push(*successor);
            }
        }
    }
    let mut predecessors = reachable
        .iter()
        .copied()
        .map(|block| (block, BTreeSet::new()))
        .collect::<BTreeMap<_, _>>();
    for (source, targets) in &successors {
        if !reachable.contains(source) {
            continue;
        }
        for target in targets.iter().filter(|target| reachable.contains(target)) {
            predecessors.get_mut(target).unwrap().insert(*source);
        }
    }
    let mut result = reachable
        .iter()
        .copied()
        .map(|block| {
            (
                block,
                if block == function.entry {
                    BTreeSet::from([block])
                } else {
                    reachable.clone()
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
    loop {
        let mut changed = false;
        for block in reachable
            .iter()
            .copied()
            .filter(|block| *block != function.entry)
        {
            let mut incoming = predecessors[&block].iter();
            let mut next = incoming
                .next()
                .map(|predecessor| result[predecessor].clone())
                .unwrap_or_default();
            for predecessor in incoming {
                next = next.intersection(&result[predecessor]).copied().collect();
            }
            next.insert(block);
            if result[&block] != next {
                result.insert(block, next);
                changed = true;
            }
        }
        if !changed {
            return result;
        }
    }
}

pub(crate) fn independently_replacement_dominates_uses(
    function: &PsiOptimizationFunction,
    dominators: &BTreeMap<BlockId, BTreeSet<BlockId>>,
    replacement: ValueId,
    parameter: ValueId,
    scalar_type: ScalarType,
) -> bool {
    if replacement == parameter {
        return false;
    }
    let Some(definition) = function
        .parameters
        .iter()
        .chain(function.blocks.iter().flat_map(|block| {
            block
                .parameters
                .iter()
                .chain(block.nodes.iter().flat_map(|node| &node.definitions))
        }))
        .find(|definition| definition.value == replacement)
    else {
        return false;
    };
    if definition.scalar_type != scalar_type {
        return false;
    }
    function
        .blocks
        .iter()
        .flat_map(|block| block.nodes.iter().flat_map(|node| &node.uses))
        .filter(|use_site| use_site.value == parameter)
        .all(|use_site| match definition.site {
            ValueDefinitionSite::FunctionParameter(_) => true,
            ValueDefinitionSite::BlockParameter {
                block: defining, ..
            } => dominators
                .get(&use_site.block)
                .is_some_and(|rows| rows.contains(&defining)),
            ValueDefinitionSite::Node {
                block: defining,
                node,
            } if defining == use_site.block => node < use_site.node,
            ValueDefinitionSite::Node {
                block: defining, ..
            } => dominators
                .get(&use_site.block)
                .is_some_and(|rows| rows.contains(&defining)),
        })
}
