//! Optimizer module role: executable entrance.
mod compute;
mod error;
#[cfg(test)]
mod order_suite;
mod ordinary;
mod validation;

pub use error::OptimizedResolvedSelectedFormLayoutError;
use register_model::ValidatedPhysicalRegisterModel;
use selected_instructions_to_register_homes::ValidatedSelectedAnalysis;

use machine_code::ResolvedMachineLayout;
pub use machine_code::{
    ResolvedBranchEvidence, ResolvedConditionalBranchEvidence, ResolvedConditionalBranchPredicate,
    ResolvedJumpEvidence, ResolvedSelectedBlockLayout, ResolvedSelectedFormLayoutIdentity,
    ResolvedSelectedFormRow, ResolvedSelectedFunctionLayout, SelectedFunctionLayoutPolicy,
};
use machine_code::{
    SelectedFormEncodingIdentity, SelectedFormMachineOptimizationCustody,
    SelectedFormMovnOptimizationCustody,
};
use physical_instructions::PostAllocationMachineOptimizationCustody;
use post_allocation_machine_to_selected_form_encoding::StagedOptimizedSelectedFormEncoding;
use register_homes_to_post_allocation_machine::StagedOptimizedPostAllocationMachinePlan;
use std::sync::Arc;
use target::NativeTarget;

/// Resolve the current encoded program and independently admit its layout.
pub fn stage_optimized_resolved_selected_form_layout<S: ValidatedSelectedAnalysis>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    pre_layout: &StagedOptimizedSelectedFormEncoding,
) -> Result<StagedOptimizedResolvedSelectedFormLayout, OptimizedResolvedSelectedFormLayoutError> {
    let artifact = compute::compute(selected, machine, physical, pre_layout)?;
    admit_resolved_machine_layout(selected, machine, physical, pre_layout, artifact.program)
}

/// Replay the exact encoding roots, byte accounting, and all resolved rows.
pub fn validate_optimized_resolved_selected_form_layout<S: ValidatedSelectedAnalysis>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    pre_layout: &StagedOptimizedSelectedFormEncoding,
    artifact: &StagedOptimizedResolvedSelectedFormLayout,
) -> Result<(), OptimizedResolvedSelectedFormLayoutError> {
    validation::validate(selected, machine, physical, pre_layout, artifact)
}

/// Independently admit retained current data, without a producer-stage history.
/// Content identity alone is insufficient: replay checks the selected program,
/// machine, target decoding, offsets, fixups, and exact optimization records.
pub fn admit_resolved_machine_layout<S: ValidatedSelectedAnalysis>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    pre_layout: &StagedOptimizedSelectedFormEncoding,
    program: std::sync::Arc<machine_code::ResolvedMachineLayout>,
) -> Result<StagedOptimizedResolvedSelectedFormLayout, OptimizedResolvedSelectedFormLayoutError> {
    let artifact = StagedOptimizedResolvedSelectedFormLayout { program };
    validation::validate(selected, machine, physical, pre_layout, &artifact)?;
    Ok(artifact)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedOptimizedResolvedSelectedFormLayout {
    program: Arc<ResolvedMachineLayout>,
}

impl StagedOptimizedResolvedSelectedFormLayout {
    pub fn selected(&self) -> selected_instructions::SelectedInstructionPlanIdentity {
        self.program.selected
    }

    pub fn machine(&self) -> physical_instructions::PostAllocationMachineIdentity {
        self.program.machine
    }

    pub fn pre_layout(&self) -> SelectedFormEncodingIdentity {
        self.program.pre_layout
    }

    pub fn machine_optimization(&self) -> Option<SelectedFormMachineOptimizationCustody> {
        self.program.machine_optimization()
    }

    pub fn movn_optimization(&self) -> Option<SelectedFormMovnOptimizationCustody> {
        self.program.movn_optimization()
    }

    pub fn post_allocation_machine_optimization(
        &self,
    ) -> Option<PostAllocationMachineOptimizationCustody> {
        self.program.post_allocation_machine_optimization
    }

    pub fn target(&self) -> NativeTarget {
        self.program.target
    }

    pub fn policy(&self) -> SelectedFunctionLayoutPolicy {
        self.program.policy
    }

    pub fn identity(&self) -> ResolvedSelectedFormLayoutIdentity {
        self.program.identity
    }

    pub fn functions(&self) -> &[ResolvedSelectedFunctionLayout] {
        &self.program.functions
    }

    pub fn program(&self) -> &ResolvedMachineLayout {
        &self.program
    }

    pub fn shared_program(&self) -> Arc<ResolvedMachineLayout> {
        Arc::clone(&self.program)
    }

    /// Reconstruct a candidate only within the owning layout stage. Public
    /// admission independently checks the raw program before returning it.
    fn from_program(program: ResolvedMachineLayout) -> Self {
        Self {
            program: Arc::new(program),
        }
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn functions_mut(&mut self) -> &mut [ResolvedSelectedFunctionLayout] {
        &mut Arc::make_mut(&mut self.program).functions
    }
}
