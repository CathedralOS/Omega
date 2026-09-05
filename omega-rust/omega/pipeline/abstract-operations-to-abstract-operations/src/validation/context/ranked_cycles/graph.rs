//! Optimizer module role: projection leaf. Canonical edge graphs from Terminal and current optimizer bodies.

use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CanonicalControlGraph {
    pub(super) machine: MachineId,
    pub(super) blocks: BTreeSet<BlockId>,
    pub(super) edges: Vec<CycleComponentEdge>,
}

impl CanonicalControlGraph {
    pub(super) fn successors(&self) -> BTreeMap<BlockId, Vec<BlockId>> {
        let mut graph = self
            .blocks
            .iter()
            .copied()
            .map(|block| (block, Vec::new()))
            .collect::<BTreeMap<_, _>>();
        for edge in &self.edges {
            if let Some(successors) = graph.get_mut(&edge.source) {
                successors.push(edge.target);
            }
        }
        for successors in graph.values_mut() {
            successors.sort_unstable();
            successors.dedup();
        }
        graph
    }
}

pub(super) fn terminal_graph(machine: &terminal_psi::TerminalMachine) -> CanonicalControlGraph {
    let mut edges = Vec::new();
    for block in &machine.blocks {
        match &block.terminator {
            terminal_psi::Terminator::Jump { edge, target, .. } => {
                edges.push(CycleComponentEdge {
                    edge: *edge,
                    source: block.id,
                    target: *target,
                });
            }
            terminal_psi::Terminator::Conditional {
                when_true,
                when_false,
                ..
            } => {
                edges.extend([when_true, when_false].map(|successor| CycleComponentEdge {
                    edge: successor.edge,
                    source: block.id,
                    target: successor.target,
                }));
            }
            _ => {}
        }
    }
    edges.sort_unstable();
    CanonicalControlGraph {
        machine: machine.id,
        blocks: machine.blocks.iter().map(|block| block.id).collect(),
        edges,
    }
}

pub(super) fn optimization_graph(function: &PsiOptimizationFunction) -> CanonicalControlGraph {
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
    CanonicalControlGraph {
        machine: function.machine,
        blocks: function.blocks.iter().map(|block| block.id).collect(),
        edges,
    }
}
