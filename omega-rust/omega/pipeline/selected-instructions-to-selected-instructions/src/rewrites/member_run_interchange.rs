//! Local scheduling of a member against a run on the selected CFG.
//!
//! Scheduling here means changing the order instructions execute inside one
//! block without changing what any of them observes. The primitive this
//! family proves is the bounded member-against-run interchange: one named
//! body instruction — the `member` — and the contiguous run the named
//! `run_first` and `run_last` members bound, of at least two members,
//! exchange places while every instruction between them keeps its relative
//! order, shifting by one less than the run's length. The member sits
//! strictly on one side of the run's span, before `run_first` or after
//! `run_last`; admission discovers which. The interchange completes the
//! scheduling family's granularity matrix: `rewrites/local_schedule`
//! interchanges two members, `rewrites/run_interchange` trades two runs,
//! and this family trades a member against a run. The degenerate shapes
//! stay with their own families — a one-member run is the pair
//! interchange's granularity and two runs are `run_interchange`'s shape,
//! and both refuse here. The move is also not a relocation: a relocation
//! rotates the crossed window toward the vacated index so the interior
//! lands on the far side of the moved body, while the interchange keeps
//! the interior between the two traded sides. Every other instruction,
//! register, roster row, call, settlement, and edge is retained
//! bit-identical, and replayed restore-by-content validation confirms the
//! proposal is exactly that interchange and nothing else.
//!
//! An interchange is admitted only when the window the two sides bound is
//! independent. The run's members keep their relative order — the run
//! moves as one body — so a data hazard is any register or
//! condition-state unit a member of one side writes and an instruction on
//! the other side or in the interior reads or also writes, or the side
//! member reads and a crossed position writes: the same access-mark plus
//! implicit-use, implicit-def, and clobber accounting the run interchange
//! proves, applied between the member and every run member and interior
//! position, and between every run member and the member and every
//! interior position. Interior positions keep their relative order with
//! each other, run members keep theirs inside the run, positions outside
//! the window keep theirs with every member, and since no location is
//! written by both a member and a crossed position the last writer before
//! every later observer — including the block's terminator and edge
//! transports — is the same instruction it was before the interchange.
//! Positions before the window are untouched.
//!
//! Positional observers bound the window the same way the run
//! interchange's does. A boundary settlement at any position after the
//! window's first index through its last observes a different executed
//! prefix once the member and the run trade places, so it refuses;
//! settlements at or outside the span observe the same executed set either
//! way. Calls, hosted effects, and terminator kinds are barriers: they
//! never interchange and never sit inside a crossed window, and an
//! instruction named by the call roster refuses for the same reason.
//!
//! Memory ordering follows the validated `memory_accesses` roster with the
//! same completeness discipline as the run interchange's: a kind that can
//! reach semantic or place-backed storage must carry the rows accounting
//! for that reach. A side's accounting is the union of its members' rows —
//! roster-carrying members of the run keep their relative order, so the
//! run may carry several rows itself — but every position a row-carrying
//! side trades order with must be row-less: a second accounted actor on
//! the other side or in the interior would reorder recorded accesses.
//! When neither side carries rows the interior's own accounted positions
//! keep their relative order and admit. A memory-capable kind without
//! rows is an unaccounted access and refuses.
//!
//! No register roster row, constraint key, or instruction identity
//! changes: each instruction keeps its own id, kind, operands, provenance,
//! and implicit surface while only its position in the block vector moves.
//!
//! Proposal and independent replay share only the admission predicates.
//! Validation consumes the proposed program, requires the touched block's
//! window to equal the independently computed interchange, and restores
//! the complete source by content.

mod admission;
mod rewrite;
mod validation;

use std::sync::Arc;

use optimization_core::OptimizationUnitIdentity;
use selected_instructions::{SelectedInstructionPlan, SelectedInstructionPlanIdentity};
use semantic_vocabulary::FuelScheduleIdentity;

pub use rewrite::interchange_selected_member_and_run;
pub use validation::validate_member_run_interchange;

#[cfg(test)]
mod tests;

/// An accepted member-against-run interchange with its replay receipt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedMemberRunInterchange {
    transformed: Arc<SelectedInstructionPlan>,
    receipt: MemberRunInterchangeReceipt,
}

impl ValidatedMemberRunInterchange {
    pub fn transformed(&self) -> &SelectedInstructionPlan {
        &self.transformed
    }

    pub fn shared_transformed(&self) -> Arc<SelectedInstructionPlan> {
        Arc::clone(&self.transformed)
    }

    pub const fn receipt(&self) -> &MemberRunInterchangeReceipt {
        &self.receipt
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemberRunInterchangeReceipt {
    source_selected: SelectedInstructionPlanIdentity,
    transformed_selected: SelectedInstructionPlanIdentity,
    optimization_unit: OptimizationUnitIdentity,
    fuel_schedule: FuelScheduleIdentity,
}

impl MemberRunInterchangeReceipt {
    pub const fn source_selected(&self) -> SelectedInstructionPlanIdentity {
        self.source_selected
    }
    pub const fn transformed_selected(&self) -> SelectedInstructionPlanIdentity {
        self.transformed_selected
    }
    pub const fn optimization_unit(&self) -> OptimizationUnitIdentity {
        self.optimization_unit
    }
    pub const fn fuel_schedule(&self) -> FuelScheduleIdentity {
        self.fuel_schedule
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemberRunInterchangeError {
    SourceMismatch,
    UnsupportedInstruction,
    /// The named instructions do not bound a member and an admissible run
    /// in one block: absent, a one-member run, a member inside or equal to
    /// the run's span, or a misordered bound; a member coupled by a
    /// register or condition-state hazard with a crossed position; a
    /// roster-carrying side sharing the window with a second memory-access
    /// actor; or a boundary settlement inside the window's span.
    UnsupportedPair,
    WorkBudgetExceeded,
    IdentityOverflow,
    ReplayMismatch,
}

impl std::fmt::Display for MemberRunInterchangeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "invalid member-against-run interchange: {self:?}"
        )
    }
}

impl std::error::Error for MemberRunInterchangeError {}
