/*
Dominance over program points. fact_validity.md: "A premise must dominate its
use and remain valid on every path to it." Knowledge never becomes false, so
availability is only this question, answered on the lowered graph: block
dominators from the edge table, and order within a block from the position
of each operation.

The same graph answers where a point lies and which blocks enter a block,
which is what following a place's writes backwards from a read needs
(`places`).
*/

use semantic_vocabulary::{BlockId, MachineId};
use terminal_check_tables::edges::Destination;
use terminal_check_tables::{
    Checked, Construct, MachineTables, ProgramPoint, Refusal, RefusalReason,
};
use terminal_psi::TerminalMachine;

pub(crate) struct Graph {
    machine: MachineId,
    /// `dominators[b - 1][d - 1]`: block `d` dominates block `b`.
    // ponytail: dense n x n sets, fine for one machine's blocks; switch to an
    // immediate-dominator tree if machines grow to thousands of blocks.
    dominators: Vec<Vec<bool>>,
    /// Block and index of each operation, indexed by `OperationId - 1`.
    operations: Vec<Option<(BlockId, usize)>>,
    /// The source block of each edge that enters a block, indexed by
    /// `BlockId - 1`; a block with two edges from one source lists it twice.
    predecessors: Vec<Vec<BlockId>>,
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
        let mut sources = vec![Vec::new(); count];
        let mut tables_edges = Vec::with_capacity(tables.edges.len());
        for (_, edge) in tables.edges.iter() {
            if let Destination::Block(target) = edge.destination {
                predecessors
                    .get_mut(index(target.get()))
                    .ok_or_else(|| refuse("edge enters a block past the end"))?
                    .push(index(edge.source.get()));
                sources[index(target.get())].push(edge.source);
            }
            tables_edges.push((edge.source, edge.destination));
        }
        let entry = index(lowered.entry.get());
        if entry >= count {
            return Err(refuse("entry block is past the end"));
        }
        // Iterative dominator sets. A block nothing enters, other than the
        // entry, is dominated only by itself: the conservative answer.
        let mut dominators: Vec<Vec<bool>> = (0..count)
            .map(|block| {
                if block == entry || predecessors[block].is_empty() {
                    (0..count).map(|other| other == block).collect()
                } else {
                    vec![true; count]
                }
            })
            .collect();
        let mut changed = true;
        while changed {
            changed = false;
            for block in 0..count {
                if block == entry || predecessors[block].is_empty() {
                    continue;
                }
                let mut next = vec![true; count];
                for &predecessor in &predecessors[block] {
                    let from = dominators
                        .get(predecessor)
                        .ok_or_else(|| refuse("edge leaves a block past the end"))?;
                    for (slot, dominates) in next.iter_mut().zip(from) {
                        *slot &= *dominates;
                    }
                }
                next[block] = true;
                if next != dominators[block] {
                    dominators[block] = next;
                    changed = true;
                }
            }
        }
        Ok(Self {
            machine,
            dominators,
            operations,
            predecessors: sources,
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

    /// The source block of every edge that enters `block`.
    pub(crate) fn predecessors(&self, block: BlockId) -> Checked<&[BlockId]> {
        self.predecessors
            .get(index(block.get()))
            .map(Vec::as_slice)
            .ok_or_else(|| self.refuse(Construct::Block(block)))
    }

    /// Block and position of a point: the entry is before every operation,
    /// an operation's normal successor is after it, an edge after them all.
    pub(crate) fn locate(&self, point: ProgramPoint) -> Checked<(BlockId, usize)> {
        match point {
            ProgramPoint::BlockEntry(block) => Ok((block, 0)),
            ProgramPoint::AfterOperation(operation) => self
                .operations
                .get(index(operation.get()))
                .copied()
                .flatten()
                .map(|(block, position)| (block, position + 1))
                .ok_or_else(|| self.refuse(Construct::Operation(operation))),
            ProgramPoint::Edge(edge) => self
                .tables_edges
                .get(index(edge.get()))
                .map(|(source, _)| (*source, usize::MAX))
                .ok_or_else(|| self.refuse(Construct::Edge(edge))),
        }
    }

    fn block_dominates(&self, dominator: BlockId, block: BlockId) -> Checked<bool> {
        self.dominators
            .get(index(block.get()))
            .and_then(|set| set.get(index(dominator.get())))
            .copied()
            .ok_or_else(|| self.refuse(Construct::Block(block)))
    }

    /// Whether every path to `used` passes `available` first.
    pub(crate) fn dominates(&self, available: ProgramPoint, used: ProgramPoint) -> Checked<bool> {
        if available == used {
            return Ok(true);
        }
        let (used_block, used_position) = self.locate(used)?;
        if let ProgramPoint::Edge(edge) = available {
            // An edge dominates what its destination dominates, but only when
            // it is the destination's one way in.
            let (_, destination) = self
                .tables_edges
                .get(index(edge.get()))
                .ok_or_else(|| self.refuse(Construct::Edge(edge)))?;
            let Destination::Block(target) = destination else {
                return Ok(false);
            };
            if self.predecessors.get(index(target.get())).map(Vec::len) != Some(1) {
                return Ok(false);
            }
            return self.block_dominates(*target, used_block);
        }
        let (block, position) = self.locate(available)?;
        if block == used_block {
            return Ok(position <= used_position);
        }
        self.block_dominates(block, used_block)
    }
}
