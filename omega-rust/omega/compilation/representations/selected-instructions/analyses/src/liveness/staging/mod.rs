//! Optimizer module role: executable entrance. Selected instructions to validated liveness.
//!
//! [`analyze_liveness`](crate::analyze_liveness) returns only after its
//! independent checker accepts the facts; this entrance records their
//! identity beside the selection receipt it inherits and does not replay
//! either again.

mod compute;
mod custody;

use crate::SelectedOptimizationInput;
use crate::{LivenessError, ValidatedLiveness};
use optimization_core::{
    OptimizationIdentityBundleIdentity, OptimizationSelections, OptimizationUnitIdentity,
    OptimizationWorkBudget, OptimizedAbstractPlanProjectionIdentity,
    PrePhysicalOptimizationManifestIdentity,
};
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::SelectedProgram;
use selected_instructions::StagedOptimizedCustodyCore;
use selected_instructions::{LivenessIdentity, SelectedInstructionPlanIdentity};
use semantic_vocabulary::{FuelScheduleIdentity, MachineId};
use terminal_psi::TerminalPsiIdentity;

pub fn stage_optimized_liveness(
    selected: SelectedOptimizationInput,
) -> Result<StagedOptimizedLiveness, OptimizedLivenessCustodyError> {
    let liveness = compute::compute_liveness(&selected)?;
    let custody = custody::liveness_custody_receipt(
        selected.custody,
        selected.program.identity,
        liveness.receipt(),
    );
    Ok(StagedOptimizedLiveness {
        selected,
        liveness,
        custody,
    })
}

/// Opt-in liveness staging over the selected program instruction selection
/// handed over. This grants no interval, allocation, emission, or publication
/// authority.
#[derive(Debug)]
pub struct StagedOptimizedLiveness {
    selected: SelectedOptimizationInput,
    liveness: ValidatedLiveness,
    custody: StagedOptimizedLivenessCustodyReceipt,
}

impl StagedOptimizedLiveness {
    staged_producer_accessors!(selected; selected, register_environment, selections, budget_per_pass);

    pub const fn liveness(&self) -> &ValidatedLiveness {
        &self.liveness
    }

    receipt_accessor_rows!(ref StagedOptimizedLiveness {
        custody: StagedOptimizedLivenessCustodyReceipt,
    });
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StagedOptimizedLivenessCustodyReceipt {
    core: StagedOptimizedCustodyCore,
    selected: SelectedInstructionPlanIdentity,
    liveness: LivenessIdentity,
}

impl StagedOptimizedLivenessCustodyReceipt {
    selected_instructions::staging_custody_core_accessors!();

    receipt_accessor_rows!(StagedOptimizedLivenessCustodyReceipt {
        selected: SelectedInstructionPlanIdentity,
        liveness: LivenessIdentity,
    });
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizedLivenessCustodyError {
    Analysis(LivenessError),
}

failed_display!(
    OptimizedLivenessCustodyError,
    "optimized liveness staging failed"
);
