//! The one strongly-connected-component partition for the machine and
//! transition graphs, over adjacency lists of program positions.
//!
//! Call-cycle judgement, runtime rank relevance and state write-equation
//! finiteness all ask this same question. Each used to run its own algorithm,
//! so a repair to one left the other answering differently on the same graph.
//!
//! Two forms of the single partition are published: the member roster
//! (`strongly_connected_components`) for consumers that read a component's
//! members, and the node-addressed form (`component_ids`) for consumers that
//! only compare two nodes. Neither grants authority; both are pure functions
//! of the adjacency list.

/// Kosaraju's two passes: finish order over the adjacency list, then the
/// reversed list. A component is published before every component reachable
/// from it, and its members ascend, so a component's first member is its
/// least position. Every position lies in exactly one component, including a
/// self edge, which keeps its node in its own component.
pub(in crate::machine_calls) fn strongly_connected_components(
    adjacency: &[Vec<usize>],
) -> Vec<Vec<usize>> {
    fn finish_order(
        node: usize,
        adjacency: &[Vec<usize>],
        visited: &mut [bool],
        order: &mut Vec<usize>,
    ) {
        if std::mem::replace(&mut visited[node], true) {
            return;
        }
        for next in &adjacency[node] {
            finish_order(*next, adjacency, visited, order);
        }
        order.push(node);
    }

    fn collect_component(
        node: usize,
        reverse: &[Vec<usize>],
        visited: &mut [bool],
        component: &mut Vec<usize>,
    ) {
        if std::mem::replace(&mut visited[node], true) {
            return;
        }
        component.push(node);
        for next in &reverse[node] {
            collect_component(*next, reverse, visited, component);
        }
    }

    let mut order = Vec::with_capacity(adjacency.len());
    let mut visited = vec![false; adjacency.len()];
    for node in 0..adjacency.len() {
        finish_order(node, adjacency, &mut visited, &mut order);
    }
    let mut reverse = vec![Vec::new(); adjacency.len()];
    for (source, targets) in adjacency.iter().enumerate() {
        for target in targets {
            reverse[*target].push(source);
        }
    }
    for targets in &mut reverse {
        targets.sort_unstable();
        targets.dedup();
    }
    visited.fill(false);
    let mut components = Vec::new();
    while let Some(node) = order.pop() {
        if visited[node] {
            continue;
        }
        let mut component = Vec::new();
        collect_component(node, &reverse, &mut visited, &mut component);
        component.sort_unstable();
        components.push(component);
    }
    components
}

/// The same partition addressed by node: `ids[source] == ids[target]` exactly
/// when both positions lie in one strongly connected component. The numbering
/// is the roster's order, and every position is covered.
pub(in crate::machine_calls) fn component_ids(adjacency: &[Vec<usize>]) -> Vec<usize> {
    let mut ids = vec![usize::MAX; adjacency.len()];
    for (component, members) in strongly_connected_components(adjacency)
        .into_iter()
        .enumerate()
    {
        for member in members {
            ids[member] = component;
        }
    }
    ids
}
