//! Optimizer module role: executable entrance. Layout-independent selected-form encoding, replay, and optimization custody.
//!
//! Encode the current physical instructions, then independently admit the
//! bytes; replay validates a retained artifact against the same roots.

use register_model::ValidatedPhysicalRegisterModel;
use selected_instructions_to_register_homes::ValidatedSelectedAnalysis;

use register_homes_to_post_allocation_machine::StagedOptimizedPostAllocationMachinePlan;

mod compute;
mod error;

pub use error::*;

use crate::validation;
use machine_code::{
    SelectedFormEncoding, SelectedFormEncodingCounts, SelectedFormEncodingIdentity,
    SelectedFormEncodingRow, SelectedFormMachineOptimizationCustody,
    SelectedFormMovnOptimizationCustody,
};
use physical_instructions::{
    PostAllocationMachineIdentity, PostAllocationMachineOptimizationCustody,
};

/// Encode the current physical instructions and independently admit their bytes.
pub fn stage_optimized_layout_independent_selected_form_encoding<S: ValidatedSelectedAnalysis>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    frame: Option<&machine_code::TargetFrameLayoutPlan>,
) -> Result<StagedOptimizedSelectedFormEncoding, OptimizedSelectedFormEncodingError> {
    let artifact = compute::compute(selected, machine, physical, frame)?;
    validation::validate(selected, machine, physical, frame, &artifact)?;
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

/// Independently admitted encoding data. Raw program data cannot construct this token.
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

    pub fn machine_optimization(&self) -> Option<SelectedFormMachineOptimizationCustody> {
        self.program.machine_optimization()
    }

    pub fn movn_optimization(&self) -> Option<SelectedFormMovnOptimizationCustody> {
        self.program.movn_optimization()
    }

    pub fn identity(&self) -> SelectedFormEncodingIdentity {
        self.program.identity
    }

    pub fn rows(&self) -> &[SelectedFormEncodingRow] {
        &self.program.rows
    }

    pub fn counts(&self) -> SelectedFormEncodingCounts {
        self.program.counts
    }

    #[cfg(feature = "test-support")]
    #[doc(hidden)]
    pub fn program_mut_for_test(&mut self) -> &mut SelectedFormEncoding {
        std::sync::Arc::make_mut(&mut self.program)
    }

    #[cfg(feature = "test-support")]
    #[doc(hidden)]
    pub fn rows_mut(&mut self) -> &mut [SelectedFormEncodingRow] {
        std::sync::Arc::make_mut(&mut self.program)
            .rows
            .as_mut_slice()
    }

    #[cfg(feature = "test-support")]
    #[doc(hidden)]
    #[allow(dead_code)]
    pub fn counts_mut(&mut self) -> &mut SelectedFormEncodingCounts {
        &mut std::sync::Arc::make_mut(&mut self.program).counts
    }
}
