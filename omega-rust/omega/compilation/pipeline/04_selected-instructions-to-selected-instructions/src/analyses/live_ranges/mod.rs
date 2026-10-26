//! Optimizer module role: executable entrance. Block-local live ranges and interference compute -> validation entrance.
//!
//! Check that the liveness prerequisite belongs to the selected program
//! before the producer inspects it. `validate` is the retained-plan verifier.
use crate::ValidatedLiveness;
use crate::ValidatedSelectedAnalysis;

pub(crate) mod compute;
pub(crate) mod validate;

use crate::LivenessError;
use optimization_core::OptimizationUnitIdentity;
use selected_instructions::{
    LiveRangeIdentity, LiveRangePlan, LivenessIdentity, SelectedInstructionPlanIdentity,
};
use semantic_vocabulary::FuelScheduleIdentity;

/// Derive block-local live-range fragments and virtual-register interference
/// from an exact selected CFG and validated liveness facts.
pub fn analyze_live_ranges<S: ValidatedSelectedAnalysis>(
    selected: &S,
    liveness: &ValidatedLiveness,
) -> Result<ValidatedLiveRanges, LiveRangeError> {
    validate::require_liveness_of(selected, liveness)?;
    let plan = compute::compute_terminal_live_ranges(selected, liveness)?;
    Ok(validate::admit(plan, &[], &[], &[]))
}

/// Reuse function computations only for identical selected bodies and liveness.
pub fn analyze_live_ranges_reusing(
    previous: &impl ValidatedSelectedAnalysis,
    previous_liveness: &ValidatedLiveness,
    previous_ranges: &ValidatedLiveRanges,
    selected: &impl ValidatedSelectedAnalysis,
    liveness: &ValidatedLiveness,
) -> Result<ValidatedLiveRanges, LiveRangeError> {
    validate::require_liveness_of(selected, liveness)?;
    let (plan, carried) = compute::compute_terminal_live_ranges_reusing(
        previous,
        previous_liveness,
        previous_ranges,
        selected,
        liveness,
    )?;
    Ok(validate::admit(
        plan,
        &carried,
        &previous_ranges.function_digests,
        &previous_ranges.function_counts,
    ))
}

mod staging;
pub use staging::StagedOptimizedLiveRanges;
pub(crate) use staging::{
    OptimizedLiveRangeCustodyError, StagedOptimizedLiveRangeCustodyReceipt,
    stage_optimized_live_ranges,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LiveRangeValidationReceipt {
    pub(crate) identity: LiveRangeIdentity,
    pub(crate) selected: SelectedInstructionPlanIdentity,
    pub(crate) liveness: LivenessIdentity,
    pub(crate) optimization_unit: OptimizationUnitIdentity,
    pub(crate) fuel_schedule: FuelScheduleIdentity,
    pub(crate) virtual_register_count: usize,
    pub(crate) virtual_fragment_count: usize,
}

impl LiveRangeValidationReceipt {
    receipt_accessor_rows!(LiveRangeValidationReceipt {
        identity: LiveRangeIdentity,
        selected: SelectedInstructionPlanIdentity,
        liveness: LivenessIdentity,
        virtual_register_count: usize,
        virtual_fragment_count: usize,
        optimization_unit: OptimizationUnitIdentity,
        fuel_schedule: FuelScheduleIdentity,
    });
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedLiveRanges {
    pub(crate) plan: std::sync::Arc<LiveRangePlan>,
    pub(crate) receipt: LiveRangeValidationReceipt,
    /// Each function's share of the receipt identity, kept so a successor
    /// plan that carries a function over unchanged need not hash it again.
    pub(crate) function_digests: std::sync::Arc<[[u8; 32]]>,
    /// Report-only counts, bound to the same admitted function as its digest.
    pub(crate) function_counts: std::sync::Arc<[validate::FunctionCounts]>,
}

impl ValidatedLiveRanges {
    pub fn plan(&self) -> &LiveRangePlan {
        &self.plan
    }
    receipt_accessor_rows!(ref ValidatedLiveRanges {
        receipt: LiveRangeValidationReceipt,
    });
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LiveRangeError {
    LivenessRevalidation(LivenessError),
    RootMismatch,
    UnsupportedUseDef {
        function: usize,
        instruction: u32,
        operand: u16,
    },
    UnsupportedTiedOperand {
        function: usize,
        instruction: u32,
        operand: u16,
    },
    UnsupportedEarlyClobber {
        function: usize,
        instruction: u32,
        operand: u16,
    },
    PointOverflow {
        function: usize,
    },
    FunctionMismatch {
        function: usize,
    },
    BlockDomainMismatch {
        function: usize,
        block: u32,
    },
    VirtualRegisterMismatch {
        function: usize,
        register: u32,
    },
    ArchitecturalUnitMismatch {
        function: usize,
        unit: u16,
    },
    InterferenceMismatch {
        function: usize,
    },
    TiedPairMismatch {
        function: usize,
    },
    EarlyClobberMismatch {
        function: usize,
    },
    NonCanonicalRows {
        function: usize,
    },
}

failed_display!(LiveRangeError, "Terminal live-range derivation failed");

impl std::error::Error for LiveRangeError {}
