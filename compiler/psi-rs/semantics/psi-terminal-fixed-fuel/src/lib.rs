#![forbid(unsafe_code)]

//! Recomputable restricted fixed-fuel certificates for terminal Psi.
//!
//! The current terminal verifier accepts only an acyclic straight-line path,
//! so the checker derives exact entry-to-return costs without precondition
//! assumptions and partitions that path at every explicit jump/return edge.
//! Branch-sensitive outcomes join this surface only when that semantic
//! vocabulary exists.

use std::collections::{BTreeMap, BTreeSet};

use psi_core::{BlockId, EdgeId, MachineId, Proposition};
use psi_terminal::{TerminalMachine, Terminator};
use psi_terminal_codec::{CodecError, TerminalPsiIdentity, terminal_psi_identity};
use psi_terminal_fuel::{FuelScheduleIdentity, TerminalFuelSchedule};
use psi_terminal_verifier::VerifiedTerminalModule;

/// Exact first-slice theorem: one machine entry reaches one return edge within
/// the published v1 logical-fuel ceiling.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixedEntryFuelCertificate {
    terminal_psi: TerminalPsiIdentity,
    schedule: FuelScheduleIdentity,
    entry: MachineId,
    return_edge: EdgeId,
    relevant_preconditions: Vec<Proposition>,
    ceiling_units: u64,
}

/// Exact current-vocabulary theorem for one selected machine-local path
/// segment. The segment begins before the first operation in `start_block` and
/// includes the charged `end_edge`; its endpoint may be either a jump or a
/// return. A later safe-point classifier can select eligible endpoints without
/// changing this recomputable accounting primitive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixedSegmentFuelCertificate {
    terminal_psi: TerminalPsiIdentity,
    schedule: FuelScheduleIdentity,
    machine: MachineId,
    start_block: BlockId,
    end_edge: EdgeId,
    relevant_preconditions: Vec<Proposition>,
    ceiling_units: u64,
}

impl FixedSegmentFuelCertificate {
    pub const fn terminal_psi(&self) -> TerminalPsiIdentity {
        self.terminal_psi
    }

    pub const fn schedule(&self) -> FuelScheduleIdentity {
        self.schedule
    }

    pub const fn machine(&self) -> MachineId {
        self.machine
    }

    pub const fn start_block(&self) -> BlockId {
        self.start_block
    }

    pub const fn end_edge(&self) -> EdgeId {
        self.end_edge
    }

    pub fn relevant_preconditions(&self) -> &[Proposition] {
        &self.relevant_preconditions
    }

    pub const fn ceiling_units(&self) -> u64 {
        self.ceiling_units
    }
}

impl FixedEntryFuelCertificate {
    pub const fn terminal_psi(&self) -> TerminalPsiIdentity {
        self.terminal_psi
    }

    pub const fn schedule(&self) -> FuelScheduleIdentity {
        self.schedule
    }

    pub const fn entry(&self) -> MachineId {
        self.entry
    }

    pub const fn return_edge(&self) -> EdgeId {
        self.return_edge
    }

    pub fn relevant_preconditions(&self) -> &[Proposition] {
        &self.relevant_preconditions
    }

    pub const fn ceiling_units(&self) -> u64 {
        self.ceiling_units
    }
}

/// Derive the exact current-slice bound from a verified canonical semantic
/// module. The checker performs no search and depends on no producing compiler
/// state or proof-bundle representation.
pub fn derive_fixed_entry_fuel(
    verified: &VerifiedTerminalModule<'_>,
    entry: MachineId,
) -> Result<FixedEntryFuelCertificate, FixedFuelError> {
    let module = verified.module();
    let terminal_psi = terminal_psi_identity(module).map_err(FixedFuelError::SemanticIdentity)?;
    let machine = module
        .machines
        .iter()
        .find(|machine| machine.id == entry)
        .ok_or(FixedFuelError::UnknownEntry(entry))?;
    let (return_edge, ceiling_units) = derive_straight_line_bound(machine)?;
    Ok(FixedEntryFuelCertificate {
        terminal_psi,
        schedule: TerminalFuelSchedule::CURRENT.identity(),
        entry,
        return_edge,
        // Current control and operation costs do not depend on values. The
        // theorem therefore holds for every invocation admitted by the
        // machine contract and needs no additional premise subset.
        relevant_preconditions: Vec::new(),
        ceiling_units,
    })
}

/// Recompute and compare every public certificate field. Consumers never need
/// to trust a producing compiler's claimed ceiling.
pub fn validate_fixed_entry_fuel(
    verified: &VerifiedTerminalModule<'_>,
    certificate: &FixedEntryFuelCertificate,
) -> Result<(), FixedFuelError> {
    let expected = derive_fixed_entry_fuel(verified, certificate.entry)?;
    if expected != *certificate {
        return Err(FixedFuelError::CertificateMismatch);
    }
    Ok(())
}

/// Derive an exact bound for one selected path segment in the current
/// unconditional, acyclic terminal vocabulary. The charged endpoint is part
/// of the segment so adjacent certificates neither omit nor double-charge an
/// edge.
pub fn derive_fixed_segment_fuel(
    verified: &VerifiedTerminalModule<'_>,
    machine: MachineId,
    start_block: BlockId,
    end_edge: EdgeId,
) -> Result<FixedSegmentFuelCertificate, FixedFuelError> {
    let module = verified.module();
    let terminal_psi = terminal_psi_identity(module).map_err(FixedFuelError::SemanticIdentity)?;
    let machine_semantics = module
        .machines
        .iter()
        .find(|candidate| candidate.id == machine)
        .ok_or(FixedFuelError::UnknownEntry(machine))?;
    let ceiling_units = derive_segment_bound(machine_semantics, start_block, end_edge)?;
    Ok(FixedSegmentFuelCertificate {
        terminal_psi,
        schedule: TerminalFuelSchedule::CURRENT.identity(),
        machine,
        start_block,
        end_edge,
        // Current operation and edge costs are value-independent.
        relevant_preconditions: Vec::new(),
        ceiling_units,
    })
}

/// Recompute every public segment field from independently verified terminal
/// semantics.
pub fn validate_fixed_segment_fuel(
    verified: &VerifiedTerminalModule<'_>,
    certificate: &FixedSegmentFuelCertificate,
) -> Result<(), FixedFuelError> {
    let expected = derive_fixed_segment_fuel(
        verified,
        certificate.machine,
        certificate.start_block,
        certificate.end_edge,
    )?;
    if expected != *certificate {
        return Err(FixedFuelError::CertificateMismatch);
    }
    Ok(())
}

/// Select the complete current-vocabulary safe-point partition for one
/// machine. Every explicit unconditional jump or return edge is a semantic
/// safe point in this slice: operations are total, no partial call/suspension
/// state can cross the edge, and the successor block begins the next segment.
/// The returned order is execution order from the machine entry.
/// Semantic-v13 conditional machines refuse until the restricted checker can
/// derive and validate a complete maximum-path branch certificate.
pub fn derive_fixed_safe_point_segments(
    verified: &VerifiedTerminalModule<'_>,
    machine: MachineId,
) -> Result<Vec<FixedSegmentFuelCertificate>, FixedFuelError> {
    let machine_semantics = verified
        .module()
        .machines
        .iter()
        .find(|candidate| candidate.id == machine)
        .ok_or(FixedFuelError::UnknownEntry(machine))?;
    let blocks = machine_semantics
        .blocks
        .iter()
        .map(|block| (block.id, block))
        .collect::<BTreeMap<_, _>>();
    let mut visited = BTreeSet::new();
    let mut current = machine_semantics.entry;
    let mut segments = Vec::new();

    loop {
        if !visited.insert(current) {
            return Err(FixedFuelError::ControlCycle(current));
        }
        let block = blocks
            .get(&current)
            .copied()
            .ok_or(FixedFuelError::UnknownBlock(current))?;
        if matches!(block.terminator, Terminator::Conditional { .. }) {
            return Err(FixedFuelError::BranchingNotYetSupported(current));
        }
        segments.push(derive_fixed_segment_fuel(
            verified,
            machine,
            current,
            block.terminator.edge(),
        )?);
        match block.terminator {
            Terminator::Jump { target, .. } => current = target,
            Terminator::Conditional { .. } => unreachable!("rejected above"),
            Terminator::Return { .. } => return Ok(segments),
        }
    }
}

/// Recompute the whole ordered safe-point partition. Validating certificates
/// one at a time is insufficient because a producer could omit a reachable
/// segment or present a different order.
pub fn validate_fixed_safe_point_segments(
    verified: &VerifiedTerminalModule<'_>,
    machine: MachineId,
    certificates: &[FixedSegmentFuelCertificate],
) -> Result<(), FixedFuelError> {
    let expected = derive_fixed_safe_point_segments(verified, machine)?;
    if expected != certificates {
        return Err(FixedFuelError::CertificateMismatch);
    }
    Ok(())
}

fn derive_straight_line_bound(machine: &TerminalMachine) -> Result<(EdgeId, u64), FixedFuelError> {
    let schedule = TerminalFuelSchedule::CURRENT;
    let blocks = machine
        .blocks
        .iter()
        .map(|block| (block.id, block))
        .collect::<BTreeMap<_, _>>();
    let mut visited = BTreeSet::new();
    let mut current = machine.entry;
    let mut units = 0_u64;

    loop {
        if !visited.insert(current) {
            return Err(FixedFuelError::ControlCycle(current));
        }
        let block = blocks
            .get(&current)
            .copied()
            .ok_or(FixedFuelError::UnknownBlock(current))?;
        for operation in &block.operations {
            units = units
                .checked_add(schedule.operation_units(&operation.kind))
                .ok_or(FixedFuelError::BoundOverflow)?;
        }
        units = units
            .checked_add(schedule.terminator_units(&block.terminator))
            .ok_or(FixedFuelError::BoundOverflow)?;
        match block.terminator {
            Terminator::Jump { target, .. } => current = target,
            Terminator::Conditional { .. } => {
                return Err(FixedFuelError::BranchingNotYetSupported(current));
            }
            Terminator::Return { edge, .. } => return Ok((edge, units)),
        }
    }
}

fn derive_segment_bound(
    machine: &TerminalMachine,
    start_block: BlockId,
    end_edge: EdgeId,
) -> Result<u64, FixedFuelError> {
    let schedule = TerminalFuelSchedule::CURRENT;
    let blocks = machine
        .blocks
        .iter()
        .map(|block| (block.id, block))
        .collect::<BTreeMap<_, _>>();
    if !blocks.contains_key(&start_block) {
        return Err(FixedFuelError::UnknownBlock(start_block));
    }
    let mut visited = BTreeSet::new();
    let mut current = start_block;
    let mut units = 0_u64;

    loop {
        if !visited.insert(current) {
            return Err(FixedFuelError::ControlCycle(current));
        }
        let block = blocks
            .get(&current)
            .copied()
            .ok_or(FixedFuelError::UnknownBlock(current))?;
        for operation in &block.operations {
            units = units
                .checked_add(schedule.operation_units(&operation.kind))
                .ok_or(FixedFuelError::BoundOverflow)?;
        }
        units = units
            .checked_add(schedule.terminator_units(&block.terminator))
            .ok_or(FixedFuelError::BoundOverflow)?;
        if block.terminator.edges().any(|edge| edge == end_edge) {
            if matches!(block.terminator, Terminator::Conditional { .. }) {
                return Err(FixedFuelError::BranchingNotYetSupported(current));
            }
            return Ok(units);
        }
        match block.terminator {
            Terminator::Jump { target, .. } => current = target,
            Terminator::Conditional { .. } => {
                return Err(FixedFuelError::BranchingNotYetSupported(current));
            }
            Terminator::Return { edge, .. } => {
                return Err(FixedFuelError::SegmentEndNotReached {
                    requested: end_edge,
                    reached_return: edge,
                });
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FixedFuelError {
    SemanticIdentity(CodecError),
    UnknownEntry(MachineId),
    UnknownBlock(BlockId),
    ControlCycle(BlockId),
    BranchingNotYetSupported(BlockId),
    SegmentEndNotReached {
        requested: EdgeId,
        reached_return: EdgeId,
    },
    BoundOverflow,
    CertificateMismatch,
}

impl std::fmt::Display for FixedFuelError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for FixedFuelError {}
