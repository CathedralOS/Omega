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
pub mod text_section;

pub use control_flow::{
    ResolvedBranchEvidence, ResolvedConditionalBranchEvidence, ResolvedConditionalBranchPredicate,
    ResolvedJumpEvidence,
};
pub use evidence::{
    X86BranchRelaxationAction, X86BranchRelaxationAttempt, X86BranchRelaxationAttemptOutcome,
    X86BranchRelaxationIdentity, X86BranchRelaxationPolicy, X86BranchRelaxationRevisionIdentity,
    relaxation, relaxation_identity,
};
pub use functions::{
    ResolvedSelectedBlockLayout, ResolvedSelectedFormRow, ResolvedSelectedFunctionLayout,
};
pub use identity::{ResolvedSelectedFormLayoutIdentity, resolved_machine_layout_identity};
pub use policy::SelectedFunctionLayoutPolicy;
pub use program::ResolvedMachineProgram;
pub use text_section::{
    FunctionFragmentTextSectionManifest, FunctionFragmentTextSectionManifestDecodeError,
    FunctionFragmentTextSectionStage, FunctionFragmentTextSectionStatistics,
    FunctionFragmentTextSectionUnavailableData, InternalMachineCallResolutionKind,
    InternalMachineCallResolutionState, NormalizedForeignCallResolutionKind,
    NormalizedForeignCallResolutionState, PlacedBlockSpan, PlacedFunctionFragment,
    PlacedInstructionSpan, PlacedInternalMachineCallResolution,
    PlacedNormalizedForeignCallResolution, RelocationFreeTextSectionPlacement,
    TextSectionPlacementPolicy, TextSectionRelocationRequirements,
    relocation_free_text_section_identity,
};

use physical_instructions::PostAllocationMachineOptimizationCustody;
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

    /// The resolved layout for `machine`, when the layout carries it.
    pub fn function(
        &self,
        machine: semantic_vocabulary::MachineId,
    ) -> Option<&ResolvedSelectedFunctionLayout> {
        self.functions
            .iter()
            .find(|function| function.machine == machine)
    }

    /// Every function's block layouts, in function order.
    pub fn blocks(&self) -> impl Iterator<Item = &ResolvedSelectedBlockLayout> + '_ {
        self.functions()
            .iter()
            .flat_map(|function| &function.blocks)
    }

    /// Every function's resolved instructions, in function then block order.
    pub fn instructions(&self) -> impl Iterator<Item = &ResolvedSelectedFormRow> + '_ {
        self.blocks().flat_map(|block| &block.instructions)
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
        )
    }
}
