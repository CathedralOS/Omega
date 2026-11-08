/*
Control-flow graph reachability, predecessor/successor indices, dominance, and edge-order queries
over `TerminalMachine` blocks.
*/

use std::collections::{BTreeMap, BTreeSet};

use semantic_vocabulary::MachineId;
use semantic_vocabulary::{BlockId, EdgeId, ValueId};
use semantic_vocabulary::{CycleComponentId, PsiSemanticId};
use terminal_psi::{Block, StructuralArgument, TerminalMachine, TerminalNaturalCycle, Terminator};

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

#[derive(Debug, Clone, Copy)]
pub struct ForwardedBinding<'block> {
    pub target: BlockId,
    pub arguments: Option<&'block [ValueId]>,
    pub structural_arguments: &'block [StructuralArgument],
}

pub fn forwarded_bindings(block: &Block) -> impl Iterator<Item = ForwardedBinding<'_>> {
    let (forwarded, cases) = match &block.terminator {
        Terminator::Jump {
            target,
            arguments,
            structural_arguments,
            ..
        } => (
            [
                Some(ForwardedBinding {
                    target: *target,
                    arguments: Some(arguments.as_slice()),
                    structural_arguments: structural_arguments.as_slice(),
                }),
                None,
            ],
            &[][..],
        ),
        Terminator::Conditional {
            when_true,
            when_false,
            ..
        } => (
            [when_true, when_false].map(|successor| {
                Some(ForwardedBinding {
                    target: successor.target,
                    arguments: Some(successor.arguments.as_slice()),
                    structural_arguments: successor.structural_arguments.as_slice(),
                })
            }),
            &[][..],
        ),
        Terminator::StructuralCase { cases, .. } => ([None, None], cases.as_slice()),
        _ => ([None, None], &[][..]),
    };
    forwarded
        .into_iter()
        .flatten()
        .chain(cases.iter().map(|successor| ForwardedBinding {
            target: successor.target,
            arguments: None,
            structural_arguments: &successor.structural_arguments,
        }))
}

pub fn reaches_by_case(block: &Block, target: BlockId) -> bool {
    matches!(&block.terminator, Terminator::StructuralCase { cases, .. }
        if cases.iter().any(|successor| successor.target == target))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionGraphError {
    UnknownMachine(MachineId),
    AmbiguousMachine(MachineId),
    InvalidControl {
        machine: MachineId,
        error: ControlGraphError,
    },
}

pub trait NormalCompletionGraph {
    type Error;
    fn entry(&mut self, machine: MachineId) -> Result<BlockId, Self::Error>;
    fn disposition(
        &self,
        machine: MachineId,
        block: BlockId,
    ) -> Result<NormalCompletionDisposition, Self::Error>;
}

pub enum NormalCompletionDisposition {
    Return,
    Stop,
    Invoke(MachineId),
    Successors(Vec<BlockId>),
}

pub fn normal_completion_possible_in<Graph: NormalCompletionGraph>(
    graph: &mut Graph,
    initial: MachineId,
) -> Result<bool, Graph::Error> {
    let entry = graph.entry(initial)?;
    let mut visited = BTreeSet::new();
    let mut pending = vec![(initial, entry)];
    while let Some((machine, block)) = pending.pop() {
        if !visited.insert((machine, block)) {
            continue;
        }
        match graph.disposition(machine, block)? {
            NormalCompletionDisposition::Return => return Ok(true),
            NormalCompletionDisposition::Stop => {}
            NormalCompletionDisposition::Invoke(callee) => {
                let entry = graph.entry(callee)?;
                pending.push((callee, entry));
            }
            NormalCompletionDisposition::Successors(targets) => {
                pending.extend(targets.into_iter().map(|target| (machine, target)))
            }
        }
    }
    Ok(false)
}

struct ModuleCompletionGraph<'a> {
    module: &'a terminal_psi::TerminalModule,
    validated: BTreeSet<MachineId>,
}
impl ModuleCompletionGraph<'_> {
    fn machine(&self, identity: MachineId) -> Result<&TerminalMachine, CompletionGraphError> {
        let mut candidates = self
            .module
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
    }
}
impl NormalCompletionGraph for ModuleCompletionGraph<'_> {
    type Error = CompletionGraphError;
    fn entry(&mut self, identity: MachineId) -> Result<BlockId, Self::Error> {
        if !self.validated.contains(&identity) {
            validate_control_graph(self.machine(identity)?).map_err(|error| {
                CompletionGraphError::InvalidControl {
                    machine: identity,
                    error,
                }
            })?;
            self.validated.insert(identity);
        }
        Ok(self.machine(identity)?.entry)
    }
    fn disposition(
        &self,
        machine: MachineId,
        block: BlockId,
    ) -> Result<NormalCompletionDisposition, Self::Error> {
        use terminal_psi::Terminator;
        let block =
            self.machine(machine)?
                .block(&block)
                .ok_or(CompletionGraphError::InvalidControl {
                    machine,
                    error: ControlGraphError::UnknownTarget(block),
                })?;
        Ok(match &block.terminator {
            Terminator::Return { .. }
            | Terminator::ReturnUnit { .. }
            | Terminator::ReturnStructural { .. } => NormalCompletionDisposition::Return,
            Terminator::InvokeNoNormal { callee, .. } => {
                NormalCompletionDisposition::Invoke(*callee)
            }
            Terminator::Crash { .. } | Terminator::ExternalCompletion { .. } => {
                NormalCompletionDisposition::Stop
            }
            Terminator::Jump { .. }
            | Terminator::Conditional { .. }
            | Terminator::StructuralCase { .. } => {
                NormalCompletionDisposition::Successors(block.terminator.successor_targets())
            }
        })
    }
}

pub fn normal_completion_possible(
    module: &terminal_psi::TerminalModule,
    initial: MachineId,
) -> Result<bool, CompletionGraphError> {
    normal_completion_possible_in(
        &mut ModuleCompletionGraph {
            module,
            validated: BTreeSet::new(),
        },
        initial,
    )
}

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

    pub fn depth(&self, block: BlockId) -> Option<usize> {
        self.positions
            .get(&block)
            .map(|&position| self.nodes[position].depth)
    }
}

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
    semantic_id(&digest.finish())
}

fn semantic_id<Identity: PsiSemanticId>(digest: &[u8; 32]) -> Identity {
    let raw = u64::from_le_bytes(digest[..8].try_into().expect("SHA-256 prefix")) | 1;
    Identity::new(raw).expect("the low bit establishes a nonzero semantic identity")
}
