//! Optimizer module role: executable entrance. Layout-independent selected-form encoding, replay, and optimization custody.
//!
//! Encode the current physical instructions. Replay, kept for
//! retained-artifact readers and tests, independently admits a retained
//! artifact's bytes against the same roots; the stage does not re-encode what
//! it just produced (verification placement in `omega-rust/pipeline.md`).

use register_model::ValidatedPhysicalRegisterModel;
use selected_instructions_to_register_homes::ValidatedSelectedAnalysis;

use register_homes_to_post_allocation_machine::StagedOptimizedPostAllocationMachinePlan;

mod compute;
mod error;

pub use error::OptimizedSelectedFormEncodingError;

use crate::validation;
use machine_code::{
    SelectedFormEncoding, SelectedFormEncodingCounts, SelectedFormEncodingIdentity,
    SelectedFormEncodingRow,
};
use physical_instructions::{
    PostAllocationMachineIdentity, PostAllocationMachineOptimizationCustody,
};

/// Encode the current physical instructions.
pub fn stage_optimized_layout_independent_selected_form_encoding<S: ValidatedSelectedAnalysis>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    frame: Option<&machine_code::TargetFrameLayoutPlan>,
) -> Result<StagedOptimizedSelectedFormEncoding, OptimizedSelectedFormEncodingError> {
    let artifact = compute::compute(selected, machine, physical, frame)?;
    Ok(StagedOptimizedSelectedFormEncoding {
        program: std::sync::Arc::new(artifact),
    })
}

/// Replay the canonical selected-form encoding join against all retained
/// selected, physical-machine, and exact frame roots.
pub fn validate_optimized_layout_independent_selected_form_encoding<
    S: ValidatedSelectedAnalysis,
>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    frame: Option<&machine_code::TargetFrameLayoutPlan>,
    artifact: &StagedOptimizedSelectedFormEncoding,
) -> Result<(), OptimizedSelectedFormEncodingError> {
    validation::validate(selected, machine, physical, frame, artifact.program())
}

/// Encoding data produced by this stage. Raw program data cannot construct this token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedOptimizedSelectedFormEncoding {
    program: std::sync::Arc<SelectedFormEncoding>,
}

impl StagedOptimizedSelectedFormEncoding {
    pub fn program(&self) -> &SelectedFormEncoding {
        &self.program
    }

    pub fn shared_program(&self) -> std::sync::Arc<SelectedFormEncoding> {
        std::sync::Arc::clone(&self.program)
    }

    pub fn selected(&self) -> selected_instructions::SelectedInstructionPlanIdentity {
        self.program.selected
    }

    pub fn machine(&self) -> PostAllocationMachineIdentity {
        self.program.machine
    }

    pub fn post_allocation_machine_optimization(
        &self,
    ) -> Option<PostAllocationMachineOptimizationCustody> {
        self.program.post_allocation_machine_optimization
    }

    pub fn identity(&self) -> SelectedFormEncodingIdentity {
        self.program.identity
    }

    pub fn rows(&self) -> &[SelectedFormEncodingRow] {
        &self.program.rows
    }

    /// The byte image and footprint table the rows above name.
    pub fn encoded(&self) -> &machine_code::SelectedFormEncodedRowStore {
        self.program.encoded()
    }

    pub fn counts(&self) -> SelectedFormEncodingCounts {
        self.program.counts
    }
}
