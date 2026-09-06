//! Independently admitted layout and its shared current representation.
//!
//! The program is retained directly, without a source-stage wrapper. Sharing
//! its immutable data grants no way to construct another admitted layout.

use std::sync::Arc;

use machine_code::ResolvedMachineLayout;
use physical_instructions::PostAllocationMachineOptimizationCustody;
use target::NativeTarget;

pub use machine_code::{
    ResolvedBranchEvidence, ResolvedConditionalBranchEvidence, ResolvedConditionalBranchPredicate,
    ResolvedJumpEvidence, ResolvedSelectedBlockLayout, ResolvedSelectedFormLayoutIdentity,
    ResolvedSelectedFormRow, ResolvedSelectedFunctionLayout, ResolvedStructuralUnitCallLayout,
    ResolvedStructuralUnitFunctionLayout, SelectedFunctionLayoutPolicy,
};

use machine_code::{
    SelectedFormEncodingIdentity, SelectedFormMachineOptimizationCustody,
    SelectedFormMovnOptimizationCustody,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedOptimizedResolvedSelectedFormLayout {
    pub(super) program: Arc<ResolvedMachineLayout>,
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

    pub fn structural_unit_functions(&self) -> &[ResolvedStructuralUnitFunctionLayout] {
        &self.program.structural_unit_functions
    }

    pub fn program(&self) -> &ResolvedMachineLayout {
        &self.program
    }

    pub fn shared_program(&self) -> Arc<ResolvedMachineLayout> {
        Arc::clone(&self.program)
    }

    /// Reconstruct a candidate only within the owning layout stage. Public
    /// admission independently checks the raw program before returning it.
    pub(super) fn from_program(program: ResolvedMachineLayout) -> Self {
        Self {
            program: Arc::new(program),
        }
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn functions_mut(&mut self) -> &mut [ResolvedSelectedFunctionLayout] {
        &mut Arc::make_mut(&mut self.program).functions
    }

    #[cfg(any(test, feature = "test-support"))]
    #[allow(dead_code)]
    pub fn structural_unit_functions_mut(&mut self) -> &mut [ResolvedStructuralUnitFunctionLayout] {
        &mut Arc::make_mut(&mut self.program).structural_unit_functions
    }
}
