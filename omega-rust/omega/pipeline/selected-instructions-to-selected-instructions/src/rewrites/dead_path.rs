//! The forward dead-on-path audit shared by the relocation rewrites that
//! move one member across a branch or a join.
//!
//! Moving the member changes which traversals execute it. A sink into one
//! arm removes its execution from every traversal that leaves the branch
//! through a different edge; a hoist into a fork head or a sink into a join
//! adds its execution to traversals that never ran it. Either move is sound
//! only when nothing on the affected paths can observe the definitions the
//! member no longer publishes, or newly publishes: every register the
//! member writes and every condition-state unit it defines or clobbers must
//! be dead on those paths — read by no body or terminator position and no
//! edge transport before a write retires the stale or foreign definition.
//!
//! The audit is the forward mirror of `rewrites/condition_state`'s
//! entry-event walk. Per-block entry sets of still-live member locations
//! propagate forward through each successor edge's register surface — a
//! binding or case-payload argument or a structural transport's argument
//! reading a live location refuses, while a parameter or payload writing
//! one retires it — and scan each reached block's body and terminator
//! positions the same way: a reader of a live location refuses, a writer
//! of one kills it. Entry sets only ever hold member locations and grow by
//! union at joins, so the walk is a monotone fixpoint over a finite subset
//! lattice and terminates.
//!
//! Two positions are special. At the landing position the member's
//! occupancy on the walked paths decides what the walk sees
//! ([`Landing`]). Where every traversal reaching the block runs the member
//! there, its position republishes every location it writes and clears the
//! live set. Where its execution is speculative on the walked paths, its
//! reads belong to the added execution and go unaudited, while its writes
//! republish member locations that are foreign to the path: the position
//! re-seeds the live set rather than clearing it, so a reader after it
//! still refuses and only an ordinary writer retires them. The destination
//! instruction at the landing index sits after the member in the
//! transformed stream and is scanned against the published set.
//!
//! In the member's own block the vacated index is absent from the
//! transformed stream — a dead path that loops back through it scans the
//! remaining positions only ([`Vacated`]). Whether the member's missing
//! write diverges there depends on the move: when every walked path
//! reaching the vacated block crossed the member's new position first, or
//! when every position behind the vacated index that could observe the
//! missing write sits inside the crossed window the hazard audit owns,
//! the index stays silent. When a walked path can reach the vacated block
//! without the member running — the confluence-hoist family, whose other
//! inflow edges arrive beside the member's new position rather than
//! through it — the vacated index is where the missing write leaves every
//! member location divergent, so it publishes them into the live set and
//! a later reader still refuses.
//!
//! The member's own reads need no audit on the source side either: the
//! window's position-level hazard proof already refuses every write to a
//! location the member reads between its old and new positions, on every
//! path — so the reaching definition it observes at the landing index is
//! the one it observed at its original position.
use std::collections::{BTreeSet, VecDeque};

use register_model::RegisterUnitId;
use selected_instructions::{
    SelectedBlockId, SelectedCasePayloadTransport, SelectedFunction, SelectedInstruction,
    SelectedStructuralTransport, SelectedSuccessor, SelectedValueTransport, VirtualRegisterId,
};

use crate::rewrites::block_edges::{terminator_instruction, terminator_successors};
use crate::rewrites::window_hazards::{register_reads, register_writes};

/// How the moved member occupies its landing position on the paths the
/// audit walks.
pub(super) enum Landing {
    /// Every traversal reaching the landing block runs the member there — a
    /// sink into one arm, whose only predecessors are the branch's own
    /// edges — so its position publishes every location it writes afresh.
    Executed,
    /// The walked traversals never ran the member before — a hoist into a
    /// fork head, a sink into a join — so its writes at the landing
    /// position are foreign to the path and stay live until an ordinary
    /// writer retires them.
    Speculated,
}

/// What the member's absence at its old index means on the walked paths.
pub(super) enum Vacated {
    /// The vacated index diverges nothing the walk must see: every walked
    /// path reaching the vacated block crossed the member's new position
    /// first — the hoist into a fork head, where reaching the arm means
    /// the member already ran — or every position behind it that could
    /// observe the missing write is a crossed window position the hazard
    /// audit already refuses, as the sink families' own-block tails are.
    /// The index is simply absent from the scanned stream.
    Silent,
    /// The member's write is missing where the source still ran it: a
    /// walked path reaches the vacated block beside the member's new
    /// position rather than through it, so every location the member
    /// writes goes live at the vacated index — a later reader meets the
    /// stale value where the source met the member's own.
    Removed,
}

/// The member's old and new positions in the transformed stream.
pub(super) struct Relocation<'a> {
    pub(super) member: &'a SelectedInstruction,
    /// The block the member leaves; `vacated_index` is absent from its
    /// transformed stream.
    pub(super) vacated_block: usize,
    pub(super) vacated_index: usize,
    /// The block the member enters and the index it occupies there.
    pub(super) landing_block: usize,
    pub(super) landing_index: usize,
    pub(super) landing: Landing,
    pub(super) vacated: Vacated,
}

/// Where the walk begins.
pub(super) enum Start<'a> {
    /// The member's locations are live on each of these edges: the branch
    /// terminator's successor edges that do not reach the landing block.
    Edges(&'a [&'a SelectedSuccessor]),
    /// The walk enters this block with nothing live yet; its landing
    /// position publishes the member's locations on every arrival, the
    /// member's own inflow included, since the region behind and ahead of
    /// that position cannot tell which inflow a traversal took.
    Block(usize),
}

/// The locations a moved member publishes that a path it no longer
/// executes on, or newly executes on, must never observe: every register
/// it writes and every condition-state unit it defines or clobbers.
#[derive(Clone, Default)]
struct LiveLocations {
    registers: BTreeSet<VirtualRegisterId>,
    units: BTreeSet<RegisterUnitId>,
}

impl LiveLocations {
    fn is_empty(&self) -> bool {
        self.registers.is_empty() && self.units.is_empty()
    }

    /// Union `other` into this entry set; whether the set grew decides
    /// whether the block must be rescanned.
    fn union_with(&mut self, other: &LiveLocations) -> bool {
        let before = self.registers.len() + self.units.len();
        self.registers.extend(other.registers.iter().copied());
        self.units.extend(other.units.iter().copied());
        self.registers.len() + self.units.len() != before
    }
}

/// Everything the member writes: written registers plus defined and
/// clobbered condition-state units.
fn member_locations(member: &SelectedInstruction) -> LiveLocations {
    LiveLocations {
        registers: register_writes(member).collect(),
        units: member
            .implicit_defs
            .iter()
            .chain(member.clobbers.iter())
            .copied()
            .collect(),
    }
}

/// The register surface one crossed edge carries at the boundary. Value
/// bindings and case payloads move register arguments into parameters;
/// structural bindings read an argument register into storage;
/// unmaterialized payloads write their parameter without reading. Reads
/// observe the pre-edge values — so a live location among them is a stale
/// observation — while writes retire the locations they name.
fn edge_registers(
    successor: &SelectedSuccessor,
) -> (BTreeSet<VirtualRegisterId>, BTreeSet<VirtualRegisterId>) {
    let mut reads = BTreeSet::new();
    let mut writes = BTreeSet::new();
    for binding in &successor.bindings {
        if let SelectedValueTransport::Registers {
            argument,
            parameter,
        } = binding.transport
        {
            reads.insert(argument);
            writes.insert(parameter);
        }
    }
    for binding in &successor.structural_bindings {
        match binding.transport {
            SelectedStructuralTransport::WholeValue { argument, .. }
            | SelectedStructuralTransport::Descriptor { argument, .. } => {
                reads.insert(argument);
            }
            SelectedStructuralTransport::Unused => {}
        }
    }
    if let Some(case) = &successor.structural_case {
        for payload in &case.payloads {
            match payload.transport {
                SelectedCasePayloadTransport::Registers {
                    argument,
                    parameter,
                } => {
                    reads.insert(argument);
                    writes.insert(parameter);
                }
                SelectedCasePayloadTransport::Unmaterialized { parameter } => {
                    writes.insert(parameter);
                }
                SelectedCasePayloadTransport::Unused => {}
            }
        }
    }
    (reads, writes)
}

/// Whether the position reads a still-live member location: an explicit
/// operand read of a live register, or an implicit use of a live
/// condition-state unit.
fn reads_live(instruction: &SelectedInstruction, live: &LiveLocations) -> bool {
    register_reads(instruction).any(|register| live.registers.contains(&register))
        || instruction
            .implicit_uses
            .iter()
            .any(|unit| live.units.contains(unit))
}

/// Retire the locations the position writes: a rewritten location is dead
/// to the member's stale definition from this position onward.
fn retire_writes(instruction: &SelectedInstruction, live: &mut LiveLocations) {
    for register in register_writes(instruction) {
        live.registers.remove(&register);
    }
    for unit in instruction
        .implicit_defs
        .iter()
        .chain(instruction.clobbers.iter())
    {
        live.units.remove(unit);
    }
}

/// Audit the live set against one crossed edge: an argument read of a
/// live location refuses; otherwise the edge's writes retire their
/// locations and the remainder enters the target block.
fn cross_edge(live: &LiveLocations, successor: &SelectedSuccessor) -> Option<LiveLocations> {
    let (reads, writes) = edge_registers(successor);
    if reads
        .iter()
        .any(|register| live.registers.contains(register))
    {
        return None;
    }
    let mut out = live.clone();
    for register in writes {
        out.registers.remove(&register);
    }
    Some(out)
}

/// The member's landing position as a walked path sees it.
fn land(live: &mut LiveLocations, landing: &Landing, locations: &LiveLocations) {
    match landing {
        Landing::Executed => *live = LiveLocations::default(),
        Landing::Speculated => {
            live.union_with(locations);
        }
    }
}

/// Prove every location the member writes dead on every path the walk
/// reaches from `start`, with the member absent from its vacated position
/// and present at its landing position as `relocation.landing` says.
///
/// Returns `false` the moment a still-live member location meets a reader:
/// a body or terminator position reading it, or a crossed edge whose
/// transports read it at the boundary.
pub(super) fn dead(
    function: &SelectedFunction,
    relocation: Relocation<'_>,
    start: Start<'_>,
) -> bool {
    let locations = member_locations(relocation.member);
    if locations.is_empty() {
        return true;
    }
    let index_of = |id: SelectedBlockId| function.blocks.iter().position(|block| block.id == id);
    let mut entry: Vec<LiveLocations> = (0..function.blocks.len())
        .map(|_| LiveLocations::default())
        .collect();
    let mut pending: VecDeque<usize> = VecDeque::new();
    match start {
        Start::Edges(edges) => {
            for edge in edges {
                let Some(out) = cross_edge(&locations, edge) else {
                    return false;
                };
                if let Some(target) = index_of(edge.block)
                    && entry[target].union_with(&out)
                    && !pending.contains(&target)
                {
                    pending.push_back(target);
                }
            }
        }
        Start::Block(block) => pending.push_back(block),
    }
    while let Some(current) = pending.pop_front() {
        let block = &function.blocks[current];
        let mut live = entry[current].clone();
        for (position, instruction) in block.instructions.iter().enumerate() {
            if current == relocation.vacated_block && position == relocation.vacated_index {
                if matches!(relocation.vacated, Vacated::Removed) {
                    live.union_with(&locations);
                }
                continue;
            }
            if current == relocation.landing_block && position == relocation.landing_index {
                land(&mut live, &relocation.landing, &locations);
            }
            if reads_live(instruction, &live) {
                return false;
            }
            retire_writes(instruction, &mut live);
        }
        if current == relocation.landing_block
            && relocation.landing_index == block.instructions.len()
        {
            land(&mut live, &relocation.landing, &locations);
        }
        let terminator = terminator_instruction(&block.terminator);
        if reads_live(terminator, &live) {
            return false;
        }
        retire_writes(terminator, &mut live);
        if live.is_empty() {
            continue;
        }
        for edge in terminator_successors(&block.terminator) {
            let Some(out) = cross_edge(&live, edge) else {
                return false;
            };
            if let Some(target) = index_of(edge.block)
                && !out.is_empty()
                && entry[target].union_with(&out)
                && !pending.contains(&target)
            {
                pending.push_back(target);
            }
        }
    }
    true
}
