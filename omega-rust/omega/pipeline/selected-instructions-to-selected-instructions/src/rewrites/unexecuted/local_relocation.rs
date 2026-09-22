//! Local instruction relocation on the selected CFG.
//!
//! Relocation here means moving one body instruction to a new position
//! inside its block without changing what any instruction observes. The
//! primitive this family proves is the bounded one-sided move: the named
//! `member` takes the named `destination` instruction's position, and every
//! instruction between them — the destination included — shifts one slot
//! toward the member's vacated position. The window the move crosses
//! rotates by one slot rather than trading two endpoints: relocation
//! admits a row-less member passing any number of roster-carrying
//! instructions, which the pairwise interchange refuses whenever the
//! destination member carries rows beside an accounted interior, while the
//! interchange keeps every interior position fixed, which a relocation
//! never does past distance one. An adjacent destination degenerates to
//! the adjacent interchange. Every other instruction, register, roster
//! row, call, settlement, and edge is retained bit-identical, and replayed
//! restore-by-content validation confirms the proposal is exactly that
//! rotation and nothing else.
//!
//! A relocation is admitted only when the window the move crosses is
//! independent. The hazard audit is one-sided: the member is the only
//! instruction whose position relative to the crossed run changes, so a
//! data hazard is any register or condition-state unit the member writes
//! and a crossed instruction reads or also writes, or the member reads
//! and a crossed instruction writes — the same access-mark plus
//! implicit-use, implicit-def, and clobber accounting the interchange
//! proves, applied between the member and every crossed position. Crossed
//! instructions keep their relative order, so hazards between them cannot
//! change, and since no location is written by both the member and a
//! crossed instruction the last writer before every later observer —
//! including the block's terminator and edge transports — is the same
//! instruction it was before the move. Positions before the window are
//! untouched.
//!
//! Positional observers bound the window the same way the interchange's
//! does. A boundary settlement at any position after the window's first
//! index through its last observes a different executed prefix once the
//! member lands — the member joins the prefix from the trailing side or
//! leaves it from the leading side — so a settlement inside the window's
//! span refuses; settlements at or outside the span observe the same
//! executed set either way. Calls, hosted effects, and terminator kinds
//! are barriers: they never relocate and never sit inside a crossed
//! window, and an instruction named by the call roster refuses for the
//! same reason.
//!
//! Memory ordering follows the validated `memory_accesses` roster with the
//! same completeness discipline as the interchange: a kind that can reach
//! semantic or place-backed storage must carry the rows accounting for
//! that reach. The member trades order with every crossed position, so a
//! row-carrying member may cross only row-less positions — a second
//! accounted actor anywhere in the window would need a place-alias
//! decision this step does not take. A row-less member cannot observe
//! memory at all under the roster's completeness, so it crosses any mix
//! of accounted and interior positions while every recorded access in the
//! program keeps its relative order. A memory-capable kind without rows
//! is an unaccounted access and refuses.
//!
//! No register roster row, constraint key, or instruction identity
//! changes: the member keeps its own id, kind, operands, provenance, and
//! implicit surface while only its position in the block vector moves.
//!
//! Proposal and independent replay share only the admission predicates.
//! Validation consumes the proposed program, requires the touched block to
//! equal the independently computed rotation, and restores the complete
//! source by content.

mod admission;
mod rewrite;
mod validation;

use std::sync::Arc;

use optimization_core::OptimizationUnitIdentity;
use selected_instructions::{SelectedInstructionPlan, SelectedInstructionPlanIdentity};
use semantic_vocabulary::FuelScheduleIdentity;

pub use rewrite::relocate_selected_instruction;
pub use validation::validate_local_relocation;

#[cfg(test)]
mod tests;

/// An accepted relocation with its replay receipt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedLocalRelocation {
    transformed: Arc<SelectedInstructionPlan>,
    receipt: LocalRelocationReceipt,
}

impl ValidatedLocalRelocation {
    pub fn transformed(&self) -> &SelectedInstructionPlan {
        &self.transformed
    }

    pub fn shared_transformed(&self) -> Arc<SelectedInstructionPlan> {
        Arc::clone(&self.transformed)
    }

    pub const fn receipt(&self) -> &LocalRelocationReceipt {
        &self.receipt
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalRelocationReceipt {
    source_selected: SelectedInstructionPlanIdentity,
    transformed_selected: SelectedInstructionPlanIdentity,
    optimization_unit: OptimizationUnitIdentity,
    fuel_schedule: FuelScheduleIdentity,
}

impl LocalRelocationReceipt {
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
pub enum LocalRelocationError {
    SourceMismatch,
    UnsupportedInstruction,
    /// The named instructions do not bound an admissible in-block window:
    /// absent or coincident in the member's block, the member coupled by a
    /// register or condition-state hazard with a crossed instruction, a
    /// roster-carrying member sharing the window with a second
    /// memory-access actor, or a boundary settlement inside the window's
    /// span.
    UnsupportedPair,
    WorkBudgetExceeded,
    IdentityOverflow,
    ReplayMismatch,
}

impl std::fmt::Display for LocalRelocationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid local instruction relocation: {self:?}")
    }
}

impl std::error::Error for LocalRelocationError {}
