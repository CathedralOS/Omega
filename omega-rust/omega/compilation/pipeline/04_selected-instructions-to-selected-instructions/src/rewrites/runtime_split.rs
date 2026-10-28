//! Runtime live-range splits at fixed-view definitions and fixed uses: recovery
//! the allocator orders before rematerialization and private storage.
//!
//! One virtual register receives one home for its whole live range. When an
//! instruction pins its result to an exact physical view — x86-64 division
//! leaves its quotient in `rax`, a call leaves its result in the return
//! register — that pin extends over every later point the value is live,
//! including points where another value's own fixed use needs the same view.
//! No single home then exists, and allocation reports `NoCompatibleHome`
//! even though the machine has free registers. The split ends the pinned
//! segment at the definition: one `CopyI64` immediately after the defining
//! instruction moves the value into a fresh, unconstrained register, and
//! every later reference reads that register instead. The original register
//! keeps its fixed home for exactly the definition-to-copy window; the fresh
//! segment is homed independently by ordinary assignment. No reload
//! interval and no stack slot are created, so a successful split replaces
//! the private storage a spill would otherwise realize.
//!
//! Soundness rests on single definition and dominance: the victim is an
//! instruction result written by exactly one operand, the copy runs
//! immediately after that write, and the definition block dominates every
//! use block. The fresh register therefore holds the victim's value at every
//! point a use can observe it. A split whose definition carries no fixed view
//! could never free a home — the fresh register would inherit the same
//! interval minus one point — so admission requires the pin; it is the
//! pressure reason, not a cosmetic filter.
//!
//! Use custody: instruction and terminator operand uses are rebound to the
//! fresh register with their own fixed views, ties onto them, and classes
//! unchanged, so an ABI-pinned consumer still reads its exact unit. Outgoing
//! register value bindings are rebound when their semantic argument is the
//! victim's source value. Structural transports, case payloads, and
//! borrowed dispatch pointers naming the victim stay rejected: they carry
//! register identity as edge evidence this rewrite does not relocate.
//! Redefinitions, tied or early-clobber references to the victim, and a
//! tied or early-clobber definition stay rejected as well; lifting them is
//! the next restriction on the owning board item.
//!
//! Validation re-derives the same contract from the source records on its own
//! audit and never consults the producer's admission routine. It then
//! requires the proposal to carry exactly the one copy, the one fresh
//! register and the rebound references that audit derives, with boundary
//! settlements after the definition shifted by the one insertion, and
//! requires stripping those additions to restore the complete source by
//! content.

//! A separate fixed-use leg serves an empty whole-range domain: one untied
//! integer read receives a copy immediately before its consumer. The source
//! lifetime, other references and every incoming edge stay intact; the copy
//! dominates its one read even in cyclic functions. Allocation rebuilds the
//! current graph's analyses and homes after either leg. This is constraint
//! refinement, not permission to reinterpret an empty domain as pressure.

mod admission;
mod before_use;
mod rewrite;
mod validation;

use std::sync::Arc;

use optimization_core::OptimizationUnitIdentity;
use selected_instructions::{SelectedInstructionPlan, SelectedInstructionPlanIdentity};
use semantic_vocabulary::FuelScheduleIdentity;

pub(crate) use admission::visit_structural_registers;
pub use rewrite::split_selected_runtime_value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedRuntimeSplit {
    transformed: Arc<SelectedInstructionPlan>,
    receipt: RuntimeSplitReceipt,
}

validated_transformed_receipt!(ValidatedRuntimeSplit, RuntimeSplitReceipt);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeSplitReceipt {
    source_selected: SelectedInstructionPlanIdentity,
    transformed_selected: SelectedInstructionPlanIdentity,
    optimization_unit: OptimizationUnitIdentity,
    fuel_schedule: FuelScheduleIdentity,
}

receipt_selected_identities!(RuntimeSplitReceipt);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeSplitError {
    SourceMismatch,
    UnsupportedValue,
    UnsupportedUse,
    ConstraintMismatch,
    WorkBudgetExceeded,
    IdentityOverflow,
    ReplayMismatch,
}

impl RuntimeSplitError {
    /// Whether the victim is simply outside the split family. Recovery
    /// treats these as "try the next decision"; every other variant is a
    /// malformed input or budget failure that must surface.
    pub const fn is_inadmissible(self) -> bool {
        matches!(self, Self::UnsupportedValue | Self::UnsupportedUse)
    }
}

failed_display!(RuntimeSplitError, "invalid runtime live-range split");

/// Every boundary-settlement position strictly after the definition moves
/// past the one inserted copy; positions at or before it are unchanged. The
/// copy is part of the definition's step, so no settlement can land between
/// the pinned write and the move that ends its pinned segment.
pub(crate) const fn shifted_position(position: u32, definition_index: u32) -> Option<u32> {
    if position > definition_index {
        position.checked_add(1)
    } else {
        Some(position)
    }
}
