//! Optimizer module role: executable entrance. Validated liveness to validated live ranges.
//!
//! This crate owns the analysis-to-independent-replay join over complete
//! liveness custody. No interval or interference fact escapes before replay.

mod compute;
mod custody;
#[cfg(any(test, feature = "test-support"))]
mod test_support;
mod validation;

#[cfg(any(test, feature = "test-support"))]
pub use test_support::*;
pub use validation::validate_optimized_live_range_custody;

use crate::StagedOptimizedLiveness;
use crate::{
    LiveRangeError, OptimizedLivenessCustodyError, ValidatedLiveRanges, ValidatedLiveness,
};
use optimization_core::{
    OptimizationIdentityBundleIdentity, OptimizationSelections, OptimizationUnitIdentity,
    OptimizationWorkBudget, OptimizedAbstractPlanProjectionIdentity,
    PrePhysicalOptimizationManifestIdentity,
};
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::{LiveRangeIdentity, SelectedInstructionPlanIdentity};
use semantic_vocabulary::{FuelScheduleIdentity, MachineId};
use target_operations_to_selected_instructions::ValidatedSelectedInstructions;
use terminal_psi::TerminalPsiIdentity;

pub fn stage_optimized_live_ranges(
    liveness: StagedOptimizedLiveness,
) -> Result<StagedOptimizedLiveRanges, OptimizedLiveRangeCustodyError> {
    let ranges = compute::compute_live_ranges(&liveness)?;
    let custody = validate_optimized_live_range_custody(&liveness, &ranges)?;
    Ok(StagedOptimizedLiveRanges {
        liveness,
        ranges,
        custody,
    })
}

/// Opt-in CFG-aware live-range staging over complete liveness custody. This
/// grants no splitting, allocation, spill, frame, emission, or publication
/// authority.
#[derive(Debug)]
pub struct StagedOptimizedLiveRanges {
    liveness: StagedOptimizedLiveness,
    ranges: ValidatedLiveRanges,
    custody: StagedOptimizedLiveRangeCustodyReceipt,
}

impl StagedOptimizedLiveRanges {
    /// The retained producer stage. Replay and custody validation inspect it;
    /// ordinary consumers read the current program and analyses directly.
    pub const fn liveness_stage(&self) -> &StagedOptimizedLiveness {
        &self.liveness
    }

    /// The liveness facts over the current program.
    pub const fn liveness(&self) -> &ValidatedLiveness {
        self.liveness.liveness()
    }

    /// The current selected program these ranges describe.
    pub const fn selected(&self) -> &ValidatedSelectedInstructions {
        self.liveness.selected()
    }

    /// The target register environment admitted with the current program.
    pub const fn register_environment(&self) -> &ValidatedTargetRegisterEnvironment {
        self.liveness.register_environment()
    }

    /// The governing optimizer selections for this admission.
    pub fn selections(&self) -> &OptimizationSelections {
        self.liveness.selections()
    }

    /// The per-pass work budget admitted beside the same evidence.
    pub fn budget_per_pass(&self) -> OptimizationWorkBudget {
        self.liveness.budget_per_pass()
    }

    pub const fn ranges(&self) -> &ValidatedLiveRanges {
        &self.ranges
    }

    pub const fn custody(&self) -> StagedOptimizedLiveRangeCustodyReceipt {
        self.custody
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StagedOptimizedLiveRangeCustodyReceipt {
    psi: TerminalPsiIdentity,
    target: target::NativeTarget,
    entry: MachineId,
    optimization: OptimizationIdentityBundleIdentity,
    projection: OptimizedAbstractPlanProjectionIdentity,
    manifest: PrePhysicalOptimizationManifestIdentity,
    optimization_unit: OptimizationUnitIdentity,
    fuel_schedule: FuelScheduleIdentity,
    register_environment: register_model::TargetRegisterEnvironmentIdentity,
    selected: SelectedInstructionPlanIdentity,
    liveness: selected_instructions::LivenessIdentity,
    ranges: LiveRangeIdentity,
    function_count: usize,
    block_count: usize,
    virtual_register_count: usize,
    virtual_occurrence_count: usize,
    fixed_constraint_count: usize,
    virtual_fragment_count: usize,
    architectural_unit_count: usize,
    architectural_action_count: usize,
    architectural_fragment_count: usize,
    virtual_edge_connector_count: usize,
    architectural_edge_connector_count: usize,
    interference_count: usize,
}

impl StagedOptimizedLiveRangeCustodyReceipt {
    pub const fn psi(self) -> TerminalPsiIdentity {
        self.psi
    }

    pub const fn target(self) -> target::NativeTarget {
        self.target
    }

    pub const fn entry(self) -> MachineId {
        self.entry
    }

    pub const fn optimization(self) -> OptimizationIdentityBundleIdentity {
        self.optimization
    }

    pub const fn projection(self) -> OptimizedAbstractPlanProjectionIdentity {
        self.projection
    }

    pub const fn manifest(self) -> PrePhysicalOptimizationManifestIdentity {
        self.manifest
    }

    pub const fn optimization_unit(self) -> OptimizationUnitIdentity {
        self.optimization_unit
    }

    pub const fn fuel_schedule(self) -> FuelScheduleIdentity {
        self.fuel_schedule
    }

    pub const fn selected(self) -> SelectedInstructionPlanIdentity {
        self.selected
    }

    pub const fn register_environment(self) -> register_model::TargetRegisterEnvironmentIdentity {
        self.register_environment
    }

    pub const fn liveness(self) -> selected_instructions::LivenessIdentity {
        self.liveness
    }

    pub const fn ranges(self) -> LiveRangeIdentity {
        self.ranges
    }

    pub const fn function_count(self) -> usize {
        self.function_count
    }

    pub const fn block_count(self) -> usize {
        self.block_count
    }

    pub const fn virtual_register_count(self) -> usize {
        self.virtual_register_count
    }

    pub const fn virtual_fragment_count(self) -> usize {
        self.virtual_fragment_count
    }

    pub const fn virtual_occurrence_count(self) -> usize {
        self.virtual_occurrence_count
    }

    pub const fn fixed_constraint_count(self) -> usize {
        self.fixed_constraint_count
    }

    pub const fn architectural_unit_count(self) -> usize {
        self.architectural_unit_count
    }

    pub const fn architectural_fragment_count(self) -> usize {
        self.architectural_fragment_count
    }

    pub const fn architectural_action_count(self) -> usize {
        self.architectural_action_count
    }

    pub const fn virtual_edge_connector_count(self) -> usize {
        self.virtual_edge_connector_count
    }

    pub const fn architectural_edge_connector_count(self) -> usize {
        self.architectural_edge_connector_count
    }

    pub const fn interference_count(self) -> usize {
        self.interference_count
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizedLiveRangeCustodyError {
    UpstreamLiveness(OptimizedLivenessCustodyError),
    Analysis(LiveRangeError),
    Revalidation(LiveRangeError),
    ReceiptMismatch,
}

impl std::fmt::Display for OptimizedLiveRangeCustodyError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "optimized live-range staging failed: {self:?}")
    }
}

impl std::error::Error for OptimizedLiveRangeCustodyError {}
