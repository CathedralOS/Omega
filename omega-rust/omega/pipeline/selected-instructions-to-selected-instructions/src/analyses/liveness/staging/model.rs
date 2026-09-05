use crate::{LivenessError, LivenessIdentity, ValidatedLiveness};
use optimization_core::{
    OptimizationIdentityBundleIdentity, OptimizationUnitIdentity,
    OptimizedAbstractPlanProjectionIdentity, PrePhysicalOptimizationManifestIdentity,
};
use selected_instructions::SelectedInstructionPlanIdentity;
use semantic_vocabulary::{FuelScheduleIdentity, MachineId};
use terminal_psi::TerminalPsiIdentity;

use target_operations_to_selected_instructions::{
    OptimizedSelectionCustodyError, StagedOptimizedSelectedInstructions,
};

/// Opt-in liveness staging over the complete selected-instruction custody
/// carrier. This grants no interval, allocation, emission, or publication
/// authority.
#[derive(Debug)]
pub struct StagedOptimizedLiveness {
    pub(super) selected: StagedOptimizedSelectedInstructions,
    pub(super) liveness: ValidatedLiveness,
    pub(super) custody: StagedOptimizedLivenessCustodyReceipt,
}

impl StagedOptimizedLiveness {
    pub const fn selected_stage(&self) -> &StagedOptimizedSelectedInstructions {
        &self.selected
    }

    pub const fn liveness(&self) -> &ValidatedLiveness {
        &self.liveness
    }

    pub const fn custody(&self) -> StagedOptimizedLivenessCustodyReceipt {
        self.custody
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StagedOptimizedLivenessCustodyReceipt {
    pub(super) psi: TerminalPsiIdentity,
    pub(super) target: target::NativeTarget,
    pub(super) entry: MachineId,
    pub(super) optimization: OptimizationIdentityBundleIdentity,
    pub(super) projection: OptimizedAbstractPlanProjectionIdentity,
    pub(super) manifest: PrePhysicalOptimizationManifestIdentity,
    pub(super) optimization_unit: OptimizationUnitIdentity,
    pub(super) fuel_schedule: FuelScheduleIdentity,
    pub(super) register_environment: register_model::TargetRegisterEnvironmentIdentity,
    pub(super) selected: SelectedInstructionPlanIdentity,
    pub(super) liveness: LivenessIdentity,
    pub(super) function_count: usize,
    pub(super) structural_unit_function_count: usize,
    pub(super) block_count: usize,
    pub(super) virtual_register_count: usize,
    pub(super) instruction_count: usize,
    pub(super) successor_count: usize,
}

impl StagedOptimizedLivenessCustodyReceipt {
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

    pub const fn liveness(self) -> LivenessIdentity {
        self.liveness
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

    pub const fn instruction_count(self) -> usize {
        self.instruction_count
    }

    pub const fn successor_count(self) -> usize {
        self.successor_count
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizedLivenessCustodyError {
    UpstreamSelection(OptimizedSelectionCustodyError),
    Analysis(LivenessError),
    Revalidation(LivenessError),
    ReceiptMismatch,
}

impl std::fmt::Display for OptimizedLivenessCustodyError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "optimized liveness staging failed: {self:?}")
    }
}

impl std::error::Error for OptimizedLivenessCustodyError {}
