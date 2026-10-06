//! Source-independent analysis of a Terminal machine's control graph:
//! successors, cyclic components, dominators, feedback edges, and the
//! topology identities of control cycles. Producers, the optimizer, fuel
//! certification and the verifier all read the same derivation.

use std::collections::{BTreeMap, BTreeSet};

use semantic_vocabulary::{BlockId, EdgeId};
use semantic_vocabulary::{CycleComponentId, PsiSemanticId};
use sha2::{Digest, Sha256};
use terminal_psi::{TerminalMachine, TerminalNaturalCycle, Terminator};

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
            let edges = match &block.terminator {
                Terminator::Jump { edge, target, .. } => vec![(*edge, *target)],
                Terminator::Conditional {
                    when_true,
                    when_false,
                    ..
                } => vec![
                    (when_true.edge, when_true.target),
                    (when_false.edge, when_false.target),
                ],
                Terminator::StructuralCase { cases, .. } => cases
                    .iter()
                    .map(|successor| (successor.edge, successor.target))
                    .collect(),
                Terminator::Return { .. }
                | Terminator::ReturnUnit { .. }
                | Terminator::ReturnStructural { .. }
                | Terminator::Crash { .. } => Vec::new(),
            };
            (block.id, edges)
        })
        .collect()
}

/// Complete cyclic components in canonical member order. Targets and entry
/// must have passed ordinary graph validation before this private query.
pub fn cyclic_components(machine: &TerminalMachine) -> Vec<Vec<BlockId>> {
    let outgoing = successors(machine);
    let mut entered = BTreeSet::from([machine.entry]);
    let mut finished = Vec::new();
    let mut pending = vec![(machine.entry, 0usize)];
    while let Some((block, position)) = pending.last_mut() {
        let Some((_, target)) = outgoing[block].get(*position).copied() else {
            finished.push(*block);
            pending.pop();
            continue;
        };
        *position += 1;
        if entered.insert(target) {
            pending.push((target, 0));
        }
    }
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
    let mut entered = BTreeSet::from([entry]);
    let mut finished = Vec::new();
    let mut pending = vec![(entry, 0usize)];
    while let Some((block, position)) = pending.last_mut() {
        let Some((_, target)) = outgoing[block].get(*position).copied() else {
            finished.push(*block);
            pending.pop();
            continue;
        };
        *position += 1;
        if entered.insert(target) {
            pending.push((target, 0));
        }
    }
    finished.reverse();
    finished
}

/// One interval per block replaces the expanded set of all its ancestors.
/// Block identities are sparse; only the private reverse-postorder positions
/// index contiguous working storage.
pub struct DominatorTree {
    positions: BTreeMap<BlockId, usize>,
    nodes: Vec<DominatorNode>,
    /// Reverse-postorder position to block identity.
    blocks: Vec<BlockId>,
    /// Each position's immediate dominator; the entry names itself.
    immediate: Vec<usize>,
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

    /// The blocks strictly dominating `block`, nearest first: its dominator
    /// tree ancestors. Unreachable or unknown blocks have none. Walking this
    /// chain costs its length, where testing every candidate definition
    /// block with [`Self::dominates`] costs the number of candidates.
    pub fn strict_dominators(&self, block: BlockId) -> impl Iterator<Item = BlockId> + '_ {
        let mut position = self.positions.get(&block).copied();
        std::iter::from_fn(move || {
            // Position zero is the entry, which has no strict dominator.
            let current = position.filter(|&current| current != 0)?;
            let parent = self.immediate[current];
            position = Some(parent);
            Some(self.blocks[parent])
        })
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
    DominatorTree {
        positions,
        nodes,
        blocks: ordered,
        immediate,
    }
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

/// Removing DFS ancestor edges makes the remaining graph acyclic. Their
/// targets must be treated as arbitrary iteration entries for proof facts;
/// deleting the edges does not prove a loop invariant or termination.
pub fn feedback_edges(machine: &TerminalMachine) -> BTreeMap<EdgeId, BlockId> {
    let successors = successors(machine);
    let mut entered = BTreeSet::from([machine.entry]);
    let mut finished = BTreeSet::new();
    let mut pending = vec![(machine.entry, 0usize)];
    let mut feedback = BTreeMap::new();
    while let Some((block, successor_position)) = pending.last_mut() {
        let Some((edge, target)) = successors[block].get(*successor_position).copied() else {
            finished.insert(*block);
            pending.pop();
            continue;
        };
        *successor_position += 1;
        if !entered.insert(target) {
            if !finished.contains(&target) {
                feedback.insert(edge, target);
            }
        } else {
            pending.push((target, 0));
        }
    }
    feedback
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

/// Topology-derived component identity for a cyclic component that carries no
/// producer ranking row — the name an absence-of-bound report uses when an
/// unranked component is the directed cause. Validation pins a producer's
/// component edges to ascending edge-id order covering exactly the members'
/// internal edges, so for any member set a producer later ranks this identity
/// equals `control_cycle_identity` on that row.
pub fn cyclic_component_identity(
    machine: &TerminalMachine,
    members: &[BlockId],
) -> CycleComponentId {
    let member_set: std::collections::BTreeSet<BlockId> = members.iter().copied().collect();
    let outgoing = successors(machine);
    let mut internal: Vec<(EdgeId, BlockId, BlockId)> = Vec::new();
    for member in members {
        for (edge, target) in outgoing.get(member).into_iter().flatten() {
            if member_set.contains(target) {
                internal.push((*edge, *member, *target));
            }
        }
    }
    internal.sort();
    topology_identity(machine, internal)
}

fn topology_identity(
    machine: &TerminalMachine,
    edges: impl IntoIterator<Item = (EdgeId, BlockId, BlockId)>,
) -> CycleComponentId {
    let mut digest = Sha256::new();
    digest.update(b"psi.control-cycle.topology.v1\0");
    digest.update(machine.id.get().to_le_bytes());
    for (edge, source, target) in edges {
        digest.update(edge.get().to_le_bytes());
        digest.update(source.get().to_le_bytes());
        digest.update(target.get().to_le_bytes());
    }
    semantic_id(&digest.finalize().into())
}

fn semantic_id<Identity: PsiSemanticId>(digest: &[u8; 32]) -> Identity {
    let raw = u64::from_le_bytes(digest[..8].try_into().expect("SHA-256 prefix")) | 1;
    Identity::new(raw).expect("the low bit establishes a nonzero semantic identity")
}
