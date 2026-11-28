//! Completed selected instructions and analyses passed to register allocation.
//! The product contains records only; allocation decides which further analysis it needs.
#![forbid(unsafe_code)]
use optimization_core::{OptimizationSelections, OptimizationWorkBudget};
use register_environment::ValidatedTargetRegisterEnvironment;
use register_homes::{AllocationLegalityPlan, AllocatorAvailabilityPlan};
use selected_instructions::{
    LiveRangePlan, LivenessPlan, SelectedProgram, StagedOptimizedCustodyCore,
};
use std::sync::Arc;
mod analysis_records;
mod history;
pub use analysis_records::*;
pub use history::SelectedOptimizationRecord;

#[derive(Debug)]
pub struct LivenessFacts {
    pub plan: Arc<LivenessPlan>,
    pub receipt: LivenessValidationReceipt,
}
#[derive(Debug)]
pub struct LiveRangeFacts {
    pub plan: Arc<LiveRangePlan>,
    pub receipt: LiveRangeValidationReceipt,
    pub function_counts: Arc<[LiveRangeFunctionCounts]>,
}
#[derive(Debug)]
pub struct AllocationLegalityFacts {
    pub plan: Arc<AllocationLegalityPlan>,
    pub receipt: AllocationLegalityValidationReceipt,
    pub function_counts: Arc<[LegalityFunctionCounts]>,
    pub availability: AllocatorAvailabilityPlan,
    pub availability_receipt: AllocatorAvailabilityValidationReceipt,
}

/// Unissued records assembled by the producer after its analyses complete.
#[derive(Debug)]
pub struct SelectedBodyParts {
    pub body: Arc<physical_program::NativeBodyFacts>,
    pub selected: SelectedProgram,
    pub custody: StagedOptimizedCustodyCore,
    pub environment: ValidatedTargetRegisterEnvironment,
    pub selections: OptimizationSelections,
    pub budget: OptimizationWorkBudget,
    pub history: SelectedOptimizationRecord,
    pub liveness: LivenessFacts,
    pub ranges: LiveRangeFacts,
    pub legality: Option<AllocationLegalityFacts>,
}

/// Only selected optimization may issue the completed input to allocation.
/// ```compile_fail,E0277
/// fn bypass(parts: selected_body::SelectedBodyParts) -> selected_body::SelectedBody {
///     parts.into()
/// }
/// ```
#[derive(Debug)]
pub struct SelectedBody {
    parts: SelectedBodyParts,
}
impl SelectedBody {
    pub fn new(
        _issuer: selected_body_issuer::SelectedBodyIssuer,
        parts: SelectedBodyParts,
    ) -> Self {
        Self { parts }
    }
    pub const fn parts(&self) -> &SelectedBodyParts {
        &self.parts
    }
    pub fn into_parts(self) -> SelectedBodyParts {
        self.parts
    }
}
