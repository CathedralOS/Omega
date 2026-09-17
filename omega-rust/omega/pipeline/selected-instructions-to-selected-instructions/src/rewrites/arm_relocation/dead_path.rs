//! The forward dead-on-path audit for arm relocation.
//!
//! Hoisting the member into the fork head adds its execution to every
//! traversal that leaves the head through an edge that does not reach the
//! member's arm. The move is sound only when nothing on those paths can
//! observe the definitions the member newly publishes: every register the
//! member writes and every condition-state unit it defines or clobbers
//! must be dead on every path leaving the head's non-source edges — read
//! by no body or terminator position and no edge transport before a write
//! retires the foreign definition.
//!
//! The audit is the forward mirror of `rewrites/condition_state`'s
//! entry-event walk, shared in shape with `rewrites/fork_relocation`'s
//! dead-path audit but with the occupancy rule reversed. Per-block entry
//! sets of still-live member locations propagate forward through each
//! successor edge's register surface — a binding or case-payload argument
//! or a structural transport's argument reading a live location refuses,
//! while a parameter or payload writing one retires it — and scan each
//! reached block's body and terminator positions the same way: a reader
//! of a live location refuses, a writer of one kills it. Entry sets only
//! ever hold member locations and grow by union at joins, so the walk is
//! a monotone fixpoint over a finite subset lattice and terminates.
//!
//! Two positions are special. In the member's own block the vacated index
//! is absent from the transformed stream — a dead path that loops back
//! through it scans the remaining positions only. In the head the member
//! occupies the landing index on every traversal reaching the block —
//! including the speculative traversals this audit walks — so its reads
//! belong to the added execution and go unaudited, while its writes
//! republish member locations that remain foreign to the path: the
//! position re-seeds the live set rather than clearing it. Where the
//! sinking family's member re-publishes the values it always produced —
//! every path into its arm crosses the window the hazard audit pins —
//! this family's member on a speculative path reads and writes state the
//! path never carried, so a reader after its position still observes
//! foreign values and must refuse.
//!
//! The member's own reads need no audit on the source side either: the
//! window's position-level hazard proof already refuses every write to a
//! location the member reads between its old and new positions, on every
//! path through the head's edge into the arm — so the reaching definition
//! it observes at the landing index is the one it observed at its
//! original position.
use std::collections::{BTreeSet, VecDeque};

use register_model::RegisterUnitId;
use selected_instructions::{
    SelectedBlock, SelectedBlockId, SelectedCasePayloadTransport, SelectedFunction,
    SelectedInstruction, SelectedStructuralTransport, SelectedSuccessor, SelectedTerminator,
    SelectedValueTransport, VirtualRegisterId,
};

use crate::rewrites::window_hazards::{register_reads, register_writes};

/// The locations a speculated member publishes that a path it never ran
/// on must never observe: every register it writes and every
/// condition-state unit it defines or clobbers.
#[derive(Clone, Default)]
pub(super) struct LiveLocations {
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

/// Everything the member writes that a speculative execution would leave
/// foreign: written registers plus defined and clobbered condition-state
/// units.
pub(super) fn member_locations(member: &SelectedInstruction) -> LiveLocations {
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

/// Every successor edge of `terminator`, whatever its role: the dead-path
/// audit reads the physical successor record a traversal would take.
fn successor_edges(terminator: &SelectedTerminator) -> Vec<&SelectedSuccessor> {
    match terminator {
        SelectedTerminator::Jump { successor, .. } => vec![successor],
        SelectedTerminator::ConditionalBranch {
            when_nonzero,
            when_zero,
            ..
        }
        | SelectedTerminator::ConditionalBranchU64LessThan {
            when_less: when_nonzero,
            when_not_less: when_zero,
            ..
        }
        | SelectedTerminator::ConditionalBranchI64LessThan {
            when_less: when_nonzero,
            when_not_less: when_zero,
            ..
        } => vec![when_nonzero, when_zero],
        SelectedTerminator::HostedExitProcess { .. } | SelectedTerminator::Return { .. } => {
            Vec::new()
        }
    }
}

/// The instruction a terminator carries — a position every traversal of
/// its block observes.
fn terminator_instruction(block: &SelectedBlock) -> &SelectedInstruction {
    match &block.terminator {
        SelectedTerminator::HostedExitProcess { instruction, .. }
        | SelectedTerminator::Jump { instruction, .. }
        | SelectedTerminator::ConditionalBranch { instruction, .. }
        | SelectedTerminator::ConditionalBranchU64LessThan { instruction, .. }
        | SelectedTerminator::ConditionalBranchI64LessThan { instruction, .. }
        | SelectedTerminator::Return { instruction, .. } => instruction,
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
/// to the member's foreign definition from this position onward.
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

/// Prove every location the member writes dead on every path leaving the
/// head's non-source edges: `seed_edges` are the head terminator's
/// successor edges that do not reach the member's arm. `arm_block` /
/// `member_index` name the member's vacated position — absent from the
/// transformed stream — and `head_index` / `landing_index` name the
/// position it occupies instead, which the walk treats as a foreign
/// republisher: on a speculative traversal the member's re-execution
/// reads without observing and writes member locations back into the
/// live set, so a reader after its position still refuses and only an
/// ordinary writer retires them.
///
/// Returns `false` the moment a still-live member location meets a
/// reader: a body or terminator position reading it, or a crossed edge
/// whose transports read it at the boundary.
pub(super) fn dead(
    function: &SelectedFunction,
    arm_block: usize,
    member_index: usize,
    head_index: usize,
    landing_index: usize,
    member: &SelectedInstruction,
    seed_edges: &[&SelectedSuccessor],
) -> bool {
    let locations = member_locations(member);
    if locations.is_empty() {
        return true;
    }
    let index_of = |id: SelectedBlockId| function.blocks.iter().position(|block| block.id == id);
    let mut entry: Vec<LiveLocations> = (0..function.blocks.len())
        .map(|_| LiveLocations::default())
        .collect();
    let mut pending: VecDeque<usize> = VecDeque::new();
    for edge in seed_edges {
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
    while let Some(current) = pending.pop_front() {
        let block = &function.blocks[current];
        let mut live = entry[current].clone();
        for (position, instruction) in block.instructions.iter().enumerate() {
            // The member is gone from its own block's stream, and at its
            // new position in the head its speculative execution reads
            // nothing the audit tracks and republishes every location it
            // writes: those values are foreign to the path, so the live
            // set gains them back rather than clearing.
            if current == arm_block && position == member_index {
                continue;
            }
            if current == head_index && position == landing_index {
                live.union_with(&locations);
                continue;
            }
            if reads_live(instruction, &live) {
                return false;
            }
            retire_writes(instruction, &mut live);
        }
        if current == head_index && landing_index == block.instructions.len() {
            live.union_with(&locations);
        }
        let terminator = terminator_instruction(block);
        if reads_live(terminator, &live) {
            return false;
        }
        retire_writes(terminator, &mut live);
        if live.is_empty() {
            continue;
        }
        for edge in successor_edges(&block.terminator) {
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
