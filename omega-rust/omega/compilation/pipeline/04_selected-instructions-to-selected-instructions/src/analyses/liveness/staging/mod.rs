//! Optimizer module role: executable entrance. Selected instructions to validated liveness.
//!
//! [`analyze_liveness`](crate::analyze_liveness) returns only after its
//! independent checker accepts the facts; this entrance records their
//! identity beside the selection receipt it inherits and does not replay
//! either again.

mod compute;
mod custody;

use crate::{LivenessError, ValidatedLiveness};
use optimization_core::{
    OptimizationIdentityBundleIdentity, OptimizationSelections, OptimizationUnitIdentity,
    OptimizationWorkBudget, OptimizedAbstractPlanProjectionIdentity,
    PrePhysicalOptimizationManifestIdentity,
};
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::{LivenessIdentity, SelectedInstructionPlanIdentity};
use semantic_vocabulary::{FuelScheduleIdentity, MachineId};
use target_operations_to_selected_instructions::{
    StagedOptimizedCustodyCore, StagedOptimizedSelectedInstructions, ValidatedSelectedInstructions,
};
use terminal_psi::TerminalPsiIdentity;

pub(crate) fn stage_optimized_liveness(
    selected: StagedOptimizedSelectedInstructions,
) -> Result<StagedOptimizedLiveness, OptimizedLivenessCustodyError> {
    let liveness = compute::compute_liveness(&selected)?;
    let custody = custody::liveness_custody_receipt(selected.custody(), liveness.receipt());
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
    /// The producer stage, which owns the optimized-target product later
    /// stages share; ordinary consumers read the current program through
    /// `selected` below.
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
    core: StagedOptimizedCustodyCore,
    selected: SelectedInstructionPlanIdentity,
    liveness: LivenessIdentity,
}

impl StagedOptimizedLivenessCustodyReceipt {
    target_operations_to_selected_instructions::staging_custody_core_accessors!();

    pub const fn selected(self) -> SelectedInstructionPlanIdentity {
        self.selected
    }

    pub const fn liveness(self) -> LivenessIdentity {
        self.liveness
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizedLivenessCustodyError {
    Analysis(LivenessError),
}

failed_display!(
    OptimizedLivenessCustodyError,
    "optimized liveness staging failed"
);

impl std::error::Error for OptimizedLivenessCustodyError {}
