use crate::{LiveRangeError, LiveRangeIdentity, ValidatedLiveRanges};
use omega_optimization_core::{
    OptimizationIdentityBundleIdentity, OptimizationUnitIdentity,
    OptimizedAbstractPlanProjectionIdentity, PrePhysicalOptimizationManifestIdentity,
};
use omega_selected_instructions::SelectedInstructionPlanIdentity;
use psi_core::{FuelScheduleIdentity, MachineId};
use psi_terminal::TerminalPsiIdentity;

use crate::{OptimizedLivenessCustodyError, StagedOptimizedLiveness};

/// Opt-in CFG-aware live-range staging over complete liveness custody. This
/// grants no splitting, allocation, spill, frame, emission, or publication
/// authority.
#[derive(Debug)]
pub struct StagedOptimizedLiveRanges {
    pub(super) liveness: StagedOptimizedLiveness,
    pub(super) ranges: ValidatedLiveRanges,
    pub(super) custody: StagedOptimizedLiveRangeCustodyReceipt,
}

impl StagedOptimizedLiveRanges {
    pub const fn liveness_stage(&self) -> &StagedOptimizedLiveness {
        &self.liveness
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
    pub(super) psi: TerminalPsiIdentity,
    pub(super) target: omega_target::NativeTarget,
    pub(super) entry: MachineId,
    pub(super) optimization: OptimizationIdentityBundleIdentity,
    pub(super) projection: OptimizedAbstractPlanProjectionIdentity,
    pub(super) manifest: PrePhysicalOptimizationManifestIdentity,
    pub(super) optimization_unit: OptimizationUnitIdentity,
    pub(super) fuel_schedule: FuelScheduleIdentity,
    pub(super) register_environment: omega_register_model::TargetRegisterEnvironmentIdentity,
    pub(super) selected: SelectedInstructionPlanIdentity,
    pub(super) liveness: crate::LivenessIdentity,
    pub(super) ranges: LiveRangeIdentity,
    pub(super) function_count: usize,
    pub(super) structural_unit_function_count: usize,
    pub(super) block_count: usize,
    pub(super) virtual_register_count: usize,
    pub(super) virtual_occurrence_count: usize,
    pub(super) fixed_constraint_count: usize,
    pub(super) virtual_fragment_count: usize,
    pub(super) architectural_unit_count: usize,
    pub(super) architectural_action_count: usize,
    pub(super) architectural_fragment_count: usize,
    pub(super) virtual_edge_connector_count: usize,
    pub(super) architectural_edge_connector_count: usize,
    pub(super) interference_count: usize,
}

impl StagedOptimizedLiveRangeCustodyReceipt {
    pub const fn psi(self) -> TerminalPsiIdentity {
        self.psi
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

    pub const fn selected(self) -> SelectedInstructionPlanIdentity {
        self.selected
    }

    pub const fn register_environment(
        self,
    ) -> omega_register_model::TargetRegisterEnvironmentIdentity {
        self.register_environment
    }

    pub const fn liveness(self) -> crate::LivenessIdentity {
        self.liveness
    }

    pub const fn ranges(self) -> LiveRangeIdentity {
        self.ranges
    }

    pub const fn function_count(self) -> usize {
        self.function_count
    }

    pub const fn structural_unit_function_count(self) -> usize {
        self.structural_unit_function_count
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
