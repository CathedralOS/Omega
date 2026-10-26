//! Source-independent analysis of a Terminal machine's control graph:
//! successors, cyclic components, dominators, feedback edges, and the
//! topology identities of control cycles. Producers, the optimizer, fuel
//! certification and the verifier all read the same derivation.

use std::collections::{BTreeMap, BTreeSet};

use semantic_vocabulary::{BlockId, EdgeId};
use semantic_vocabulary::{CycleComponentId, PsiSemanticId};
use terminal_psi::{TerminalMachine, TerminalNaturalCycle};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlGraphError {
    DuplicateBlock(BlockId),
    MissingEntry(BlockId),
    UnknownTarget(BlockId),
    UnreachableBlock(BlockId),
}

pub fn successors(machine: &TerminalMachine) -> BTreeMap<BlockId, Vec<(EdgeId, BlockId)>> {
    machine
        .blocks
        .iter()
        .map(|block| {
            let edges = block.terminator.successor_edges();
            (block.id, edges)
        })
        .collect()
}

/// Failure to establish an exact, closed module control graph is not evidence
/// that an invocation cannot return.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionGraphError {
    UnknownMachine(semantic_vocabulary::MachineId),
    AmbiguousMachine(semantic_vocabulary::MachineId),
    InvalidControl {
        machine: semantic_vocabulary::MachineId,
        error: ControlGraphError,
    },
}

/// Reconstruct potential normal completion from actual module control, never
/// from a caller's marker or the callee's crash ceiling. A closed cycle has no
/// normal exit, but supplies no termination, disposal or abandonment authority.
/// Ordinary prefix calls remain conservatively returning; an explicit
/// no-normal invocation transfers this query to its exact callee entry.
pub fn normal_completion_possible(
    module: &terminal_psi::TerminalModule,
    initial: semantic_vocabulary::MachineId,
) -> Result<bool, CompletionGraphError> {
    use terminal_psi::Terminator;
    let exact_machine = |identity| {
        let mut candidates = module
            .machines
            .iter()
            .filter(|machine| machine.id == identity);
        let machine = candidates
            .next()
            .ok_or(CompletionGraphError::UnknownMachine(identity))?;
        if candidates.next().is_some() {
            return Err(CompletionGraphError::AmbiguousMachine(identity));
        }
        Ok(machine)
    };
    let entry = exact_machine(initial)?.entry;
    let mut validated = BTreeSet::new();
    let mut visited = BTreeSet::new();
    let mut pending = vec![(initial, entry)];
    while let Some((machine_id, block_id)) = pending.pop() {
        if !visited.insert((machine_id, block_id)) {
            continue;
        }
        let machine = exact_machine(machine_id)?;
        if validated.insert(machine_id) {
            validate_control_graph(machine).map_err(|error| {
                CompletionGraphError::InvalidControl {
                    machine: machine_id,
                    error,
                }
            })?;
        }
        let block = machine
            .block(&block_id)
            .ok_or(CompletionGraphError::InvalidControl {
                machine: machine_id,
                error: ControlGraphError::UnknownTarget(block_id),
            })?;
        match &block.terminator {
            Terminator::Return { .. }
            | Terminator::ReturnUnit { .. }
            | Terminator::ReturnStructural { .. } => return Ok(true),
            Terminator::InvokeNoNormal { callee, .. } => {
                let target = exact_machine(*callee)?;
                pending.push((*callee, target.entry));
            }
            Terminator::Crash { .. } | Terminator::ExternalCompletion { .. } => {}
            Terminator::Jump { .. }
            | Terminator::Conditional { .. }
            | Terminator::StructuralCase { .. } => {
                pending.extend(
                    block
                        .terminator
                        .successor_targets()
                        .into_iter()
                        .map(|target| (machine_id, target)),
                );
            }
        }
    }
    Ok(false)
}

/// Iterative DFS returning blocks in first-finished order — the shared
/// pending-stack walk behind `cyclic_components` and `reverse_postorder`.
/// `successor(block, position)` projects the block's edge table.
pub fn finished_order(
    entry: BlockId,
    successor: impl Fn(BlockId, usize) -> Option<BlockId>,
) -> Vec<BlockId> {
    let mut entered = BTreeSet::from([entry]);
    let mut finished = Vec::new();
    let mut pending = vec![(entry, 0usize)];
    while let Some((block, position)) = pending.last_mut() {
        let Some(target) = successor(*block, *position) else {
            finished.push(*block);
            pending.pop();
            continue;
        };
        *position += 1;
        if entered.insert(target) {
            pending.push((target, 0));
        }
    }
    finished
}

/// Complete cyclic components in canonical member order. Targets and entry
/// must have passed ordinary graph validation before this private query.
pub fn cyclic_components(machine: &TerminalMachine) -> Vec<Vec<BlockId>> {
    let outgoing = successors(machine);
    let finished = finished_order(machine.entry, |block, position| {
        outgoing[&block].get(position).map(|(_, target)| *target)
    });
    let mut incoming = outgoing
        .keys()
        .map(|block| (*block, Vec::new()))
        .collect::<BTreeMap<_, _>>();
    for (block, edges) in &outgoing {
        for (_, target) in edges {
            incoming
                .get_mut(target)
                .expect("validated target")
                .push(*block);
        }
    }
    let mut assigned = BTreeSet::new();
    let mut components = Vec::new();
    for root in finished.into_iter().rev() {
        if !assigned.insert(root) {
            continue;
        }
        let mut component = vec![root];
        let mut pending = vec![root];
        while let Some(block) = pending.pop() {
            for predecessor in &incoming[&block] {
                if assigned.insert(*predecessor) {
                    component.push(*predecessor);
                    pending.push(*predecessor);
                }
            }
        }
        component.sort();
        if component.len() > 1 || outgoing[&root].iter().any(|(_, target)| *target == root) {
            components.push(component);
        }
    }
    components.sort();
    components
}

/// Canonical topology for consumers that need cyclic members without proof or
/// progress authority.
pub fn control_cycle_members(
    machine: &TerminalMachine,
) -> Result<Vec<Vec<BlockId>>, ControlGraphError> {
    validate_control_graph(machine)?;
    Ok(cyclic_components(machine))
}

pub fn validate_control_graph(machine: &TerminalMachine) -> Result<(), ControlGraphError> {
    let mut blocks = BTreeSet::new();
    for block in &machine.blocks {
        if !blocks.insert(block.id) {
            return Err(ControlGraphError::DuplicateBlock(block.id));
        }
    }
    if !blocks.contains(&machine.entry) {
        return Err(ControlGraphError::MissingEntry(machine.entry));
    }
    let outgoing = successors(machine);
    for (_, target) in outgoing.values().flatten() {
        if !blocks.contains(target) {
            return Err(ControlGraphError::UnknownTarget(*target));
        }
    }
    let mut reached = BTreeSet::new();
    let mut pending = vec![machine.entry];
    while let Some(block) = pending.pop() {
        if reached.insert(block) {
            pending.extend(outgoing[&block].iter().map(|(_, target)| *target));
        }
    }
    if let Some(block) = blocks.difference(&reached).next() {
        return Err(ControlGraphError::UnreachableBlock(*block));
    }
    Ok(())
}

/// Reachable blocks in reverse postorder: every dominator precedes each block
/// it dominates, so a scan in this order meets every definition before its
/// dominated uses.
pub fn reverse_postorder(machine: &TerminalMachine) -> Vec<BlockId> {
    let outgoing = successors(machine);
    reverse_postorder_from_successors(machine.entry, &outgoing)
}

fn reverse_postorder_from_successors(
    entry: BlockId,
    outgoing: &BTreeMap<BlockId, Vec<(EdgeId, BlockId)>>,
) -> Vec<BlockId> {
    let mut finished = finished_order(entry, |block, position| {
        outgoing[&block].get(position).map(|(_, target)| *target)
    });
    finished.reverse();
    finished
}

/// One interval per block replaces the expanded set of all its ancestors.
/// Block identities are sparse; only the private reverse-postorder positions
/// index contiguous working storage.
pub struct DominatorTree {
    positions: BTreeMap<BlockId, usize>,
    nodes: Vec<DominatorNode>,
}

struct DominatorNode {
    traversal_start: usize,
    traversal_end: usize,
    depth: usize,
}

impl DominatorTree {
    pub fn dominates(&self, definition: BlockId, use_block: BlockId) -> bool {
        let (Some(&definition), Some(&use_block)) = (
            self.positions.get(&definition),
            self.positions.get(&use_block),
        ) else {
            return false;
        };
        let definition = &self.nodes[definition];
        let use_block = &self.nodes[use_block];
        definition.traversal_start <= use_block.traversal_start
            && use_block.traversal_start < definition.traversal_end
    }

    /// Number of dominators including the block itself; the entry has depth 1.
    pub fn depth(&self, block: BlockId) -> Option<usize> {
        self.positions
            .get(&block)
            .map(|&position| self.nodes[position].depth)
    }
}

/// A cyclic block definition is available only when it dominates the use in
/// the full graph, not merely a traversal with backedges removed. Entry,
/// targets, unique identities and reachability must already be validated.
pub fn dominators(machine: &TerminalMachine) -> DominatorTree {
    let outgoing = successors(machine);
    let ordered = reverse_postorder_from_successors(machine.entry, &outgoing);
    let positions = ordered
        .iter()
        .enumerate()
        .map(|(position, block)| (*block, position))
        .collect::<BTreeMap<_, _>>();
    let mut predecessors = vec![Vec::new(); ordered.len()];
    for (block, edges) in &outgoing {
        for (_, target) in edges {
            predecessors[positions[target]].push(positions[block]);
        }
    }

    // Reverse-postorder fixed point. Known parent links always move toward
    // smaller positions, so intersecting two chains terminates even while
    // cyclic and irreducible predecessors still refine the provisional tree.
    let mut immediate = vec![usize::MAX; ordered.len()];
    immediate[0] = 0;
    loop {
        let mut changed = false;
        for position in 1..ordered.len() {
            let mut common = usize::MAX;
            for &predecessor in &predecessors[position] {
                if immediate[predecessor] == usize::MAX {
                    continue;
                }
                common = if common == usize::MAX {
                    predecessor
                } else {
                    intersect_dominator_chains(common, predecessor, &immediate)
                };
            }
            // The DFS tree supplies an earlier predecessor for every
            // reachable non-entry block, including on the first sweep.
            debug_assert!(common < position);
            if immediate[position] != common {
                immediate[position] = common;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }

    let mut children = vec![Vec::new(); ordered.len()];
    for position in 1..ordered.len() {
        children[immediate[position]].push(position);
    }
    let mut nodes = (0..ordered.len())
        .map(|_| DominatorNode {
            traversal_start: 0,
            traversal_end: 0,
            depth: 0,
        })
        .collect::<Vec<_>>();
    nodes[0].depth = 1;
    let mut next_start = 1;
    let mut pending = vec![(0, 0)];
    while let Some((position, child_position)) = pending.last_mut() {
        let Some(&child) = children[*position].get(*child_position) else {
            nodes[*position].traversal_end = next_start;
            pending.pop();
            continue;
        };
        *child_position += 1;
        nodes[child].depth = nodes[*position].depth + 1;
        nodes[child].traversal_start = next_start;
        next_start += 1;
        pending.push((child, 0));
    }
    DominatorTree { positions, nodes }
}

fn intersect_dominator_chains(mut first: usize, mut second: usize, immediate: &[usize]) -> usize {
    while first != second {
        if first > second {
            first = immediate[first];
        } else {
            second = immediate[second];
        }
    }
    first
}

/// Topology identity has no producer-supplied component key. Rank bindings
/// separately enter the question commitment, so changing a witness invalidates
/// its comparison evidence even when the component topology is unchanged.
pub fn control_cycle_identity(
    machine: &TerminalMachine,
    component: &TerminalNaturalCycle,
) -> CycleComponentId {
    let edges = component
        .edges
        .iter()
        .map(|edge| (edge.edge, edge.source, edge.target));
    topology_identity(machine, edges)
}

fn topology_identity(
    machine: &TerminalMachine,
    edges: impl IntoIterator<Item = (EdgeId, BlockId, BlockId)>,
) -> CycleComponentId {
    let mut digest =
        crate::digest_encoder::DomainDigestEncoder::new(b"psi.control-cycle.topology.v1\0");
    digest.u64(machine.id.get());
    for (edge, source, target) in edges {
        digest.u64(edge.get());
        digest.u64(source.get());
        digest.u64(target.get());
    }
    semantic_id(&digest.finish().into())
}

fn semantic_id<Identity: PsiSemanticId>(digest: &[u8; 32]) -> Identity {
    let raw = u64::from_le_bytes(digest[..8].try_into().expect("SHA-256 prefix")) | 1;
    Identity::new(raw).expect("the low bit establishes a nonzero semantic identity")
}
