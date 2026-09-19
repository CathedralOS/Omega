//! Borrow-aware mutation motion on the selected CFG.
//!
//! A `Store { byte_offset, byte_size }` writes the low exact-width bits of
//! its value operand through the referent pointer — or through the
//! materialized address of the place's own local storage, its
//! `StructuralParameter`/`StructuralBlockParameter` slot or the producing
//! operation's `Structural` home, which a `Store64` also writes directly. A
//! `StorePacked { byte_offset, width }` takes the same pointer routes for
//! the odd fragment widths the plain store cannot encode. A `Store { 0, 1 }`
//! through a fully computed view address carries a `WriteByteSequence` row
//! instead: its written byte sits at the row's `byte_offset + index` for the
//! runtime `index`, so the moved extent is unbounded upward from that fixed
//! offset — unless the `index` resolves to a clean `MaterializeI64`, when
//! admission collapses the extent to the one byte `byte_offset + index`
//! before the walk.
//! A `Structural` slot the place's declaration does not charge to the
//! slot's operation only stages bytes that name the place under its own
//! slot coordinates: a store into it — the `Store`/`StorePacked` through
//! its materialized address or the direct `Store64` — is the moved store
//! of the slot's own bytes, and no place-named row can reach them. The
//! staging store sinks under the same walk, bounded by the first roster row
//! naming that very slot — a `WriteLocal` rewriting the staged bytes or an
//! `AddressLocal` materializing their address — rather than by the place's
//! own accesses.
//! Sinking the store later along the control-flow path defers the write
//! inside the window where nothing can observe the written bytes' old
//! contents: every instruction the store slides past must leave the
//! relative order of the write and every later access on the same storage
//! unchanged. The motion
//! lands the store immediately before the first position that must stay
//! ordered after it — the first roster access on the moved storage, a call or
//! hosted effect, a redefinition of a carried register, an unaccounted
//! memory-capable instruction, or a boundary settlement — or at the end of
//! the last block the walk can prove it still reaches on every path. The
//! packed form's early-clobber scratch `Def` and its declared clobbers move
//! with the instruction, so the same coupling that guards the carried reads
//! also proves their custody: a window instruction may not read or rewrite
//! the scratch, nor read or publish a condition-state unit the moved row
//! clobbers, and neither may a crossed edge transport or terminator.
//!
//! The alias decision is borrow-aware: it comes from the validated
//! `memory_accesses` roster, not from pointer-register equality. Each access
//! row names the semantic `PlaceId` and the exact byte range the instruction
//! touches. Two simultaneous accesses that can write cannot share one
//! referent under different place identities — exclusivity rejects
//! overlapping exclusive custody before selection — so rows for other places
//! and exact rows on disjoint ranges of the moved place cannot observe the
//! slide. A `WriteLocal` or `AddressLocal` row names a slot: the moved
//! bytes' storage — a parameter, block-parameter, or producer-declared
//! `Structural` home for a place subject, the staging slot itself for a
//! staging subject — interferes on them like a place row, while a slot
//! holding other bytes never reaches them. A dynamic-extent
//! row reaches only upward from its fixed byte offset — a span covers
//! `length` bytes there and a sequence row touches `offset + index` — so
//! one starting at or past the moved range's end is provably disjoint and
//! slides past like any disjoint row. A sequence row whose `index` resolves
//! to a clean `MaterializeI64` touches exactly the byte `offset + index`
//! instead, so it stops the slide only by landing inside the moved extent.
//! When the moved extent is itself
//! dynamic the direction mirrors: an exact or local row still reaches the
//! moved byte once its own extent ends past the row's fixed offset, and a
//! dynamic-extent row on the place always can — unless the moved `index`
//! resolved the same way, collapsing the extent to that one byte before the
//! walk. Only a
//! potentially overlapping access on the moved place, a dynamic-extent row
//! on it still able to reach the moved bytes, a write to or materialized
//! address of the place's own storage, or
//! a call/hosted effect bounds the window.
//!
//! The walk is not confined to one block: reaching a block's end without a
//! stop continues through its terminator's successor edges when every edge
//! names one block and that block's only predecessor is the crossed block.
//! Both directions of uniqueness are required — a fork out of the crossed
//! block would drop the write from the paths that leave it, and a join into
//! the successor would add the write to paths that never carried it. The
//! terminator's roster rows and register definitions decide before each
//! crossed edge's transports, which may not redefine the carried registers or
//! write the moved storage. Returns and hosted exits, forked or
//! joined targets, and re-entered blocks each bound the motion at the crossed
//! block's end.
//!
//! The store's roster row names the instruction by identity, not position, so
//! the access roster is retained unchanged. Boundary settlements are ordinal
//! positions: a settlement inside the crossed interval would change which
//! side of the write it observes, so the walk stops before it; settlements at
//! or after the landing position in the target block shift one ordinal later
//! when a crossed-edge move inserts there, while a same-block move leaves
//! every settlement position unchanged.
//!
//! Proposal and independent replay share only the admission predicates and
//! the settlement remap. Validation consumes the proposed program, requires
//! the touched blocks, roster, and settlements to equal the independently
//! computed motion, and restores the complete source by content.

mod admission;
mod rewrite;
mod validation;

use std::sync::Arc;

use optimization_core::OptimizationUnitIdentity;
use selected_instructions::{SelectedInstructionPlan, SelectedInstructionPlanIdentity};
use semantic_vocabulary::FuelScheduleIdentity;

pub use rewrite::sink_selected_store_mutation;
pub use validation::validate_store_mutation_motion;

#[cfg(test)]
mod tests;

/// An accepted store mutation motion with its replay receipt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedStoreMutationMotion {
    transformed: Arc<SelectedInstructionPlan>,
    receipt: StoreMutationMotionReceipt,
}

impl ValidatedStoreMutationMotion {
    pub fn transformed(&self) -> &SelectedInstructionPlan {
        &self.transformed
    }

    pub fn shared_transformed(&self) -> Arc<SelectedInstructionPlan> {
        Arc::clone(&self.transformed)
    }

    pub const fn receipt(&self) -> &StoreMutationMotionReceipt {
        &self.receipt
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreMutationMotionReceipt {
    source_selected: SelectedInstructionPlanIdentity,
    transformed_selected: SelectedInstructionPlanIdentity,
    optimization_unit: OptimizationUnitIdentity,
    fuel_schedule: FuelScheduleIdentity,
}

impl StoreMutationMotionReceipt {
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
pub enum StoreMutationMotionError {
    SourceMismatch,
    UnsupportedInstruction,
    UnsupportedPair,
    ConstraintMismatch,
    WorkBudgetExceeded,
    IdentityOverflow,
    ReplayMismatch,
}

impl std::fmt::Display for StoreMutationMotionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid store mutation motion: {self:?}")
    }
}

impl std::error::Error for StoreMutationMotionError {}
