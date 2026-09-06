//! Current function-relative machine layout, independent of its producing stages.
//!
//! Rows retain exact bytes, offsets, branch facts, and unresolved call fixups.
//! Optimization records identify replay inputs; these data alone do not admit
//! layout, authorize transformations, or grant executable publication.

pub mod control_flow;
pub mod evidence;
pub mod functions;
pub mod identity;
pub mod policy;
pub mod program;
pub mod structural;
pub mod text_section;

pub use control_flow::*;
pub use evidence::*;
pub use functions::*;
pub use identity::{ResolvedSelectedFormLayoutIdentity, resolved_machine_layout_identity};
pub use policy::*;
pub use program::ResolvedMachineProgram;
pub use structural::*;
pub use text_section::*;

use crate::{SelectedFormMachineOptimizationCustody, SelectedFormMovnOptimizationCustody};
use optimization_core::Optimization;
use physical_instructions::PostAllocationMachineOptimizationCustody;
use physical_instructions::{Aarch64CbnzFusionIdentity, Aarch64MovnMaterializationIdentity};
use target::NativeTarget;

use crate::SelectedFormEncodingIdentity;

/// Unchecked current layout. Its content identity is not independent admission.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedMachineLayout {
    pub selected: selected_instructions::SelectedInstructionPlanIdentity,
    pub machine: physical_instructions::PostAllocationMachineIdentity,
    pub pre_layout: SelectedFormEncodingIdentity,
    pub post_allocation_machine_optimization: Option<PostAllocationMachineOptimizationCustody>,
    pub target: NativeTarget,
    pub policy: SelectedFunctionLayoutPolicy,
    pub identity: ResolvedSelectedFormLayoutIdentity,
    pub functions: Vec<ResolvedSelectedFunctionLayout>,
    pub structural_unit_functions: Vec<ResolvedStructuralUnitFunctionLayout>,
}

impl ResolvedMachineLayout {
    pub fn selected(&self) -> selected_instructions::SelectedInstructionPlanIdentity {
        self.selected
    }

    pub fn machine(&self) -> physical_instructions::PostAllocationMachineIdentity {
        self.machine
    }

    pub fn pre_layout(&self) -> SelectedFormEncodingIdentity {
        self.pre_layout
    }

    pub fn machine_optimization(&self) -> Option<SelectedFormMachineOptimizationCustody> {
        match self.post_allocation_machine_optimization {
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
        match self.post_allocation_machine_optimization {
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
        self.post_allocation_machine_optimization
    }

    pub fn target(&self) -> NativeTarget {
        self.target
    }

    pub fn policy(&self) -> SelectedFunctionLayoutPolicy {
        self.policy
    }

    pub fn identity(&self) -> ResolvedSelectedFormLayoutIdentity {
        self.identity
    }

    pub fn functions(&self) -> &[ResolvedSelectedFunctionLayout] {
        &self.functions
    }

    pub fn structural_unit_functions(&self) -> &[ResolvedStructuralUnitFunctionLayout] {
        &self.structural_unit_functions
    }

    pub fn recomputed_identity(&self) -> ResolvedSelectedFormLayoutIdentity {
        resolved_machine_layout_identity(
            self.selected,
            self.machine,
            self.pre_layout,
            self.post_allocation_machine_optimization,
            self.target,
            self.policy,
            &self.functions,
            &self.structural_unit_functions,
        )
    }
}
