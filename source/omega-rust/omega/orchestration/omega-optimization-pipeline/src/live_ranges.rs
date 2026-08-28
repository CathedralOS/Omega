use omega_optimization_core::{
    OptimizationIdentityBundleIdentity, OptimizationUnitIdentity,
    OptimizedAbstractPlanProjectionIdentity, PrePhysicalOptimizationManifestIdentity,
};
use omega_regalloc::{
    TerminalLiveRangeError, TerminalLiveRangeIdentity, ValidatedTerminalLiveRanges,
    analyze_terminal_live_ranges, validate_terminal_live_ranges,
};
use omega_terminal_selected_instructions::TerminalSelectedInstructionPlanIdentity;
use psi_core::{FuelScheduleIdentity, MachineId};
use psi_terminal::TerminalPsiIdentity;

use crate::{
    OptimizedLivenessCustodyError, StagedOptimizedLiveness, validate_optimized_liveness_custody,
};

/// Opt-in CFG-aware live-range staging over complete liveness custody. This
/// grants no splitting, allocation, spill, frame, emission, or publication
/// authority.
#[derive(Debug)]
pub struct StagedOptimizedLiveRanges {
    liveness: StagedOptimizedLiveness,
    ranges: ValidatedTerminalLiveRanges,
    custody: StagedOptimizedLiveRangeCustodyReceipt,
}

impl StagedOptimizedLiveRanges {
    pub const fn liveness_stage(&self) -> &StagedOptimizedLiveness {
        &self.liveness
    }

    pub const fn ranges(&self) -> &ValidatedTerminalLiveRanges {
        &self.ranges
    }

    pub const fn custody(&self) -> StagedOptimizedLiveRangeCustodyReceipt {
        self.custody
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StagedOptimizedLiveRangeCustodyReceipt {
    terminal_psi: TerminalPsiIdentity,
    target: omega_target::NativeTarget,
    entry: MachineId,
    optimization: OptimizationIdentityBundleIdentity,
    projection: OptimizedAbstractPlanProjectionIdentity,
    manifest: PrePhysicalOptimizationManifestIdentity,
    optimization_unit: OptimizationUnitIdentity,
    fuel_schedule: FuelScheduleIdentity,
    register_environment: omega_register_model::TargetRegisterEnvironmentIdentity,
    selected: TerminalSelectedInstructionPlanIdentity,
    liveness: omega_regalloc::TerminalLivenessIdentity,
    ranges: TerminalLiveRangeIdentity,
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
    pub const fn terminal_psi(self) -> TerminalPsiIdentity {
        self.terminal_psi
    }

    pub const fn target(self) -> omega_target::NativeTarget {
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

    pub const fn selected(self) -> TerminalSelectedInstructionPlanIdentity {
        self.selected
    }

    pub const fn register_environment(
        self,
    ) -> omega_register_model::TargetRegisterEnvironmentIdentity {
        self.register_environment
    }

    pub const fn liveness(self) -> omega_regalloc::TerminalLivenessIdentity {
        self.liveness
    }

    pub const fn ranges(self) -> TerminalLiveRangeIdentity {
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
    Analysis(TerminalLiveRangeError),
    Revalidation(TerminalLiveRangeError),
    ReceiptMismatch,
}

impl std::fmt::Display for OptimizedLiveRangeCustodyError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "optimized live-range staging failed: {self:?}")
    }
}

impl std::error::Error for OptimizedLiveRangeCustodyError {}

pub fn stage_optimized_live_ranges(
    liveness: StagedOptimizedLiveness,
) -> Result<StagedOptimizedLiveRanges, OptimizedLiveRangeCustodyError> {
    let upstream =
        validate_optimized_liveness_custody(liveness.selected_stage(), liveness.liveness())
            .map_err(OptimizedLiveRangeCustodyError::UpstreamLiveness)?;
    let ranges =
        analyze_terminal_live_ranges(liveness.selected_stage().selected(), liveness.liveness())
            .map_err(OptimizedLiveRangeCustodyError::Analysis)?;
    let replayed = validate_terminal_live_ranges(
        liveness.selected_stage().selected(),
        liveness.liveness(),
        ranges.plan().clone(),
    )
    .map_err(OptimizedLiveRangeCustodyError::Revalidation)?;
    if replayed.receipt() != ranges.receipt() {
        return Err(OptimizedLiveRangeCustodyError::ReceiptMismatch);
    }
    let custody = custody_receipt(upstream, ranges.receipt());
    Ok(StagedOptimizedLiveRanges {
        liveness,
        ranges,
        custody,
    })
}

pub fn validate_optimized_live_range_custody(
    liveness: &StagedOptimizedLiveness,
    ranges: &ValidatedTerminalLiveRanges,
) -> Result<StagedOptimizedLiveRangeCustodyReceipt, OptimizedLiveRangeCustodyError> {
    let upstream =
        validate_optimized_liveness_custody(liveness.selected_stage(), liveness.liveness())
            .map_err(OptimizedLiveRangeCustodyError::UpstreamLiveness)?;
    let replayed = validate_terminal_live_ranges(
        liveness.selected_stage().selected(),
        liveness.liveness(),
        ranges.plan().clone(),
    )
    .map_err(OptimizedLiveRangeCustodyError::Revalidation)?;
    if replayed.receipt() != ranges.receipt() {
        return Err(OptimizedLiveRangeCustodyError::ReceiptMismatch);
    }
    Ok(custody_receipt(upstream, replayed.receipt()))
}

fn custody_receipt(
    upstream: crate::StagedOptimizedLivenessCustodyReceipt,
    ranges: omega_regalloc::TerminalLiveRangeValidationReceipt,
) -> StagedOptimizedLiveRangeCustodyReceipt {
    StagedOptimizedLiveRangeCustodyReceipt {
        terminal_psi: upstream.terminal_psi(),
        target: upstream.target(),
        entry: upstream.entry(),
        optimization: upstream.optimization(),
        projection: upstream.projection(),
        manifest: upstream.manifest(),
        optimization_unit: upstream.optimization_unit(),
        fuel_schedule: upstream.fuel_schedule(),
        register_environment: upstream.register_environment(),
        selected: upstream.selected(),
        liveness: upstream.liveness(),
        ranges: ranges.identity(),
        function_count: ranges.function_count(),
        block_count: ranges.block_count(),
        virtual_register_count: ranges.virtual_register_count(),
        virtual_occurrence_count: ranges.virtual_occurrence_count(),
        fixed_constraint_count: ranges.fixed_constraint_count(),
        virtual_fragment_count: ranges.virtual_fragment_count(),
        architectural_unit_count: ranges.architectural_unit_count(),
        architectural_action_count: ranges.architectural_action_count(),
        architectural_fragment_count: ranges.architectural_fragment_count(),
        virtual_edge_connector_count: ranges.virtual_edge_connector_count(),
        architectural_edge_connector_count: ranges.architectural_edge_connector_count(),
        interference_count: ranges.interference_count(),
    }
}
