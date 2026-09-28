//! Optimizer module role: semantic leaf. Canonical members, edge identity, entries, and exits for each SCC.

use super::super::super::BTreeSet;
use semantic_vocabulary::BlockId;

use super::{
    CycleComponentId, OptimizationUnitValidationError, OptimizerCycleComponent, components, graph,
};

/// Components of the current optimizer body. Member identity is
/// reconstructed privately over the canonical edge projection so a
/// transformed unit cannot carry roster authority forward.
pub(super) fn derive_components(
    graph: &graph::CanonicalControlGraph,
) -> Vec<OptimizerCycleComponent> {
    assemble(graph, components::cyclic_components(&graph.successors()))
}

/// Components of one authenticated Terminal machine. Member identity is the
/// verifier's canonical cyclic-component surface — the exact topology every
/// retained `Natural` ranking row was validated against — so the roster loop
/// consumers iterate is the validated Terminal SCC evidence itself rather
/// than a second private derivation. Edge identity still comes from the
/// canonical edge projection: classifying each edge against the member set
/// names internal edges, entries, and exits without re-deriving components.
pub(super) fn derive_terminal_components(
    machine: &terminal_psi::TerminalMachine,
) -> Result<Vec<OptimizerCycleComponent>, OptimizationUnitValidationError> {
    let members = terminal_verifier::control_cycle_members(machine).map_err(|_| {
        OptimizationUnitValidationError::RankedCycleTopologyMismatch {
            machine: machine.id,
        }
    })?;
    Ok(assemble(&graph::terminal_graph(machine), members))
}

fn assemble(
    graph: &graph::CanonicalControlGraph,
    members: Vec<Vec<BlockId>>,
) -> Vec<OptimizerCycleComponent> {
    let mut components = members
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
