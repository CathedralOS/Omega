//! Optimizer module role: semantic leaf. Canonical members, edge identity, entries, and exits for each SCC.

use super::*;

pub(super) fn derive_components(
    graph: &graph::CanonicalControlGraph,
) -> Vec<OptimizerCycleComponent> {
    let mut components = components::cyclic_components(&graph.successors())
        .into_iter()
        .map(|members| derive_component(graph, members))
        .collect::<Vec<_>>();
    components.sort_by(|left, right| left.id.cmp(&right.id));
    components
}

fn derive_component(
    graph: &graph::CanonicalControlGraph,
    members: Vec<BlockId>,
) -> OptimizerCycleComponent {
    let member_set = members.iter().copied().collect::<BTreeSet<_>>();
    let mut internal_edges = Vec::new();
    let mut entries = Vec::new();
    let mut exits = Vec::new();
    for edge in &graph.edges {
        match (
            member_set.contains(&edge.source),
            member_set.contains(&edge.target),
        ) {
            (true, true) => internal_edges.push(*edge),
            (false, true) => entries.push(*edge),
            (true, false) => exits.push(*edge),
            (false, false) => {}
        }
    }
    OptimizerCycleComponent {
        id: CycleComponentId {
            machine: graph.machine,
            internal_edges,
        },
        members,
        entries,
        exits,
    }
}
