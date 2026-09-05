//! Forward dominators and all-represented-exit post-dominators.

use std::collections::{BTreeMap, BTreeSet};

use optimization_unit::PsiOptimizationUnit;
use semantic_vocabulary::BlockId;

use super::{DominatorAnalysis, FunctionControlFlow, graph::control_flow};

pub(in crate::analyses) fn dominators(
    unit: &PsiOptimizationUnit,
    reverse: bool,
) -> DominatorAnalysis {
    let cfg = control_flow(unit);
    DominatorAnalysis {
        functions: cfg
            .functions
            .iter()
            .map(|function| {
                (
                    function.machine,
                    fixed_point_dominators(function, reverse)
                        .into_iter()
                        .map(|(block, set)| (block, set.into_iter().collect()))
                        .collect(),
                )
            })
            .collect(),
    }
}

pub(in crate::analyses) fn fixed_point_dominators(
    function: &FunctionControlFlow,
    reverse: bool,
) -> BTreeMap<BlockId, BTreeSet<BlockId>> {
    let rows = function
        .blocks
        .iter()
        .map(|block| (block.block, block))
        .collect::<BTreeMap<_, _>>();
    let considered = function
        .blocks
        .iter()
        .filter(|block| block.reachable)
        .map(|block| block.block)
        .collect::<BTreeSet<_>>();
    let roots = if reverse {
        function
            .blocks
            .iter()
            .filter(|block| block.reachable && !block.exits.is_empty())
            .map(|block| block.block)
            .collect::<BTreeSet<_>>()
    } else {
        BTreeSet::from([function.entry])
    };
    let mut result = considered
        .iter()
        .copied()
        .map(|block| {
            let initial = if roots.contains(&block) {
                BTreeSet::from([block])
            } else {
                considered.clone()
            };
            (block, initial)
        })
        .collect::<BTreeMap<_, _>>();
    loop {
        let mut changed = false;
        for block in considered
            .iter()
            .copied()
            .filter(|block| !roots.contains(block))
        {
            let adjacent = if reverse {
                &rows[&block].successors
            } else {
                &rows[&block].predecessors
            };
            let mut incoming = adjacent.iter().filter_map(|other| result.get(other));
            let mut next = incoming.next().cloned().unwrap_or_default();
            for set in incoming {
                next = next.intersection(set).copied().collect();
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
