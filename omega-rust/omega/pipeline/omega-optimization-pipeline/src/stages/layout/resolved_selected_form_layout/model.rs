//! Independently admitted layout and its shared current representation.
//!
//! The program is retained directly, without a source-stage wrapper. Sharing
//! its immutable data grants no way to construct another admitted layout.

use std::sync::Arc;

use omega_machine_code::ResolvedMachineLayout;
use omega_machine_optimizer::{Aarch64CbnzFusionIdentity, Aarch64MovnMaterializationIdentity};
use omega_optimization_core::Optimization;
use omega_physical_instructions::PostAllocationMachineOptimizationCustody;
use omega_target::NativeTarget;

pub use omega_machine_code::{
    ResolvedConditionalBranchEvidence, ResolvedConditionalBranchPredicate,
    ResolvedSelectedBlockLayout, ResolvedSelectedFormLayoutIdentity, ResolvedSelectedFormRow,
    ResolvedSelectedFunctionLayout, ResolvedStructuralUnitCallLayout,
    ResolvedStructuralUnitFunctionLayout, SelectedFunctionLayoutPolicy,
};

use crate::{
    SelectedFormEncodingIdentity, SelectedFormMachineOptimizationCustody,
    SelectedFormMovnOptimizationCustody,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedOptimizedResolvedSelectedFormLayout {
    pub(super) program: Arc<ResolvedMachineLayout>,
}

impl StagedOptimizedResolvedSelectedFormLayout {
    pub fn selected(&self) -> omega_selected_instructions::SelectedInstructionPlanIdentity {
        self.program.selected
    }

    pub fn machine(&self) -> omega_machine_optimizer::PostAllocationMachineIdentity {
        self.program.machine
    }

    pub fn pre_layout(&self) -> SelectedFormEncodingIdentity {
        self.program.pre_layout
    }

    pub fn machine_optimization(&self) -> Option<SelectedFormMachineOptimizationCustody> {
        match self.program.post_allocation_machine_optimization {
            Some(custody)
                if matches!(
                    custody.optimization(),
                    Optimization::Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1
                ) =>
            {
                Some(SelectedFormMachineOptimizationCustody::from_parts(
                    custody.selections(),
                    custody.post_allocation_machine_selections(),
                    Aarch64CbnzFusionIdentity::from_bytes(custody.artifact_identity()),
                ))
            }
            _ => None,
        }
    }

    pub fn movn_optimization(&self) -> Option<SelectedFormMovnOptimizationCustody> {
        match self.program.post_allocation_machine_optimization {
            Some(custody)
                if matches!(
                    custody.optimization(),
                    Optimization::Aarch64SelectShortestMovnSeededI64MaterializationV1
                ) =>
            {
                Some(SelectedFormMovnOptimizationCustody::from_parts(
                    custody.selections(),
                    custody.post_allocation_machine_selections(),
                    Aarch64MovnMaterializationIdentity::from_bytes(custody.artifact_identity()),
                ))
            }
            _ => None,
        }
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

    /// Rebuild current data for independent replay of a layout transformation.
    pub(crate) fn with_replayed_functions(
        &self,
        functions: Vec<ResolvedSelectedFunctionLayout>,
    ) -> Self {
        let mut program = ResolvedMachineLayout {
            selected: self.program.selected,
            machine: self.program.machine,
            pre_layout: self.program.pre_layout,
            post_allocation_machine_optimization: self.program.post_allocation_machine_optimization,
            target: self.program.target,
            policy: self.program.policy,
            identity: self.program.identity,
            functions,
            structural_unit_functions: self.program.structural_unit_functions.clone(),
        };
        program.identity = program.recomputed_identity();
        Self::from_program(program)
    }

    #[cfg(test)]
    pub(crate) fn functions_mut(&mut self) -> &mut [ResolvedSelectedFunctionLayout] {
        &mut Arc::make_mut(&mut self.program).functions
    }

    #[cfg(test)]
    #[allow(dead_code)]
    pub(crate) fn structural_unit_functions_mut(
        &mut self,
    ) -> &mut [ResolvedStructuralUnitFunctionLayout] {
        &mut Arc::make_mut(&mut self.program).structural_unit_functions
    }
}
