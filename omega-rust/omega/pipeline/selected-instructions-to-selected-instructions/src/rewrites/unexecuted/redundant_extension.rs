//! Redundant carrier-extension removal on the selected CFG.
//!
//! Selection emits `ZeroExtend*`/`SignExtend*` wherever a narrow scalar crosses
//! a boundary requiring a normalized full-register carrier
//! (`integer_carrier_normalization`: ABI inputs, call arguments, modular
//! arithmetic results). When the input's producer already guarantees the same
//! normalization, the extension cannot change the register's bits; the rewrite
//! then replaces it with `CopyI64`, keeping the result register, instruction
//! identity, and provenance so allocation can coalesce the move.
//!
//! The admission table is bit-exact. A producer either guarantees every result
//! bit at or above `width` is zero (`Load8`, `Load8Indexed`, `Load16`,
//! `Load32`, `ZeroExtendU8`, `ZeroExtendU16`, and the `MaterializeBoolean*`
//! forms whose result is zero or one), guarantees the bits at or above
//! `width - 1` replicate bit `width - 1` (`SignExtendI8`, `SignExtendI16`,
//! `SignExtendI32`), publishes its exact pattern (`MaterializeI64`), or
//! promises a meaningful low `width` with the upper bits left unmeaningful
//! (`ZeroExtendU32`, `LoadPacked`, `Float32ToBits`). A zero extension of
//! width `m` is then the identity on a producer zero-normalized to
//! `n <= m`, on a pattern below `2^m`, or — for `m = 32` alone, the one
//! extension whose own result stays partial — on a partial carrier of
//! `n <= 32`; a sign extension of width `m` is the identity on a producer
//! sign-normalized to `n <= m`, zero-normalized to `n < m` (bit `m - 1` is
//! already zero), or on a pattern surviving the sign-extension round-trip.
//!
//! Partial carriers are the second producer class. `ZeroExtendU32` declares
//! its upper result bits "not meaningful" rather than zero, `LoadPacked`'s
//! assembled result names only its loaded bytes, and `Float32ToBits`
//! preserves a payload without naming the upper GPR bits. None of them can
//! witness an extension whose result fixes the high bits, but `ZeroExtendU32`
//! itself promises nothing there — so when the producer's meaningful content
//! already fits in 32 bits, that extension is the identity and becomes the
//! copy. A wider partial producer genuinely narrows and refuses, as does a
//! packed load's undocumented scratch register, which shares the defining
//! instruction without carrying its guarantee.
//!
//! Producers whose contracts say nothing about high bits in either direction
//! — ordinary arithmetic, copies, calls, and address forms — refuse, as does
//! a second definition of the input, including a `UseDef` rewrite: the
//! guarantee must hold wherever the extension reads the register, not only
//! at one definition site.
//!
//! Validation shares nothing with the producer's `admission` routine. It
//! re-derives the pair's legality from the source records — the extension
//! form, the operand and constraint shapes, the result register's origin, the
//! input's unique producer and its promised normalization, the identity table,
//! and the target's copy row — requires the proposed function to equal the one
//! the contract demands, and restores the complete source by content: every
//! other instruction, register, roster row, call, and settlement is retained
//! bit-identical.

mod admission;
mod rewrite;
mod validation;

use std::sync::Arc;

use optimization_core::OptimizationUnitIdentity;
use selected_instructions::{SelectedInstructionPlan, SelectedInstructionPlanIdentity};
use semantic_vocabulary::FuelScheduleIdentity;

pub use rewrite::remove_selected_redundant_extension;
pub use validation::validate_redundant_extension_removal;

#[cfg(test)]
mod tests;

/// An accepted redundant-extension removal with its replay receipt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedRedundantExtension {
    transformed: Arc<SelectedInstructionPlan>,
    receipt: RedundantExtensionReceipt,
}

impl ValidatedRedundantExtension {
    pub fn transformed(&self) -> &SelectedInstructionPlan {
        &self.transformed
    }

    pub fn shared_transformed(&self) -> Arc<SelectedInstructionPlan> {
        Arc::clone(&self.transformed)
    }

    pub const fn receipt(&self) -> &RedundantExtensionReceipt {
        &self.receipt
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RedundantExtensionReceipt {
    source_selected: SelectedInstructionPlanIdentity,
    transformed_selected: SelectedInstructionPlanIdentity,
    optimization_unit: OptimizationUnitIdentity,
    fuel_schedule: FuelScheduleIdentity,
}

impl RedundantExtensionReceipt {
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
pub enum RedundantExtensionError {
    SourceMismatch,
    UnsupportedInstruction,
    UnsupportedProducer,
    UnsupportedUse,
    ConstraintMismatch,
    WorkBudgetExceeded,
    IdentityOverflow,
    ReplayMismatch,
}

impl std::fmt::Display for RedundantExtensionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid redundant-extension removal: {self:?}")
    }
}

impl std::error::Error for RedundantExtensionError {}
