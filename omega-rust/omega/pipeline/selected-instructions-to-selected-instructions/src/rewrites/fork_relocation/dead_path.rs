//! The forward dead-on-path audit for fork relocation.
//!
//! Sinking the member into one arm removes its execution from every
//! traversal that leaves the branch through a different edge. The move is
//! sound only when nothing on those paths can observe the definitions the
//! member no longer publishes: every register the member writes and every
//! condition-state unit it defines or clobbers must be dead on every path
//! leaving the branch's non-landing edges — read by no body or terminator
//! position and no edge transport before a write retires the stale
//! definition.
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
//! Two positions are special. In the member's own block the vacated index
//! is absent from the transformed stream — a dead path that loops back
//! through it scans the remaining positions only. In the landing arm the
//! member executes at the landing index on every traversal reaching the
//! block — the arm's only predecessors are the branch's own edges — so
//! positions before it still see live locations stale, while the member's
//! own position republishes every location it writes and clears the set.
//! The member's own reads need no audit there: the window's position-level
//! hazard proof already refuses every write to a location the member reads
//! between its old and new positions, on every path — so the reaching
//! definition it observes at the landing index is the one it observed at
//! its original position.
use std::collections::{BTreeSet, VecDeque};

use register_model::RegisterUnitId;
use selected_instructions::{
    SelectedBlock, SelectedBlockId, SelectedCasePayloadTransport, SelectedFunction,
    SelectedInstruction, SelectedStructuralTransport, SelectedSuccessor, SelectedTerminator,
    SelectedValueTransport, VirtualRegisterId,
};

use crate::rewrites::window_hazards::{register_reads, register_writes};

/// The locations a moved member publishes that a path it no longer
/// executes on must never observe stale: every register it writes and
/// every condition-state unit it defines or clobbers.
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

/// Everything the member writes that a skipped execution would leave
/// stale: written registers plus defined and clobbered condition-state
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

/// Prove every location the member writes dead on every path leaving the
/// branch's non-landing edges: `seed_edges` are the branch terminator's
/// successor edges that do not reach the landing arm. `member_block` /
/// `member_index` name the member's vacated position — absent from the
/// transformed stream — and `arm_index` / `landing_index` name the
/// position it occupies instead, where any traversal reaching it
/// republishes every live location fresh.
///
/// Returns `false` the moment a still-live member location meets a reader:
/// a body or terminator position reading it, or a crossed edge whose
/// transports read it at the boundary.
pub(super) fn dead(
    function: &SelectedFunction,
    member_block: usize,
    member_index: usize,
    arm_index: usize,
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
            // The member is gone from its own block's stream and executes
            // at the landing index in the arm — publishing every location
            // it writes, so the live set ends there on any path through it.
            if current == member_block && position == member_index {
                continue;
            }
            if current == arm_index && position == landing_index {
                live = LiveLocations::default();
            }
            if reads_live(instruction, &live) {
                return false;
            }
            retire_writes(instruction, &mut live);
        }
        if current == arm_index && landing_index == block.instructions.len() {
            live = LiveLocations::default();
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
