//! Optimizer module role: executable entrance. Explicit x86 conditional-branch relaxation and independent replay.

mod catalog;
mod compute;
mod error;
use post_allocation_machine_to_selected_form_encoding::machine_code::layout::evidence::relaxation_identity as identity;
mod validation;

pub use catalog::x86_rel8_selected;
pub use catalog::{
    FUNCTION_RELATIVE_LAYOUT_RULE_CATALOG, FunctionRelativeLayoutCatalogError,
    FunctionRelativeLayoutRuleCatalogEntry, ORDERED_FUNCTION_RELATIVE_LAYOUT_RULES,
};
pub use error::{OptimizedX86BranchRelaxationError, X86BranchRelaxationWorkAxis};

use optimization_core::OptimizationWorkBudget;
use selected_instructions_to_register_homes::ValidatedSelectedAnalysis;
use target_operations_to_selected_instructions::register_model::ValidatedPhysicalRegisterModel;

use compute::{compute_relaxation, replay_relaxation};
use optimization_core::OptimizationWorkUsage;
use post_allocation_machine_to_selected_form_encoding::StagedOptimizedSelectedFormEncoding;
pub use post_allocation_machine_to_selected_form_encoding::machine_code::layout::evidence::{
    X86BranchRelaxationAction, X86BranchRelaxationAttempt, X86BranchRelaxationAttemptOutcome,
    X86BranchRelaxationIdentity, X86BranchRelaxationPolicy, X86BranchRelaxationRevisionIdentity,
};
use post_allocation_machine_to_selected_form_encoding::machine_code::{
    ResolvedSelectedFormLayoutIdentity, ResolvedSelectedFunctionLayout,
};
use register_homes_to_post_allocation_machine::StagedOptimizedPostAllocationMachinePlan;
use selected_form_encoding_to_resolved_layout::{
    StagedOptimizedResolvedSelectedFormLayout, validate_optimized_resolved_selected_form_layout,
};
use target::NativeTarget;
use validation::{compare_replayed_evidence, validate_roots};

pub fn stage_optimized_x86_branch_relaxation<S: ValidatedSelectedAnalysis>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    pre_layout: &StagedOptimizedSelectedFormEncoding,
    source: &StagedOptimizedResolvedSelectedFormLayout,
    budget: OptimizationWorkBudget,
) -> Result<StagedOptimizedX86BranchRelaxation, OptimizedX86BranchRelaxationError> {
    validate_optimized_resolved_selected_form_layout(
        selected, machine, physical, pre_layout, source,
    )
    .map_err(OptimizedX86BranchRelaxationError::Source)?;
    validate_roots(source, physical)?;
    let artifact = compute_relaxation(source, physical, budget)?;
    validate_optimized_x86_branch_relaxation(
        selected, machine, physical, pre_layout, source, &artifact,
    )?;
    Ok(artifact)
}

/// Independent replay does not call the production fixed-point driver. It
/// reconstructs the ordered scan, each shrink, every dense offset, the terminal
/// no-change sweep, work usage, revisions, and final receipt.
pub fn validate_optimized_x86_branch_relaxation<S: ValidatedSelectedAnalysis>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    pre_layout: &StagedOptimizedSelectedFormEncoding,
    source: &StagedOptimizedResolvedSelectedFormLayout,
    artifact: &StagedOptimizedX86BranchRelaxation,
) -> Result<(), OptimizedX86BranchRelaxationError> {
    validate_optimized_resolved_selected_form_layout(
        selected, machine, physical, pre_layout, source,
    )
    .map_err(OptimizedX86BranchRelaxationError::Source)?;
    validate_roots(source, physical)?;
    if artifact.source != source.identity()
        || artifact.selected != source.selected()
        || artifact.machine != source.machine()
        || artifact.pre_layout != source.pre_layout()
        || artifact.target != source.target()
        || artifact.policy != X86BranchRelaxationPolicy::X86RelaxConditionalBranchesToRel8V1
        || !artifact.usage.within(artifact.budget)
    {
        return Err(OptimizedX86BranchRelaxationError::ArtifactMismatch);
    }
    let replayed = replay_relaxation(source, physical, artifact.budget)?;
    compare_replayed_evidence(artifact, &replayed)?;
    if artifact != &replayed {
        return Err(OptimizedX86BranchRelaxationError::ArtifactMismatch);
    }
    Ok(())
}

/// Immutable result of the explicit post-layout fixed point. The baseline
/// layout remains retained by identity; this carrier owns only the rewritten
/// function-relative roster and grants no baseline-layout, emission, or
/// publication authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedOptimizedX86BranchRelaxation {
    source: ResolvedSelectedFormLayoutIdentity,
    selected: target_operations_to_selected_instructions::SelectedInstructionPlanIdentity,
    machine: register_homes_to_post_allocation_machine::PostAllocationMachineIdentity,
    pre_layout: post_allocation_machine_to_selected_form_encoding::machine_code::SelectedFormEncodingIdentity,
    target: NativeTarget,
    policy: X86BranchRelaxationPolicy,
    budget: OptimizationWorkBudget,
    usage: OptimizationWorkUsage,
    output: ResolvedSelectedFormLayoutIdentity,
    output_revision: X86BranchRelaxationRevisionIdentity,
    identity: X86BranchRelaxationIdentity,
    attempts: Vec<X86BranchRelaxationAttempt>,
    actions: Vec<X86BranchRelaxationAction>,
    layout: std::sync::Arc<post_allocation_machine_to_selected_form_encoding::machine_code::ResolvedMachineLayout>,
}

impl StagedOptimizedX86BranchRelaxation {
    pub const fn source(&self) -> ResolvedSelectedFormLayoutIdentity {
        self.source
    }

    pub const fn selected(
        &self,
    ) -> target_operations_to_selected_instructions::SelectedInstructionPlanIdentity {
        self.selected
    }

    pub const fn machine(
        &self,
    ) -> register_homes_to_post_allocation_machine::PostAllocationMachineIdentity {
        self.machine
    }

    pub const fn pre_layout(
        &self,
    ) -> post_allocation_machine_to_selected_form_encoding::machine_code::SelectedFormEncodingIdentity
    {
        self.pre_layout
    }

    pub const fn target(&self) -> NativeTarget {
        self.target
    }

    pub const fn policy(&self) -> X86BranchRelaxationPolicy {
        self.policy
    }

    pub const fn budget(&self) -> OptimizationWorkBudget {
        self.budget
    }

    pub const fn usage(&self) -> OptimizationWorkUsage {
        self.usage
    }

    pub const fn output(&self) -> ResolvedSelectedFormLayoutIdentity {
        self.output
    }

    pub const fn output_revision(&self) -> X86BranchRelaxationRevisionIdentity {
        self.output_revision
    }

    pub const fn identity(&self) -> X86BranchRelaxationIdentity {
        self.identity
    }

    pub fn attempts(&self) -> &[X86BranchRelaxationAttempt] {
        &self.attempts
    }

    pub fn actions(&self) -> &[X86BranchRelaxationAction] {
        &self.actions
    }

    pub fn functions(&self) -> &[ResolvedSelectedFunctionLayout] {
        self.layout.functions()
    }

    pub fn layout(
        &self,
    ) -> &post_allocation_machine_to_selected_form_encoding::machine_code::ResolvedMachineLayout
    {
        &self.layout
    }

    pub fn shared_layout(
        &self,
    ) -> std::sync::Arc<
        post_allocation_machine_to_selected_form_encoding::machine_code::ResolvedMachineLayout,
    > {
        std::sync::Arc::clone(&self.layout)
    }
}
