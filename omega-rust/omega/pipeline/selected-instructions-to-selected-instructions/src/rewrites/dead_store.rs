//! Borrow-aware dead-store elimination on the selected CFG.
//!
//! A `Store { byte_offset, byte_size }` writes the low exact-width bits of
//! its value operand through the referent pointer, and a `StorePacked`
//! writes its packed width the same way; either can be the dead store, the
//! packed form only when its early-clobber scratch `Def` occurs nowhere
//! else in the function — a surviving mention would lose its definition to
//! the removal. Both place stores can also write the place's own local
//! storage — its `StructuralParameter` or `StructuralBlockParameter` slot,
//! which is the place's storage under the same byte coordinates — through
//! the slot's materialized address, and a `Store64` writes that slot
//! directly; a dead store carrying the `WriteLocal` row on such a slot is
//! removed the same way, while a `WriteLocal` on an operation-owned
//! `Structural` slot only stages bytes that name the place and stays
//! inadmissible. A later write of the place's storage whose byte range
//! covers the first store's range entirely replaces those bytes. When no
//! intervening instruction can observe or partially overwrite the first
//! store's bytes, the earlier store is dead: every observer of the place
//! sees the covering write's value, so the first instruction and its roster
//! row can be removed without changing any reachable memory state.
//!
//! The alias decision is borrow-aware: it comes from the validated
//! `memory_accesses` roster, not from pointer-register equality. Each access
//! row names the semantic `PlaceId` and the exact byte range the instruction
//! touches. Two simultaneous accesses that can write cannot share one
//! referent under different place identities — exclusivity rejects
//! overlapping exclusive custody before selection — so a write row for a
//! different `PlaceId` cannot disturb the dead bytes, and a read row for a
//! different place cannot observe them. Only an access on the dead place
//! itself matters: an overlapping or dynamic-extent read observes the bytes,
//! a partial or dynamic-extent write leaves them observable, and a
//! materialized local address can reach the same storage by another route.
//! The covering write must be the first access on the dead place after the
//! removed store, carrying exactly one row on the dead place whose encoded
//! byte range contains the dead range entirely — the row need only cover,
//! not equal, the removed write. Two routes to the place's storage qualify:
//! a `Store` or `StorePacked` carrying `WritePlace`, and a write into the
//! place's own local storage — its `StructuralParameter` or
//! `StructuralBlockParameter` slot — carrying `WriteLocal`, either a
//! `Store64` naming that slot or a place store through its materialized
//! address. A `Structural` operation slot can instead stage bytes that
//! merely name the place — a call's staged view descriptor — so a write to
//! it stays interference rather than a route to the place's storage.
//!
//! Instructions inserted by private-slot rewrites (spill stores, reloads,
//! frame addresses over `Spill`/`Boundary` slots) carry no roster row; they
//! touch compiler-owned activation storage that no referent place aliases, so
//! they can neither observe nor overwrite the dead bytes. Every other
//! memory-capable instruction without a row, every call, and every hosted
//! effect conservatively blocks elimination.
//!
//! The walk is not confined to one block: reaching a block's end without
//! interference continues through its terminator's successor edges when every
//! edge names one block — each path forward from the store then arrives
//! there, so a covering store in that block still rewrites the dead bytes
//! before any observer. A join at a crossed block is harmless because
//! coverage looks forward. Terminator roster rows decide before each crossed
//! edge's transports, which may not write or retire the dead place's storage.
//! A terminator without successors, edges fanning out to distinct blocks, and
//! re-entering a walked block each leave a path the covering store never runs
//! on, so they end the walk unproven.
//!
//! Removing the store shortens its block's instruction vector, so boundary
//! settlements positioned after it shift one ordinal earlier. Settlement
//! positions inside the dead interval — after the removed store in its block,
//! anywhere in a fully crossed block, or at or before the covering store in
//! its block — are rejected outright: a boundary event in that interval could
//! observe the dead bytes.
//! Proposal and independent replay share only the admission predicates and
//! the settlement remap. Validation consumes the proposed program, requires
//! the block, roster, and settlements to equal the independently computed
//! removals, and restores the complete source by content.

mod admission;
mod rewrite;
mod validation;

use std::sync::Arc;

use optimization_core::OptimizationUnitIdentity;
use selected_instructions::{SelectedInstructionPlan, SelectedInstructionPlanIdentity};
use semantic_vocabulary::FuelScheduleIdentity;

pub use rewrite::eliminate_selected_dead_store;
pub use validation::validate_dead_store_elimination;

#[cfg(test)]
mod tests;

/// An accepted dead-store elimination with its replay receipt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedDeadStoreElimination {
    transformed: Arc<SelectedInstructionPlan>,
    receipt: DeadStoreEliminationReceipt,
}

impl ValidatedDeadStoreElimination {
    pub fn transformed(&self) -> &SelectedInstructionPlan {
        &self.transformed
    }

    pub fn shared_transformed(&self) -> Arc<SelectedInstructionPlan> {
        Arc::clone(&self.transformed)
    }

    pub const fn receipt(&self) -> &DeadStoreEliminationReceipt {
        &self.receipt
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeadStoreEliminationReceipt {
    source_selected: SelectedInstructionPlanIdentity,
    transformed_selected: SelectedInstructionPlanIdentity,
    optimization_unit: OptimizationUnitIdentity,
    fuel_schedule: FuelScheduleIdentity,
}

impl DeadStoreEliminationReceipt {
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
pub enum DeadStoreEliminationError {
    SourceMismatch,
    UnsupportedInstruction,
    UnsupportedPair,
    InterveningAccess,
    ConstraintMismatch,
    WorkBudgetExceeded,
    IdentityOverflow,
    ReplayMismatch,
}

impl std::fmt::Display for DeadStoreEliminationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid dead-store elimination: {self:?}")
    }
}

impl std::error::Error for DeadStoreEliminationError {}
