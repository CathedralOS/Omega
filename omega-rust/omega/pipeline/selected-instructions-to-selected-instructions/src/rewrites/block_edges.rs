//! The control-flow surface of one selected block, shared by every rewrite
//! that walks a function: the successor edges a terminator names, the
//! instruction it carries, the whole-function edge roster, and the per-edge
//! checks and surfaces the relocation admissions apply to an edge a member
//! crosses. `acyclic_paths` and `crossed_window` additionally derive the
//! general relocation window — the positions and edges every acyclic path
//! between a member run and its destination crosses — so a family that
//! admits a run-and-destination pair applies its own traversal-parity gates
//! and hands the window to one derivation and one audit.
use std::collections::{BTreeMap, BTreeSet};

use selected_instructions::{
    SelectedBlock, SelectedBlockId, SelectedFunction, SelectedInstruction,
    SelectedMemoryAccessOrigin, SelectedStructuralTransport, SelectedSuccessor,
    SelectedSuccessorRole, SelectedTerminator, SelectedValueTransport,
};

use crate::rewrites::window_hazards::{has_memory_rows, register_reads, register_writes};

/// The instruction a terminator carries — a position every traversal of
/// its block observes, and the body-end landing position a destination can
/// name.
pub(super) fn terminator_instruction(terminator: &SelectedTerminator) -> &SelectedInstruction {
    match terminator {
        SelectedTerminator::Crash { instruction, .. }
        | SelectedTerminator::HostedExitProcess { instruction, .. }
        | SelectedTerminator::Jump { instruction, .. }
        | SelectedTerminator::ConditionalBranch { instruction, .. }
        | SelectedTerminator::ConditionalBranchU64LessThan { instruction, .. }
        | SelectedTerminator::ConditionalBranchI64LessThan { instruction, .. }
        | SelectedTerminator::Return { instruction, .. } => instruction,
    }
}

/// Every successor edge a terminator names, in its physical record order;
/// `HostedExitProcess` and `Return` name none.
pub(super) fn terminator_successors(terminator: &SelectedTerminator) -> Vec<&SelectedSuccessor> {
    match terminator {
        SelectedTerminator::Jump { successor, .. } => vec![successor],
        SelectedTerminator::ConditionalBranch {
            when_nonzero,
            when_zero,
            ..
        } => vec![when_nonzero, when_zero],
        SelectedTerminator::ConditionalBranchU64LessThan {
            when_less,
            when_not_less,
            ..
        }
        | SelectedTerminator::ConditionalBranchI64LessThan {
            when_less,
            when_not_less,
            ..
        } => vec![when_less, when_not_less],
        SelectedTerminator::Crash { .. }
        | SelectedTerminator::HostedExitProcess { .. }
        | SelectedTerminator::Return { .. } => Vec::new(),
    }
}

/// Every position of a block: its body instructions, then the instruction
/// its terminator carries.
pub(super) fn block_instructions(
    block: &SelectedBlock,
) -> impl Iterator<Item = &SelectedInstruction> {
    block
        .instructions
        .iter()
        .chain(std::iter::once(terminator_instruction(&block.terminator)))
}

/// Every successor edge in the function paired with the block it leaves,
/// for predecessor-count audits.
pub(super) fn all_edges(
    function: &SelectedFunction,
) -> impl Iterator<Item = (SelectedBlockId, &SelectedSuccessor)> {
    function.blocks.iter().flat_map(|block| {
        terminator_successors(&block.terminator)
            .into_iter()
            .map(move |edge| (block.id, edge))
    })
}

/// A plain semantic successor edge: case dispatch, continuation,
/// structural transfer, and per-edge fuel all carry boundary effects a
/// member would physically cross — a relocation does not cross them. This
/// applies to the edges the member lands through; edges it never traverses
/// join only the dead-path audit, which reads their complete register
/// surface directly.
pub(super) fn plain_edge(successor: &SelectedSuccessor) -> bool {
    successor.role == SelectedSuccessorRole::Semantic
        && successor.structural_case.is_none()
        && successor.fuel.is_empty()
        && successor
            .structural_bindings
            .iter()
            .all(|binding| binding.transport == SelectedStructuralTransport::Unused)
}

/// The member must not interfere with one crossed edge's register
/// transports: a member defining the transported argument would hand the
/// binding a stale value, a member defining the parameter would be
/// overwritten by it, and a member reading the parameter would observe the
/// transported value only after the move. Reading the argument is harmless
/// — the binding never writes it.
pub(super) fn transport_conflict(
    member: &SelectedInstruction,
    successor: &SelectedSuccessor,
) -> bool {
    successor.bindings.iter().any(|binding| {
        matches!(
            binding.transport,
            SelectedValueTransport::Registers { argument, parameter }
                if register_writes(member)
                    .any(|register| register == argument || register == parameter)
                    || register_reads(member).any(|register| register == parameter))
    })
}

/// The roster surface one crossed edge position carries: rows the
/// terminator instruction itself records plus any rows the roster logs
/// with the edge's own origin.
pub(super) fn edge_accounted(
    function: &SelectedFunction,
    terminator: &SelectedInstruction,
    successor: &SelectedSuccessor,
) -> bool {
    has_memory_rows(function, terminator.id)
        || function
            .memory_accesses
            .iter()
            .any(|access| access.origin == SelectedMemoryAccessOrigin::Edge(successor.psi_edge))
}

/// The register-plus-unit transport surface one edge carries — the
/// dead-path audit's per-edge scan cost.
pub(super) fn edge_surface(successor: &SelectedSuccessor) -> usize {
    successor.bindings.len()
        + successor.structural_bindings.len()
        + successor
            .structural_case
            .as_ref()
            .map_or(0, |case| case.payloads.len())
}

/// One edge traversal on a path between two blocks: the block the edge
/// leaves, the instruction its terminator carries — the edge's own
/// position — and the crossed successor row.
#[derive(Clone)]
pub(super) struct PathEdge<'function> {
    /// The block whose terminator carries the crossed edge.
    pub(super) block: SelectedBlockId,
    /// The instruction the crossed edge's terminator carries.
    pub(super) instruction: &'function SelectedInstruction,
    /// The crossed successor row.
    pub(super) successor: &'function SelectedSuccessor,
}

/// The acyclic-path walk bound the relocation admissions share: every
/// per-shape family bounded its window by shape; the shared derivation
/// bounds the walk itself. A function whose run-to-destination paths take
/// more edges than this abandons as over budget rather than reporting a
/// truncated window.
pub(super) const PATH_EDGE_LIMIT: usize = 64;

/// Every acyclic edge path from block `from` to block `to`, in walk order.
///
///
/// A relocation window crosses each edge at most once on any traversal, so
/// the walk never revisits a block: cyclic completions add no crossed
/// position and the enumeration stays finite. `edge_limit` bounds the
/// total edges the walk takes the way a per-shape family bounds its
/// window; reaching it returns `None` rather than reporting a truncated
/// path set as complete.
pub(super) fn acyclic_paths<'function>(
    function: &'function SelectedFunction,
    from: SelectedBlockId,
    to: SelectedBlockId,
    edge_limit: usize,
) -> Option<Vec<Vec<PathEdge<'function>>>> {
    fn walk<'function>(
        function: &'function SelectedFunction,
        at: SelectedBlockId,
        to: SelectedBlockId,
        visited: &mut BTreeSet<SelectedBlockId>,
        current: &mut Vec<PathEdge<'function>>,
        paths: &mut Vec<Vec<PathEdge<'function>>>,
        steps: &mut usize,
        edge_limit: usize,
    ) -> bool {
        if at == to {
            paths.push(current.clone());
            return true;
        }
        let Some(block) = function.blocks.iter().find(|block| block.id == at) else {
            return true;
        };
        let instruction = terminator_instruction(&block.terminator);
        for successor in terminator_successors(&block.terminator) {
            if !visited.insert(successor.block) {
                continue;
            }
            current.push(PathEdge {
                block: at,
                instruction,
                successor,
            });
            *steps += 1;
            let complete = *steps <= edge_limit
                && walk(
                    function,
                    successor.block,
                    to,
                    visited,
                    current,
                    paths,
                    steps,
                    edge_limit,
                );
            current.pop();
            visited.remove(&successor.block);
            if !complete {
                return false;
            }
        }
        true
    }
    let mut visited = BTreeSet::from([from]);
    let mut current = Vec::new();
    let mut paths = Vec::new();
    let mut steps = 0usize;
    walk(
        function,
        from,
        to,
        &mut visited,
        &mut current,
        &mut paths,
        &mut steps,
        edge_limit,
    )
    .then_some(paths)
}

/// The direction a relocation trades order across its window. The
/// cross-edge families move the run downstream — the paths join the run
/// block to the destination, and the run block's tail plus the
/// destination's head are crossed. The predecessor families move it
/// upstream — the paths join the destination to the run block, and the
/// destination's tail plus the run block's head are crossed.
#[derive(Clone, Copy)]
pub(super) enum CrossingDirection {
    /// Acyclic paths run from the run block to the destination block.
    Forward,
    /// Acyclic paths run from the destination block to the run block.
    Backward,
}

/// The window a relocation of a contiguous member run crosses between the
/// run's block and a destination position — the shared derivation the
/// per-shape families enumerated by hand before migrating here: the union
/// over every acyclic path of the positions and edges the move trades order
/// with, plus the span the run occupies and lands at. In the run block the
/// tail behind the run is crossed when moving downstream and the head
/// before it when moving upstream; in an intermediate block the whole body
/// is; in the destination block the head before the landing index is
/// crossed downstream and the tail at and after it upstream. Each
/// traversed edge contributes its successor row and its terminator-carried
/// instruction — the edge's own position.
pub(super) struct RelocationCrossing<'function> {
    /// Whether at least one acyclic path joins the run block to the
    /// destination block — a relocation onto an unreachable position
    /// executes on no traversal.
    pub(super) reachable: bool,
    /// Crossed body ordinals per index into `function.blocks`, sorted and
    /// deduplicated.
    pub(super) positions: Vec<(usize, Vec<usize>)>,
    /// Every successor edge any acyclic path crosses, deduplicated per
    /// (source block, edge identity) in first-encounter order.
    pub(super) edges: Vec<PathEdge<'function>>,
    /// Index into `function.blocks` of the block the run vacates.
    pub(super) run_block: usize,
    /// First body index the run occupies.
    pub(super) run_start: usize,
    /// Last body index the run occupies.
    pub(super) run_end: usize,
    /// Index into `function.blocks` of the block the run lands in.
    pub(super) destination_block: usize,
    /// Body index the run lands at; positions before it are crossed.
    pub(super) landing_index: usize,
}

/// Derives the window a relocation of the run at `run_start..=run_end` in
/// `function.blocks[run_block]` to `landing_index` in
/// `function.blocks[destination_block]` crosses, in the direction
/// `direction` names. When the blocks are the same the window is the span
/// between the run and the landing index, the landing position included —
/// the in-block family's case — and no edge is crossed either way. `None`
/// when the acyclic walk exceeds `edge_limit`.
pub(super) fn crossed_window<'function>(
    function: &'function SelectedFunction,
    run_block: usize,
    run_start: usize,
    run_end: usize,
    destination_block: usize,
    landing_index: usize,
    direction: CrossingDirection,
    edge_limit: usize,
) -> Option<RelocationCrossing<'function>> {
    let run = &function.blocks[run_block];
    let destination = &function.blocks[destination_block];
    let mut positions: BTreeMap<usize, BTreeSet<usize>> = BTreeMap::new();
    let mut edges: Vec<PathEdge<'function>> = Vec::new();
    if run.id == destination.id {
        let span = if landing_index <= run_start {
            landing_index..run_start
        } else {
            (run_end + 1)..(landing_index + 1)
        };
        positions.entry(run_block).or_default().extend(span);
    } else {
        let (from, to) = match direction {
            CrossingDirection::Forward => (run.id, destination.id),
            CrossingDirection::Backward => (destination.id, run.id),
        };
        let paths = acyclic_paths(function, from, to, edge_limit)?;
        let mut seen_edges = BTreeSet::new();
        for path in &paths {
            let run_side = match direction {
                CrossingDirection::Forward => (run_end + 1)..run.instructions.len(),
                CrossingDirection::Backward => 0..run_start,
            };
            positions.entry(run_block).or_default().extend(run_side);
            for (index, edge) in path.iter().enumerate() {
                if seen_edges.insert((edge.block, edge.successor.psi_edge)) {
                    edges.push(edge.clone());
                }
                if index + 1 < path.len()
                    && let Some(intermediate) = function
                        .blocks
                        .iter()
                        .position(|block| block.id == edge.successor.block)
                {
                    positions
                        .entry(intermediate)
                        .or_default()
                        .extend(0..function.blocks[intermediate].instructions.len());
                }
            }
            let destination_side = match direction {
                CrossingDirection::Forward => 0..landing_index,
                CrossingDirection::Backward => landing_index..destination.instructions.len(),
            };
            positions
                .entry(destination_block)
                .or_default()
                .extend(destination_side);
        }
        return Some(RelocationCrossing {
            reachable: !paths.is_empty(),
            positions: positions
                .into_iter()
                .map(|(block, ordinals)| (block, ordinals.into_iter().collect()))
                .collect(),
            edges,
            run_block,
            run_start,
            run_end,
            destination_block,
            landing_index,
        });
    }
    Some(RelocationCrossing {
        reachable: true,
        positions: positions
            .into_iter()
            .map(|(block, ordinals)| (block, ordinals.into_iter().collect()))
            .collect(),
        edges,
        run_block,
        run_start,
        run_end,
        destination_block,
        landing_index,
    })
}

#[cfg(test)]
mod tests {
    use register_model::{RegisterConstraintFamily, RegisterConstraintKey};
    use selected_instructions::{
        SelectedBlock, SelectedBlockId, SelectedBlockOrigin, SelectedFunction, SelectedInstruction,
        SelectedInstructionId, SelectedInstructionKind, SelectedSuccessor, SelectedSuccessorRole,
        SelectedTerminator,
    };
    use semantic_vocabulary::{BlockId, EdgeId, MachineId};

    use super::{CrossingDirection, RelocationCrossing, crossed_window};

    const BLOCK_A: SelectedBlockId = SelectedBlockId(0);
    const BLOCK_B: SelectedBlockId = SelectedBlockId(1);
    const BLOCK_C: SelectedBlockId = SelectedBlockId(2);
    const BLOCK_D: SelectedBlockId = SelectedBlockId(3);

    fn instruction(id: u64) -> SelectedInstruction {
        SelectedInstruction {
            id: SelectedInstructionId(id.try_into().unwrap()),
            kind: SelectedInstructionKind::MaterializeI64 {
                value: semantic_vocabulary::IntegerValue::Unsigned(0),
            },
            constraint: RegisterConstraintKey {
                family: RegisterConstraintFamily::Instruction,
                variant: 0,
            },
            operands: Vec::new(),
            implicit_uses: Vec::new(),
            implicit_defs: Vec::new(),
            clobbers: Vec::new(),
            provenance: Default::default(),
        }
    }

    fn successor(block: SelectedBlockId, edge: u64) -> SelectedSuccessor {
        SelectedSuccessor {
            role: SelectedSuccessorRole::Semantic,
            structural_case: None,
            structural_bindings: Vec::new(),
            psi_edge: EdgeId::new(edge).unwrap(),
            block,
            source_target: BlockId::new(1).unwrap(),
            bindings: Vec::new(),
            fuel: Vec::new(),
        }
    }

    fn jump(id: u64, block: SelectedBlockId, edge: u64) -> SelectedTerminator {
        SelectedTerminator::Jump {
            instruction: instruction(id),
            successor: successor(block, edge),
        }
    }

    fn ret(id: u64) -> SelectedTerminator {
        SelectedTerminator::Return {
            instruction: instruction(id),
            psi_return_edge: EdgeId::new(99).unwrap(),
        }
    }

    fn block(
        id: SelectedBlockId,
        instructions: Vec<u64>,
        terminator: SelectedTerminator,
    ) -> SelectedBlock {
        SelectedBlock {
            id,
            origin: SelectedBlockOrigin::Source(BlockId::new(1).unwrap()),
            instructions: instructions.into_iter().map(instruction).collect(),
            terminator,
        }
    }

    /// A four-block function: `A = [1;2;3] -> {B|C}`, `B = [4] -> D`,
    /// `C = [5] -> D`, `D = [6;7] -> return`. The relocation under test
    /// moves the run at A's tail to inside D.
    fn diamond() -> SelectedFunction {
        SelectedFunction {
            machine: MachineId::new(1).unwrap(),
            attachment: None,
            provenance: Default::default(),
            structural: None,
            local_storage_slots: Vec::new(),
            outgoing_arguments: Vec::new(),
            calls: Vec::new(),
            normalized_foreign_calls: Vec::new(),
            memory_accesses: Vec::new(),
            boundary_settlements: Vec::new(),
            entry_block: BLOCK_A,
            virtual_registers: Vec::new(),
            blocks: vec![
                block(
                    BLOCK_A,
                    vec![1, 2, 3],
                    SelectedTerminator::ConditionalBranch {
                        instruction: instruction(50),
                        when_nonzero: successor(BLOCK_B, 10),
                        when_zero: successor(BLOCK_C, 11),
                    },
                ),
                block(BLOCK_B, vec![4], jump(51, BLOCK_D, 12)),
                block(BLOCK_C, vec![5], jump(52, BLOCK_D, 13)),
                block(BLOCK_D, vec![6, 7], ret(53)),
            ],
        }
    }

    fn positions_of(crossing: &RelocationCrossing<'_>, block_index: usize) -> Vec<usize> {
        crossing
            .positions
            .iter()
            .find(|(index, _)| *index == block_index)
            .map(|(_, positions)| positions.clone())
            .unwrap_or_default()
    }

    fn edge_pairs(crossing: &RelocationCrossing<'_>) -> Vec<(SelectedBlockId, SelectedBlockId)> {
        let mut pairs: Vec<_> = crossing
            .edges
            .iter()
            .map(|edge| (edge.block, edge.successor.block))
            .collect();
        pairs.sort();
        pairs
    }

    #[test]
    fn crossing_a_diamond_covers_every_path() {
        let function = diamond();
        // Run is instruction 3 (index 2) in block A; lands at index 0 in D.
        let crossing =
            crossed_window(&function, 0, 2, 2, 3, 0, CrossingDirection::Forward, 64).unwrap();
        assert!(crossing.reachable);
        // A's tail is empty — the run is the last member — B and C are
        // fully crossed on their own paths, and nothing before index 0 in D.
        assert_eq!(positions_of(&crossing, 0), Vec::<usize>::new());
        assert_eq!(positions_of(&crossing, 1), vec![0]);
        assert_eq!(positions_of(&crossing, 2), vec![0]);
        assert_eq!(positions_of(&crossing, 3), Vec::<usize>::new());
        assert_eq!(
            edge_pairs(&crossing),
            vec![
                (BLOCK_A, BLOCK_B),
                (BLOCK_A, BLOCK_C),
                (BLOCK_B, BLOCK_D),
                (BLOCK_C, BLOCK_D),
            ]
        );
    }

    #[test]
    fn crossing_an_in_block_move_is_the_span_between() {
        let mut function = diamond();
        function.blocks[0].terminator = ret(54);
        // Run [1..=2] landing at index 0 crosses positions 0 and 1? No —
        // backward move: positions 0..1 (destination included, run start
        // excluded).
        let backward =
            crossed_window(&function, 0, 1, 2, 0, 0, CrossingDirection::Backward, 64).unwrap();
        assert_eq!(positions_of(&backward, 0), vec![0]);
        assert!(backward.edges.is_empty());
        // Forward move: run [0..=0] landing at index 2 crosses positions
        // 1..=2.
        let forward =
            crossed_window(&function, 0, 0, 0, 0, 2, CrossingDirection::Forward, 64).unwrap();
        assert_eq!(positions_of(&forward, 0), vec![1, 2]);
    }

    #[test]
    fn unreachable_destination_reports_no_paths() {
        let mut function = diamond();
        // D's terminator no longer returns; D jumps to a fresh dead end
        // that never rejoins — a fifth block E only reachable from D.
        function.blocks[3].terminator = jump(53, SelectedBlockId(4), 14);
        function
            .blocks
            .push(block(SelectedBlockId(4), vec![8], ret(55)));
        // Relocate A's tail run to *before* D on no path: use E as the
        // destination — unreachable from A? No, A→B→D→E is acyclic and
        // reaches E. Instead relocate to A itself already covered; target
        // a block not on any path: swap E for a detached block.
        function.blocks[4].id = SelectedBlockId(9);
        function.blocks[3].terminator = ret(53);
        let crossing =
            crossed_window(&function, 0, 2, 2, 4, 0, CrossingDirection::Forward, 64).unwrap();
        assert!(!crossing.reachable);
        assert!(crossing.edges.is_empty());
    }

    #[test]
    fn cyclic_back_edges_do_not_extend_the_window() {
        let mut function = diamond();
        // D loops back to A: the acyclic A→D paths are unchanged — the
        // back-edge can never appear on an acyclic A→D path.
        function.blocks[3].terminator = jump(53, BLOCK_A, 14);
        let crossing =
            crossed_window(&function, 0, 2, 2, 3, 0, CrossingDirection::Forward, 64).unwrap();
        assert!(crossing.reachable);
        assert!(!edge_pairs(&crossing).contains(&(BLOCK_D, BLOCK_A)));
    }

    #[test]
    fn exhausted_edge_limit_abandons_the_walk() {
        let function = diamond();
        // The diamond needs at least two edges per path; a limit of one
        // exhausts mid-walk and must not report a partial path set.
        assert!(crossed_window(&function, 0, 2, 2, 3, 0, CrossingDirection::Forward, 1).is_none());
    }
}
