/*
Premises must dominate their uses on every path (fact_validity.md). Prepare
the table-edge topology once, retain a linear-size dominance forest, and keep
operation positions for ordering within a block. Extra table edges participate
even when no terminator owns them: they conservatively remove availability.
*/

use semantic_vocabulary::{BlockId, EdgeId, MachineId};
use terminal_check_tables::edges::Destination;
use terminal_check_tables::{
    Checked, Construct, MachineTables, ProgramPoint, Refusal, RefusalReason,
};
use terminal_psi::TerminalMachine;
use terminal_semantics::control_graph::{DominanceInterval, dominance_forest};

#[path = "graph/roots.rs"]
mod roots;
#[cfg(test)]
#[path = "graph/tests.rs"]
mod tests;

pub(crate) struct Graph {
    machine: MachineId,
    dominance: Vec<DominanceInterval>,
    /// Block and index of each operation, indexed by `OperationId - 1`.
    operations: Vec<Option<(BlockId, usize)>>,
    /// Every incoming edge is counted, including repeated sources.
    predecessors: Vec<Vec<BlockId>>,
    ordered: Vec<BlockId>,
    entry: BlockId,
    tables_edges: Vec<(BlockId, Destination)>,
}

fn index(raw: u64) -> usize {
    usize::try_from(raw - 1).unwrap_or(usize::MAX)
}

impl Graph {
    pub(crate) fn new(
        machine: MachineId,
        lowered: &TerminalMachine,
        tables: &MachineTables,
    ) -> Checked<Self> {
        let refuse = |detail: &str| Refusal {
            reason: RefusalReason::ForeignIdentity,
            machine,
            concerns: Construct::Machine,
            detail: detail.to_owned(),
        };
        let count = lowered.blocks.len();
        for (position, block) in lowered.blocks.iter().enumerate() {
            if index(block.id.get()) != position {
                return Err(refuse("blocks are not dense in identity order"));
            }
        }
        if index(lowered.entry.get()) >= count {
            return Err(refuse("entry block is past the end"));
        }
        let mut operations: Vec<Option<(BlockId, usize)>> = Vec::new();
        for block in &lowered.blocks {
            for (position, operation) in block.operations.iter().enumerate() {
                let slot = index(operation.id.get());
                if operations.len() <= slot {
                    operations.resize(slot + 1, None);
                }
                if operations[slot].replace((block.id, position)).is_some() {
                    return Err(refuse("an operation appears twice"));
                }
            }
        }
        let mut predecessors = vec![Vec::new(); count];
        let mut successors = vec![Vec::new(); count];
        let mut tables_edges = Vec::with_capacity(tables.edges.len());
        for (_, edge) in tables.edges.iter() {
            let outgoing = successors
                .get_mut(index(edge.source.get()))
                .ok_or_else(|| refuse("edge leaves a block past the end"))?;
            if let Destination::Block(target) = edge.destination {
                predecessors
                    .get_mut(index(target.get()))
                    .ok_or_else(|| refuse("edge enters a block past the end"))?
                    .push(edge.source);
                outgoing.push(target);
            }
            tables_edges.push((edge.source, edge.destination));
        }
        let ordered = ordered_blocks(
            std::iter::once(lowered.entry).chain(
                (1..=count).map(|raw| BlockId::new(raw as u64).expect("dense block identity")),
            ),
            &successors,
        );
        let roots = roots::virtual_entries(lowered.entry, &predecessors, &successors, &ordered);
        // The legacy traversal may start inside a disconnected downstream
        // component before visiting its source. Keep that consumer order, but
        // dominance needs the actual reverse postorder of the virtual entries.
        let disconnected_order = (roots.iter().filter(|&&root| root).count() > 1).then(|| {
            ordered_blocks(
                roots
                    .iter()
                    .enumerate()
                    .filter(|(_, root)| **root)
                    .map(|(slot, _)| BlockId::new(slot as u64 + 1).expect("dense block identity")),
                &successors,
            )
        });
        let dominance_order = disconnected_order.as_deref().unwrap_or(&ordered);
        let incoming_positions: Vec<Vec<usize>> = predecessors
            .iter()
            .map(|incoming| incoming.iter().map(|block| index(block.get())).collect())
            .collect();
        let ordered_positions: Vec<_> = dominance_order
            .iter()
            .map(|block| index(block.get()))
            .collect();
        let dominance = dominance_forest(&incoming_positions, &ordered_positions, &roots)
            .ok_or_else(|| refuse("control graph has no complete dominance forest"))?;
        Ok(Self {
            machine,
            dominance,
            operations,
            predecessors,
            ordered,
            entry: lowered.entry,
            tables_edges,
        })
    }

    fn refuse(&self, concerns: Construct) -> Refusal {
        Refusal {
            reason: RefusalReason::ForeignIdentity,
            machine: self.machine,
            concerns,
            detail: String::from("program point is not in this machine"),
        }
    }

    pub(crate) fn entry(&self) -> BlockId {
        self.entry
    }
    pub(crate) fn block_count(&self) -> usize {
        self.predecessors.len()
    }
    /// One past the largest operation identity the machine holds.
    pub(crate) fn operation_count(&self) -> usize {
        self.operations.len()
    }
    /// The source block of every edge that enters `block`.
    pub(crate) fn predecessors(&self, block: BlockId) -> Checked<&[BlockId]> {
        self.predecessors
            .get(index(block.get()))
            .map(Vec::as_slice)
            .ok_or_else(|| self.refuse(Construct::Block(block)))
    }
    pub(crate) fn edge_destination(&self, edge: EdgeId) -> Checked<Destination> {
        self.tables_edges
            .get(index(edge.get()))
            .map(|(_, destination)| *destination)
            .ok_or_else(|| self.refuse(Construct::Edge(edge)))
    }

    /// Preserve the entry-first DFS forest's reverse finish order. Consumers
    /// borrow the prepared order instead of reconstructing table-edge topology.
    pub(crate) fn reverse_postorder(&self) -> Checked<&[BlockId]> {
        Ok(&self.ordered)
    }

    /// Entry is before every operation, a normal successor is after its
    /// operation, and an outgoing edge is after them all.
    pub(crate) fn locate(&self, point: ProgramPoint) -> Checked<(BlockId, usize)> {
        let (block, position, _) = self.locate_with_dominance(point)?;
        Ok((block, position))
    }

    /// Keep the checked block association through the query instead of
    /// resolving each interval again after locating the two program points.
    fn locate_with_dominance(
        &self,
        point: ProgramPoint,
    ) -> Checked<(BlockId, usize, &DominanceInterval)> {
        let (block, position) = match point {
            ProgramPoint::BlockEntry(block) => (block, 0),
            ProgramPoint::AfterOperation(operation) => self
                .operations
                .get(index(operation.get()))
                .copied()
                .flatten()
                .map(|(block, position)| (block, position + 1))
                .ok_or_else(|| self.refuse(Construct::Operation(operation)))?,
            ProgramPoint::Edge(edge) => self
                .tables_edges
                .get(index(edge.get()))
                .map(|(source, _)| (*source, usize::MAX))
                .ok_or_else(|| self.refuse(Construct::Edge(edge)))?,
        };
        let dominance = self
            .dominance
            .get(index(block.get()))
            .ok_or_else(|| self.refuse(Construct::Block(block)))?;
        Ok((block, position, dominance))
    }

    /// Whether every path to `used` passes `available` first.
    pub(crate) fn dominates(&self, available: ProgramPoint, used: ProgramPoint) -> Checked<bool> {
        let (used_block, used_position, used_dominance) = self.locate_with_dominance(used)?;
        let (block, position, dominance) = self.locate_with_dominance(available)?;
        if available == used {
            return Ok(true);
        }
        if let ProgramPoint::Edge(edge) = available {
            let (_, destination) = self
                .tables_edges
                .get(index(edge.get()))
                .ok_or_else(|| self.refuse(Construct::Edge(edge)))?;
            let Destination::Block(target) = destination else {
                return Ok(false);
            };
            // A real edge supplies a premise only if it is the sole way in.
            // Virtual entries also count: a backedge cannot establish a fact
            // on the first visit to the machine or a disconnected source SCC.
            if self.predecessors[index(target.get())].len() != 1
                || self.dominance[index(target.get())].depth() == 1
            {
                return Ok(false);
            }
            return Ok(self.dominance[index(target.get())].dominates(used_dominance));
        }
        if block == used_block {
            return Ok(position <= used_position);
        }
        Ok(dominance.dominates(used_dominance))
    }
}

fn ordered_blocks(
    roots: impl Iterator<Item = BlockId>,
    successors: &[Vec<BlockId>],
) -> Vec<BlockId> {
    let mut visited = vec![false; successors.len()];
    let mut postorder = Vec::with_capacity(successors.len());
    let mut pending = Vec::new();
    for root in roots {
        if std::mem::replace(&mut visited[index(root.get())], true) {
            continue;
        }
        pending.push((root, 0));
        while let Some((block, next)) = pending.last_mut() {
            if let Some(&successor) = successors[index(block.get())].get(*next) {
                *next += 1;
                if !std::mem::replace(&mut visited[index(successor.get())], true) {
                    pending.push((successor, 0));
                }
            } else {
                postorder.push(*block);
                pending.pop();
            }
        }
    }
    postorder.reverse();
    postorder
}
