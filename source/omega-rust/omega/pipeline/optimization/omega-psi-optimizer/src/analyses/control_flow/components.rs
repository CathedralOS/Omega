//! Canonical strongly connected components.

use std::collections::{BTreeMap, BTreeSet};

use omega_optimization_unit::PsiOptimizationUnit;

use super::{StronglyConnectedComponentAnalysis, graph::control_flow};

pub(in crate::analyses) fn block_components(
    unit: &PsiOptimizationUnit,
) -> StronglyConnectedComponentAnalysis {
    let cfg = control_flow(unit);
    StronglyConnectedComponentAnalysis {
        functions: cfg
            .functions
            .iter()
            .map(|function| {
                let graph = function
                    .blocks
                    .iter()
                    .map(|block| (block.block, block.successors.clone()))
                    .collect();
                (function.machine, strongly_connected_components(&graph))
            })
            .collect(),
    }
}
pub(in crate::analyses) fn strongly_connected_components<T>(
    graph: &BTreeMap<T, Vec<T>>,
) -> Vec<Vec<T>>
where
    T: Copy + Ord,
{
    struct Tarjan<T> {
        next: usize,
        indices: BTreeMap<T, usize>,
        lowlinks: BTreeMap<T, usize>,
        stack: Vec<T>,
        on_stack: BTreeSet<T>,
        components: Vec<Vec<T>>,
    }

    fn visit<T>(node: T, graph: &BTreeMap<T, Vec<T>>, state: &mut Tarjan<T>)
    where
        T: Copy + Ord,
    {
        let index = state.next;
        state.next += 1;
        state.indices.insert(node, index);
        state.lowlinks.insert(node, index);
        state.stack.push(node);
        state.on_stack.insert(node);
        for successor in graph.get(&node).into_iter().flatten().copied() {
            if !graph.contains_key(&successor) {
                continue;
            }
            if !state.indices.contains_key(&successor) {
                visit(successor, graph, state);
                state
                    .lowlinks
                    .insert(node, state.lowlinks[&node].min(state.lowlinks[&successor]));
            } else if state.on_stack.contains(&successor) {
                state
                    .lowlinks
                    .insert(node, state.lowlinks[&node].min(state.indices[&successor]));
            }
        }
        if state.lowlinks[&node] == state.indices[&node] {
            let mut component = Vec::new();
            loop {
                let member = state.stack.pop().expect("SCC root remains on stack");
                state.on_stack.remove(&member);
                component.push(member);
                if member == node {
                    break;
                }
            }
            component.sort_unstable();
            state.components.push(component);
        }
    }

    let mut state = Tarjan {
        next: 0,
        indices: BTreeMap::new(),
        lowlinks: BTreeMap::new(),
        stack: Vec::new(),
        on_stack: BTreeSet::new(),
        components: Vec::new(),
    };
    for node in graph.keys().copied() {
        if !state.indices.contains_key(&node) {
            visit(node, graph, &mut state);
        }
    }
    state.components.sort();
    state.components
}
