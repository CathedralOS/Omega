//! Optimizer module role: executable entrance. Explicit x86 conditional-branch relaxation.

mod catalog;
mod compute;
mod error;
use machine_code::layout::evidence::relaxation_identity as identity;
mod validation;

pub(crate) use catalog::FunctionRelativeLayoutCatalogError;
pub(crate) use catalog::x86_rel8_selected;
pub use error::OptimizedX86BranchRelaxationError;

use optimization_core::OptimizationWorkBudget;
use register_model::ValidatedPhysicalRegisterModel;
use selected_instructions_to_register_homes::ValidatedSelectedAnalysis;

use compute::compute_relaxation;
pub use machine_code::layout::evidence::{
    X86BranchRelaxationAction, X86BranchRelaxationAttempt, X86BranchRelaxationAttemptOutcome,
    X86BranchRelaxationIdentity, X86BranchRelaxationPolicy, X86BranchRelaxationRevisionIdentity,
};
use machine_code::{ResolvedSelectedFormLayoutIdentity, ResolvedSelectedFunctionLayout};
use optimization_core::OptimizationWorkUsage;
use post_allocation_machine_to_selected_form_encoding::StagedOptimizedSelectedFormEncoding;
use register_homes_to_post_allocation_machine::StagedOptimizedPostAllocationMachinePlan;
use selected_form_encoding_to_resolved_layout::{
    StagedOptimizedResolvedSelectedFormLayout, validate_optimized_resolved_selected_form_layout,
};
use target::NativeTarget;
use validation::validate_roots;

pub(crate) fn stage_optimized_x86_branch_relaxation(
    physical: &ValidatedPhysicalRegisterModel,
    source: &StagedOptimizedResolvedSelectedFormLayout,
    budget: OptimizationWorkBudget,
) -> Result<StagedOptimizedX86BranchRelaxation, OptimizedX86BranchRelaxationError> {
    validate_roots(source, physical)?;
    compute_relaxation(source, physical, budget)
}

/// Check the artifact against its retained roots: the source layout is
/// re-admitted as the input its own owner produced, the target is x86-64, and
/// the artifact's identity fields agree with the source layout, the policy and
/// the work it consumed. The relaxation is not re-derived here — the producer
/// is the mechanism, and the corpus and the stage's tests observe it
/// (verification placement, `omega-rust/pipeline.md`).
pub(crate) fn validate_optimized_x86_branch_relaxation<S: ValidatedSelectedAnalysis>(
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
    Ok(())
}

/// Immutable result of the explicit post-layout fixed point. The baseline
/// layout remains retained by identity; this carrier owns only the rewritten
/// function-relative roster and grants no baseline-layout, emission, or
/// publication authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedOptimizedX86BranchRelaxation {
    source: ResolvedSelectedFormLayoutIdentity,
    selected: selected_instructions::SelectedInstructionPlanIdentity,
    machine: physical_instructions::PostAllocationMachineIdentity,
    pre_layout: machine_code::SelectedFormEncodingIdentity,
    target: NativeTarget,
    policy: X86BranchRelaxationPolicy,
    budget: OptimizationWorkBudget,
    usage: OptimizationWorkUsage,
    output: ResolvedSelectedFormLayoutIdentity,
    output_revision: X86BranchRelaxationRevisionIdentity,
    identity: X86BranchRelaxationIdentity,
    attempts: Vec<X86BranchRelaxationAttempt>,
    actions: Vec<X86BranchRelaxationAction>,
    layout: std::sync::Arc<machine_code::ResolvedMachineLayout>,
}

impl StagedOptimizedX86BranchRelaxation {
    pub const fn source(&self) -> ResolvedSelectedFormLayoutIdentity {
        self.source
    }

    pub const fn selected(&self) -> selected_instructions::SelectedInstructionPlanIdentity {
        self.selected
    }

    pub const fn machine(&self) -> physical_instructions::PostAllocationMachineIdentity {
        self.machine
    }

    pub const fn pre_layout(&self) -> machine_code::SelectedFormEncodingIdentity {
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

    pub fn layout(&self) -> &machine_code::ResolvedMachineLayout {
        &self.layout
    }

    pub fn shared_layout(&self) -> std::sync::Arc<machine_code::ResolvedMachineLayout> {
        std::sync::Arc::clone(&self.layout)
    }
}
