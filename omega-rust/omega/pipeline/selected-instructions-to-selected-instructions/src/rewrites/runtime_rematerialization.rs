//! Runtime-value rematerialization: the cheaper recovery the allocator prefers
//! over private storage when the pressured value is defined by one pure
//! immediate materialization. Each admitted use gets its own fresh
//! `MaterializeI64` and fresh virtual register immediately before the consuming
//! instruction; no reload interval and no stack slot are created. The original
//! definition stays in place — removing it would renumber nothing but its range
//! then collapses to the defining point, so it no longer crosses the pressure.
//!
//! The cost decision is explicit and one-directional: rematerialization is
//! attempted first by the recovery coordinator, and a private-storage spill is
//! the fallback when admission rejects the value. One inserted materialize per
//! use is strictly cheaper than the address/load pair plus definition store a
//! spill would emit for the same value, and it carries no memory dependency.
//! Only `MaterializeI64` results qualify: their defining instruction has a
//! single target-row def operand, no implicit uses, no implicit defs, and no
//! clobbers, so repeating it at a use is a pure regeneration of the same source
//! value. Wider instruction families need a broader cost model and stay
//! rejected.
//!
//! Use custody mirrors runtime spilling exactly: instruction-operand uses must
//! be flexible, untied, and non-early-clobber; terminator operand uses are
//! ordinary uses at the end-of-block position and keep any fixed ABI view,
//! pinning the fresh materialize register to the same physical unit. Outgoing
//! successor transports remain unsupported, and the definition block must
//! dominate every use block.
//!
//! Validation re-derives the same legality contract from the source records
//! on its own audit — the victim's materialize definition and scalar
//! custody, the flexible-use scan, the edge-transport refusal, and the
//! dominance the definition block must hold — and never consults the
//! producer's admission routine. It then requires the proposal to equal the
//! function that audit produces and restores the complete source by
//! content, so a wrong legality decision fails validation even when the
//! proposal matches the emitted edit.

mod admission;
mod rewrite;
mod validation;

use std::sync::Arc;

use optimization_core::OptimizationUnitIdentity;
use selected_instructions::{SelectedInstructionPlan, SelectedInstructionPlanIdentity};
use semantic_vocabulary::FuelScheduleIdentity;

pub use rewrite::rematerialize_selected_runtime_value;
pub use validation::validate_runtime_rematerialization;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedRuntimeRematerialization {
    transformed: Arc<SelectedInstructionPlan>,
    receipt: RuntimeRematerializationReceipt,
}

impl ValidatedRuntimeRematerialization {
    pub fn transformed(&self) -> &SelectedInstructionPlan {
        &self.transformed
    }

    pub fn shared_transformed(&self) -> Arc<SelectedInstructionPlan> {
        Arc::clone(&self.transformed)
    }

    pub const fn receipt(&self) -> &RuntimeRematerializationReceipt {
        &self.receipt
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeRematerializationReceipt {
    source_selected: SelectedInstructionPlanIdentity,
    transformed_selected: SelectedInstructionPlanIdentity,
    optimization_unit: OptimizationUnitIdentity,
    fuel_schedule: FuelScheduleIdentity,
}

impl RuntimeRematerializationReceipt {
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
pub enum RuntimeRematerializationError {
    SourceMismatch,
    UnsupportedValue,
    UnsupportedControlFlow,
    UnsupportedUse,
    ConstraintMismatch,
    WorkBudgetExceeded,
    IdentityOverflow,
    ReplayMismatch,
}

impl std::fmt::Display for RuntimeRematerializationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "invalid runtime-value rematerialization: {self:?}"
        )
    }
}

impl std::error::Error for RuntimeRematerializationError {}

#[cfg(test)]
mod tests;
