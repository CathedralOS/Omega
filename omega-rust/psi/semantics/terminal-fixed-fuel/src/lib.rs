#![forbid(unsafe_code)]

//! Recomputable restricted fixed-fuel certificates for terminal Psi.
//!
//! Ordinary terminal verification accepts acyclic control flow, so the checker
//! derives an exact maximum entry-to-terminal-exit cost without precondition
//! assumptions and partitions the complete reachable graph at every reachable
//! explicit edge. A verified `Natural`-ranked machine also admits a whole-entry
//! ceiling: the components partition the cyclic topology, every cycle crosses a
//! strict rank descent, and the condensed graph is acyclic.

use semantic_vocabulary::{BlockId, EdgeId, MachineId, OperationId, Proposition};
use terminal_codec::{CodecError, TerminalPsiIdentity};
use terminal_fuel::FuelScheduleIdentity;

mod fuel_certification;
pub use fuel_certification::{
    derive_fixed_entry_fuel, derive_fixed_safe_point_segments, derive_fixed_segment_fuel,
    derive_validated_fixed_safe_point_segments, retain_validated_fixed_safe_point_segments,
    validate_fixed_entry_fuel, validate_fixed_safe_point_segments, validate_fixed_segment_fuel,
    validate_retained_fixed_safe_point_segments,
};

/// Exact restricted theorem: every path from one machine entry reaches a
/// return or crash within the published current logical-fuel ceiling.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixedEntryFuelCertificate {
    terminal_psi: TerminalPsiIdentity,
    schedule: FuelScheduleIdentity,
    entry: MachineId,
    relevant_preconditions: Vec<Proposition>,
    ceiling_units: u64,
}

/// Exact current-vocabulary theorem for one selected machine-local path
/// segment. The segment begins before the first operation in `start_block` and
/// includes the charged `end_edge`; its endpoint may be either a jump or a
/// return. A return endpoint's ordered nominal cleanup machines are part of
/// the segment's charged work: crossing the edge suspends into each in turn
/// before control leaves the machine. A later safe-point classifier can select
/// eligible endpoints without changing this recomputable accounting primitive.
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

/// Complete canonical safe-point partition for one verified terminal machine.
///
/// This carrier is deliberately non-clonable and has no public-field
/// constructor. It preserves the ordered segment certificates as semantic
/// evidence only: neither the catalog nor any individual row is a whole-entry
/// fixed-fuel theorem or authority to replace exact per-site charging.
#[derive(Debug, PartialEq, Eq)]
pub struct ValidatedFixedSafePointFuelSegments {
    terminal_psi: TerminalPsiIdentity,
    schedule: FuelScheduleIdentity,
    machine: MachineId,
    certificates: Vec<FixedSegmentFuelCertificate>,
}

impl ValidatedFixedSafePointFuelSegments {
    pub const fn terminal_psi(&self) -> TerminalPsiIdentity {
        self.terminal_psi
    }

    pub const fn schedule(&self) -> FuelScheduleIdentity {
        self.schedule
    }

    pub const fn machine(&self) -> MachineId {
        self.machine
    }

    pub fn certificates(&self) -> &[FixedSegmentFuelCertificate] {
        &self.certificates
    }
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

    pub fn relevant_preconditions(&self) -> &[Proposition] {
        &self.relevant_preconditions
    }

    pub const fn ceiling_units(&self) -> u64 {
        self.ceiling_units
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FixedFuelError {
    SemanticIdentity(CodecError),
    UnknownEntry(MachineId),
    UnknownBlock(BlockId),
    ControlCycle(BlockId),
    CallCycle(MachineId),
    BranchingNotYetSupported(BlockId),
    SegmentEndNotReached {
        requested: EdgeId,
        reached_terminal: EdgeId,
    },
    SegmentEndUnreachableAfterCall {
        block: BlockId,
        callee: MachineId,
    },
    NoTerminalPath(MachineId),
    /// The retained `Natural` component is malformed — a hard failure like any
    /// other broken semantic invariant, not an analysis limitation.
    InvalidRankedScc(MachineId),
    /// A descriptor-dispatched dynamic call has no indirect or stored
    /// dispatch row naming its realization — a broken semantic invariant,
    /// not an analysis limitation.
    MissingDynamicDispatch {
        owner: MachineId,
        operation: OperationId,
    },
    /// A dynamic-parameter call receives its callee from the invocation's
    /// descriptor table; the module deliberately retains no realization
    /// identity, so no fixed ceiling covers that open callee set.
    InvocationBoundCallee {
        owner: MachineId,
        operation: OperationId,
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
