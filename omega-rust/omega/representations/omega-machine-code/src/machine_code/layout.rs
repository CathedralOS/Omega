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

pub use control_flow::*;
pub use evidence::*;
pub use functions::*;
pub use identity::{ResolvedSelectedFormLayoutIdentity, resolved_machine_layout_identity};
pub use policy::*;
pub use program::ResolvedMachineProgram;
pub use structural::*;

use omega_physical_instructions::PostAllocationMachineOptimizationCustody;
use omega_target::NativeTarget;

use crate::SelectedFormEncodingIdentity;

/// Unchecked current layout. Its content identity is not independent admission.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedMachineLayout {
    pub selected: omega_selected_instructions::SelectedInstructionPlanIdentity,
    pub machine: omega_physical_instructions::PostAllocationMachineIdentity,
    pub pre_layout: SelectedFormEncodingIdentity,
    pub post_allocation_machine_optimization: Option<PostAllocationMachineOptimizationCustody>,
    pub target: NativeTarget,
    pub policy: SelectedFunctionLayoutPolicy,
    pub identity: ResolvedSelectedFormLayoutIdentity,
    pub functions: Vec<ResolvedSelectedFunctionLayout>,
    pub structural_unit_functions: Vec<ResolvedStructuralUnitFunctionLayout>,
}

impl ResolvedMachineLayout {
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
