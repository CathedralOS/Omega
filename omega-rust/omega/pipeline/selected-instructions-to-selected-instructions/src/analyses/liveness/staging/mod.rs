//! Optimizer module role: executable entrance. Selected instructions to validated liveness.
//!
//! This crate owns the analysis-to-independent-replay join. No liveness
//! result receives stage custody before replay reconstructs its exact receipt.

mod compute;
mod custody;
#[cfg(any(test, feature = "test-support"))]
mod test_support;
mod validation;

#[cfg(any(test, feature = "test-support"))]
pub use test_support::*;
pub use validation::validate_optimized_liveness_custody;
pub(crate) use validation::validate_staged_optimized_liveness_custody;

use crate::{LivenessError, ValidatedLiveness};
use optimization_core::{
    OptimizationIdentityBundleIdentity, OptimizationSelections, OptimizationUnitIdentity,
    OptimizationWorkBudget, OptimizedAbstractPlanProjectionIdentity,
    PrePhysicalOptimizationManifestIdentity,
};
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::{LivenessIdentity, SelectedInstructionPlanIdentity};
use semantic_vocabulary::{FuelScheduleIdentity, MachineId};
use target_operations_to_selected_instructions::StagedOptimizedSelectedInstructions;
use target_operations_to_selected_instructions::{
    OptimizedSelectionCustodyError, ValidatedSelectedInstructions,
};
use terminal_psi::TerminalPsiIdentity;

pub fn stage_optimized_liveness(
    selected: StagedOptimizedSelectedInstructions,
) -> Result<StagedOptimizedLiveness, OptimizedLivenessCustodyError> {
    let liveness = compute::compute_liveness(&selected)?;
    let custody = validate_optimized_liveness_custody(&selected, &liveness)?;
    Ok(StagedOptimizedLiveness {
        selected,
        liveness,
        custody,
    })
}

/// Opt-in liveness staging over the complete selected-instruction custody
/// carrier. This grants no interval, allocation, emission, or publication
/// authority.
#[derive(Debug)]
pub struct StagedOptimizedLiveness {
    selected: StagedOptimizedSelectedInstructions,
    liveness: ValidatedLiveness,
    custody: StagedOptimizedLivenessCustodyReceipt,
}

impl StagedOptimizedLiveness {
    /// The retained producer stage. Replay and custody validation inspect it;
    /// ordinary consumers read the current program through `selected` below.
    pub const fn selected_stage(&self) -> &StagedOptimizedSelectedInstructions {
        &self.selected
    }

    /// The current selected program this analysis describes.
    pub const fn selected(&self) -> &ValidatedSelectedInstructions {
        self.selected.selected()
    }

    /// The target register environment admitted with the current program.
    pub const fn register_environment(&self) -> &ValidatedTargetRegisterEnvironment {
        self.selected.register_environment()
    }

    /// The governing optimizer selections for this admission.
    pub fn selections(&self) -> &OptimizationSelections {
        self.selected.selections()
    }

    /// The per-pass work budget admitted beside the same evidence.
    pub fn budget_per_pass(&self) -> OptimizationWorkBudget {
        self.selected.budget_per_pass()
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
    liveness: LivenessIdentity,
    function_count: usize,
    block_count: usize,
    virtual_register_count: usize,
    instruction_count: usize,
    successor_count: usize,
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
