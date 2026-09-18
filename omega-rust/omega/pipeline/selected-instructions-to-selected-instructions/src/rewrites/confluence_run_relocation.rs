//! Run relocation into a confluence on the selected CFG.
//!
//! The in-block `rewrites/run_relocation` moves the contiguous run two
//! named members bound inside one body block, and the downstream
//! cross-boundary families carry that run across the shapes whose every
//! traversal still executes it once — `rewrites/edge_run_relocation`,
//! `rewrites/predecessor_run_relocation`,
//! `rewrites/diamond_run_relocation`, and
//! `rewrites/bypass_run_relocation`. This family is the run move carried
//! across the remaining converging shape the single-edge move refuses —
//! the confluence: the named `first_member` and `last_member` bound one
//! contiguous run of at least two members in a block's body whose
//! terminator is a lone `Jump` to a join at least one other
//! predecessor's edge also reaches, and the run leaves that body as a
//! single body — its members keeping their own order — and takes the
//! named `destination` instruction's position in the join. The
//! destination and every later position keep their relative order one
//! run-width later, so the run lands exactly where the destination sat
//! and every position the move crosses keeps its relative order.
//!
//! Moving the run as one body admits the window the member move refuses
//! on internal coupling alone: a producer whose only crossed reader is
//! the run's own next member cannot leave its block — the member move
//! would starve the consumer it leaves behind — while the run carries
//! the consumer with it, so internally coupled members cross the
//! confluence together and only positions outside the run count as
//! crossed.
//!
//! Sinking a run into a confluence carries the member move's flipped
//! soundness burden at body granularity. The join's other inflows hand
//! the run's new position traversals it never executed on — speculation
//! downstream — so every member must be pure register and
//! condition-state work that can never fault: no barrier kind, no
//! call-roster entry, no memory roster rows, and no memory-capable or
//! potentially-faulting kind, because an access or trap that ran only on
//! this inflow would newly run on every arrival. And every register and
//! condition-state unit any member writes must be dead — unread until
//! rewritten — from the landing index forward: the join's tail, its
//! terminator, and every reachable successor position are shared by all
//! inflows, so a reader there observes the run's foreign definitions on
//! an arrival that never ran it. The forward dead-path audit walks the
//! join from the landing index, where the run's new position republishes
//! its members' locations on every arrival, and propagates them through
//! the successor region — reading each crossed edge's register,
//! structural-binding, and case-payload transports as boundary positions
//! — refusing the moment a still-live member definition meets a reader.
//! On this inflow's own path the relocated writes are the familiar ones
//! the source produced, but the shared region cannot tell the arrivals
//! apart, so the audit holds the die-unread rule on every continuation.
//!
//! The source side needs no region structure beyond the lone exit: the
//! run's block is any block ending in a plain `Jump` to the join — a
//! conditional terminator keeps a second exit the run would still
//! execute on, which is the fork, diamond, and triangle families'
//! shapes. The join needs only the in-edge count the single-edge family
//! excludes — at least one edge leaving another block — and the usual
//! boundary honesty: not the run's own block, which is the in-block
//! family's case with a back-edge reading, not the entry block, which is
//! reached with no predecessor at all, and a plain source block, because
//! an implementation block's origin carries edge or case work the
//! bounded audit does not cross. The crossed edge itself must be a plain
//! semantic successor: case custody, fuel, and structural transfers are
//! boundary effects the move does not cross.
//!
//! Between the run's old span and its new one lie only the crossed
//! positions the window audits: the run's own block tail behind it, the
//! `Jump` terminator instruction, the edge's register transports, and
//! the join's body instructions before the landing index. Positions
//! before the run, at or after the landing index, and in every other
//! block keep the run on the side they always had, so they are never
//! crossed. The join's other inflow edges are never traversed by the
//! run's old position — they run before the landing index on their own
//! arrivals — so their transports join only the dead-path audit's
//! boundary reading where loops carry the foreign set back around.
//!
//! The hazard audit is the member move's applied across the boundary at
//! run granularity: a register or condition-state unit any member writes
//! and a crossed instruction reads or writes, or any member reads and a
//! crossed instruction writes, refuses in either direction — which also
//! pins each member's inputs, so the run's execution at the landing
//! index on this inflow's path computes the values the old positions
//! computed. The edge's register transports join the audit per member
//! rather than through an instruction: a `Registers` transport reads its
//! argument and writes its parameter at the boundary, so a member
//! defining the transported argument would hand the binding the pre-move
//! value where the source bound the member's own write, a member
//! defining the parameter would be overwritten before the join observes
//! it, and a member reading the parameter would observe the transported
//! value only after the move. Calls, hosted effects, terminator kinds,
//! and call-roster entries are barriers anywhere in the window — the
//! `Jump` terminator itself is the crossed edge's position, not a window
//! member — and a boundary settlement refuses exactly where an executed
//! prefix changes: past the run's first index in its own block, or past
//! the landing index in the join block.
//!
//! No instruction, register, roster row, call, settlement, successor, or
//! terminator record changes: every member keeps its own id, kind,
//! operands, provenance, and implicit surface while only the run's
//! position in the program moves. Proposal and independent replay share
//! only the admission predicates. Validation consumes the proposed
//! program, requires the run to sit at the landing index in its original
//! order, and restores the complete source by content — every other
//! block, instruction, register, roster row, call, and settlement is
//! retained bit-identical.

mod admission;
mod rewrite;
mod validation;

use std::sync::Arc;

use optimization_core::OptimizationUnitIdentity;
use selected_instructions::{SelectedInstructionPlan, SelectedInstructionPlanIdentity};
use semantic_vocabulary::FuelScheduleIdentity;

pub use rewrite::relocate_selected_run_into_confluence;
pub use validation::validate_confluence_run_relocation;

#[cfg(test)]
mod tests;

/// An accepted cross-confluence run relocation with its replay receipt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedConfluenceRunRelocation {
    transformed: Arc<SelectedInstructionPlan>,
    receipt: ConfluenceRunRelocationReceipt,
}

impl ValidatedConfluenceRunRelocation {
    pub fn transformed(&self) -> &SelectedInstructionPlan {
        &self.transformed
    }

    pub fn shared_transformed(&self) -> Arc<SelectedInstructionPlan> {
        Arc::clone(&self.transformed)
    }

    pub const fn receipt(&self) -> &ConfluenceRunRelocationReceipt {
        &self.receipt
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfluenceRunRelocationReceipt {
    source_selected: SelectedInstructionPlanIdentity,
    transformed_selected: SelectedInstructionPlanIdentity,
    optimization_unit: OptimizationUnitIdentity,
    fuel_schedule: FuelScheduleIdentity,
}

impl ConfluenceRunRelocationReceipt {
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
pub enum ConfluenceRunRelocationError {
    SourceMismatch,
    /// A barrier kind, a call-roster entry, a roster-carrying or
    /// unaccounted memory-capable kind, or a potentially-faulting kind
    /// sits at a position the move would cross — a run member included:
    /// an access or trap that ran only on this inflow would newly run on
    /// every arrival through the join's other inflows.
    UnsupportedInstruction,
    /// The named triple does not bound an admissible cross-confluence run
    /// window: the named members bound no contiguous run of at least two
    /// members in one block's body, the run's block does not end in a
    /// lone unconditional `Jump`, the crossed edge is not a plain
    /// semantic successor or carries case, fuel, or structural
    /// transfers, the join is the run's block, the entry block, a
    /// non-source block, or reached by no edge leaving another block —
    /// the sole-predecessor shape is the edge family's case — the
    /// destination names no body or terminator instruction there, a
    /// register or condition-state hazard couples a member with a
    /// crossed position or the crossed edge's transports, a boundary
    /// settlement observes a changed executed prefix, or the dead-path
    /// audit finds a register or unit a member writes still live at a
    /// reader on a shared continuation.
    UnsupportedPair,
    WorkBudgetExceeded,
    IdentityOverflow,
    ReplayMismatch,
}

impl std::fmt::Display for ConfluenceRunRelocationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid confluence run relocation: {self:?}")
    }
}

impl std::error::Error for ConfluenceRunRelocationError {}
