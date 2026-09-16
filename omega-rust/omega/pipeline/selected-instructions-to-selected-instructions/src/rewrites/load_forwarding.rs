//! Borrow-aware store-to-load forwarding on the selected CFG.
//!
//! A `Store { byte_offset, byte_size }` writes the low exact-width bits of
//! its value operand through the referent pointer. A later `Load64`/`Load32`/
//! `Load16`/`Load8` of the same width at the same byte offset observes
//! exactly those bytes when no intervening instruction can write or expose
//! the same storage. The rewrite replaces the load with a
//! `CopyI64` of the stored register at full width, or the matching
//! `ZeroExtend` at sub-word width: a same-width store-then-load round-trips
//! the register's low bits in the target's own byte order, so no endianness
//! assumption is needed. The loaded register keeps its identity, origin, and
//! the read's provenance.
//!
//! The store and the load need not share a block: when a block's top is
//! reached without interference, the walk crosses into every predecessor
//! block, and the block resolves once each predecessor path's last writer
//! stored the same register — one store dominating the join, or each leg's
//! own store of that register. The function entry, a block no edge reaches,
//! and a deferred region that never reaches one register — including a
//! self-loop or a writerless cycle — each leave the walk without a pair.
//! Crossed edges are admitted only when their transports move neither the
//! carried registers nor storage the forwarded place can reach.
//!
//! The alias decision is borrow-aware: it comes from the validated
//! `memory_accesses` roster, not from pointer-register equality. Each access
//! row names the semantic `PlaceId` (a referent root; field projections retain
//! the root and differ only in `byte_offset`) and the exact byte range the
//! instruction touches. Two simultaneous accesses that can write cannot share
//! one referent under different place identities — exclusivity rejects
//! overlapping exclusive custody before selection — so a write row or a
//! place-backed local slot for a different `PlaceId` cannot disturb the
//! forwarded bytes. Intervening reads of the same place cannot either; only a
//! potentially overlapping write, a dynamic-extent access, an escaped
//! place-backed address, or a call/host effect blocks the pair.
//!
//! Instructions inserted by private-slot rewrites (spill stores, reloads,
//! frame addresses over `Spill`/`Boundary` slots) carry no roster row; they
//! touch compiler-owned activation storage that no referent place aliases.
//! Every other memory-capable instruction without a row, every call, and every
//! hosted effect conservatively blocks forwarding.
//!
//! Proposal and independent replay share only the admission predicates and
//! the forwarded-instruction constructor. Validation consumes the proposed
//! program, requires the rewritten instruction to equal the independently
//! reconstructed copy, requires the roster to be the source roster minus the
//! load's read row, and restores the complete source by content.

mod admission;
mod rewrite;
mod validation;

use std::sync::Arc;

use optimization_core::OptimizationUnitIdentity;
use selected_instructions::{SelectedInstructionPlan, SelectedInstructionPlanIdentity};
use semantic_vocabulary::FuelScheduleIdentity;

pub use rewrite::forward_selected_stored_load;
pub use validation::validate_stored_load_forwarding;

#[cfg(test)]
mod tests;

/// An accepted store-to-load forwarding with its replay receipt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedStoredLoadForwarding {
    transformed: Arc<SelectedInstructionPlan>,
    receipt: StoredLoadForwardingReceipt,
}

impl ValidatedStoredLoadForwarding {
    pub fn transformed(&self) -> &SelectedInstructionPlan {
        &self.transformed
    }

    pub fn shared_transformed(&self) -> Arc<SelectedInstructionPlan> {
        Arc::clone(&self.transformed)
    }

    pub const fn receipt(&self) -> &StoredLoadForwardingReceipt {
        &self.receipt
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredLoadForwardingReceipt {
    source_selected: SelectedInstructionPlanIdentity,
    transformed_selected: SelectedInstructionPlanIdentity,
    optimization_unit: OptimizationUnitIdentity,
    fuel_schedule: FuelScheduleIdentity,
}

impl StoredLoadForwardingReceipt {
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
pub enum StoredLoadForwardingError {
    SourceMismatch,
    UnsupportedInstruction,
    UnsupportedPair,
    UnsupportedUse,
    AliasingWrite,
    ConstraintMismatch,
    WorkBudgetExceeded,
    IdentityOverflow,
    ReplayMismatch,
}

impl std::fmt::Display for StoredLoadForwardingError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid stored-load forwarding: {self:?}")
    }
}

impl std::error::Error for StoredLoadForwardingError {}
