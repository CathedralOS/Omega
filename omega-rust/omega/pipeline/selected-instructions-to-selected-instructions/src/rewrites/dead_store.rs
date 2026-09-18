//! Borrow-aware dead-store elimination on the selected CFG.
//!
//! A `Store { byte_offset, byte_size }` writes the low exact-width bits of
//! its value operand through the referent pointer, and a `StorePacked`
//! writes its packed width the same way; either can be the dead store, the
//! packed form only when its early-clobber scratch `Def` occurs nowhere
//! else in the function — a surviving mention would lose its definition to
//! the removal. Both place stores can also write the place's own local
//! storage — its `StructuralParameter`/`StructuralBlockParameter` slot, or
//! the producing operation's `Structural` home, each the place's storage
//! under the same byte coordinates — through the slot's materialized
//! address, and a `Store64` writes that slot directly; a dead store
//! carrying the `WriteLocal` row on such a slot is removed the same way.
//! The byte-sequence store is the last dead-store route: a `Store { 0, 1 }`
//! through a fully computed view address carries one `WriteByteSequence`
//! row, so its dead extent is dynamic — the written byte sits at the row's
//! `byte_offset + index`, unbounded upward from that fixed offset.
//! A `Structural` slot the place's declaration does not charge to that
//! operation only stages bytes that name the place and stays inadmissible.
//! A later write of the place's storage whose byte range
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
//! itself matters: an overlapping read observes the bytes, a partial write
//! leaves them observable, and a
//! materialized local address can reach the same storage by another route.
//! A dynamic-extent row reaches only upward from its fixed byte offset — a
//! span covers `length` bytes there and a sequence row touches
//! `offset + index` — so one whose offset starts below the dead range's
//! end still reaches its last bytes and interferes like an overlapping
//! row, while one starting at or past the end is provably disjoint and
//! walks past like any disjoint row. When the dead extent is itself
//! dynamic the directions mirror: an exact or local row still reaches the
//! dead byte once its own extent ends past the row's fixed offset, and a
//! dynamic-extent row on the dead place always meets it.
//! The covering write must be the first access on the dead place after the
//! removed store, and the only row reaching the dead range must contain it
//! entirely — the row need only cover, not equal, the removed write. Two
//! routes to the place's storage qualify: a `Store` or `StorePacked`
//! carrying `WritePlace`, and a write into the place's own local storage
//! carrying `WriteLocal` — its `StructuralParameter`/`StructuralBlockParameter`
//! slot, or the `Structural` slot of the operation the place's declaration
//! names as its producer — either a `Store64` naming that slot or a place
//! store through its materialized address. Any other `Structural`
//! operation slot only stages bytes that merely name the place — a call's
//! staged view descriptor — so its writes and addresses are not the dead
//! bytes at all.
//! A `CopyBytes` into the place covers too — the only covering write with
//! several roster rows — when its destination `WriteByteSpan` is the sole
//! row reaching the dead range and its count register resolves to a clean
//! `MaterializeI64`: the span then writes a compile-time `count` bytes at
//! its fixed offset, and coverage is containment on constants. The copy's
//! other rows — its source read among them — must stay quiet on the dead
//! range, since a read reaching it would observe the bytes first.
//! A byte-sequence store covers an exact dead range the same way once its
//! index is itself constant: the `index` value's sole `InstructionResult`
//! register must resolve to a clean `MaterializeI64` with no
//! edge-transport or case-payload redefinition, so the write lands on the
//! one fixed byte `byte_offset + index` — and a single dead byte is covered
//! exactly when that is it. A runtime index still lands the write anywhere
//! at or past the payload base and cannot provably rewrite one fixed byte.
//! A byte-sequence dead store is covered only by another byte-sequence
//! store whose `WriteByteSequence` row spells the same byte: the same
//! payload base and the same `index` value place it exactly, and distinct
//! index values still do when each resolves to a clean `MaterializeI64`
//! under the same carrier audit the exact-range route runs — the two
//! `byte_offset + index` sums then name one fixed position apiece, and
//! equal sums are the same byte. Any exact or local row would have to
//! contain a byte placed at runtime, and a sequence write whose index
//! stays runtime or lands elsewhere may land on a different byte
//! entirely.
//!
//! Instructions inserted by private-slot rewrites (spill stores, reloads,
//! frame addresses over `Spill`/`Boundary` slots) carry no roster row; they
//! touch compiler-owned activation storage that no referent place aliases, so
//! they can neither observe nor overwrite the dead bytes. Every other
//! memory-capable instruction without a row, every call, and every hosted
//! effect conservatively blocks elimination.
//!
//! The walk is not confined to one block: reaching a block's end without
//! interference continues through every successor edge of its terminator —
//! a covering write further down still rewrites the dead bytes when each
//! path forward reaches one before any observer. A block is only ever
//! entered uncovered, so its first interfering access decides all paths
//! through it at once; a block scanned clear defers coverage to its
//! distinct successors — a fork's legs may cover through different writes
//! or reconverge on one — and a join at a crossed block is harmless because
//! coverage looks forward. Terminator roster rows decide before each
//! crossed edge's transports, which may not write or retire the dead
//! place's storage. A terminator without successors and an edge back into
//! the store's own block — the removed store can never be its own covering
//! write — each end the walk in rejection.
//!
//! Coverage need not be proven on every path. The walked region is closed
//! under successor edges, so a path that never reaches a covering write is
//! confined to clear blocks forever — no access, transport, or terminator
//! on it can observe the dead bytes — and the store is dead on that path
//! too. Legs whose paths disagree about coverage and writerless cycles
//! admit on the same argument: the bytes are either rewritten before any
//! observer or never observable again.
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
