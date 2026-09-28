//! Reducible and irreducible cyclic-region classification.

use std::collections::{BTreeMap, BTreeSet};

use optimization_unit::PsiOptimizationUnit;

use super::{
    LoopAnalysis, LoopRegion, components::strongly_connected_components,
    dominance::fixed_point_dominators, graph::control_flow,
};

pub(in crate::analyses) fn loops(unit: &PsiOptimizationUnit) -> LoopAnalysis {
    let cfg = control_flow(unit);
    LoopAnalysis {
        functions: cfg
            .functions
            .iter()
            .map(|function| {
                let graph = function
                    .blocks
                    .iter()
                    .map(|block| (block.block, block.successors.clone()))
                    .collect::<BTreeMap<_, _>>();
                let predecessors = function
                    .blocks
                    .iter()
                    .map(|block| (block.block, block.predecessors.clone()))
                    .collect::<BTreeMap<_, _>>();
                let dominators = fixed_point_dominators(function, false);
                let regions = strongly_connected_components(&graph)
                    .into_iter()
                    .filter(|component| {
                        component.len() > 1
                            || component
                                .first()
                                .is_some_and(|block| graph[block].contains(block))
                    })
                    .map(|component| {
                        let members = component.iter().copied().collect::<BTreeSet<_>>();
                        let entries = component
                            .iter()
                            .copied()
                            .filter(|block| {
                                predecessors[block]
                                    .iter()
                                    .any(|predecessor| !members.contains(predecessor))
                            })
                            .collect::<Vec<_>>();
                        let header = component.iter().copied().find(|candidate| {
                            component.iter().all(|block| {
                                dominators
                                    .get(block)
                                    .is_some_and(|set| set.contains(candidate))
                            })
                        });
                        LoopRegion {
                            header,
                            blocks: component,
                            irreducible: entries.len() > 1 || header.is_none(),
                        }
                    })
                    .collect();
                (function.machine, regions)
            })
            .collect(),
    }
}
