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
//! `SignExtendI32`), or publishes its exact pattern (`MaterializeI64`). A zero
//! extension of width `m` is then the identity on a producer zero-normalized
//! to `n <= m` or on a pattern below `2^m`; a sign extension of width `m` is
//! the identity on a producer sign-normalized to `n <= m`, zero-normalized to
//! `n < m` (bit `m - 1` is already zero), or on a pattern surviving the
//! sign-extension round-trip.
//!
//! Producers whose contracts do not promise the needed bits are refused.
//! `ZeroExtendU32` declares its upper result bits "not meaningful" rather than
//! zero, `LoadPacked` writes an early-clobber scratch with no documented
//! extension guarantee, `Float32ToBits` preserves a payload without naming the
//! upper GPR bits, and ordinary arithmetic, copies, calls, and address forms
//! say nothing about high bits. A second definition of the input — including a
//! `UseDef` rewrite — also refuses: the guarantee must hold wherever the
//! extension reads the register, not only at one definition site.
//!
//! Proposal and independent replay share only the admission predicates and
//! the copy constructor. Validation re-derives the pair from the source,
//! requires the proposed instruction to equal the reconstructed copy, and
//! restores the complete source by content: every other instruction, register,
//! roster row, call, and settlement is retained bit-identical.

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
