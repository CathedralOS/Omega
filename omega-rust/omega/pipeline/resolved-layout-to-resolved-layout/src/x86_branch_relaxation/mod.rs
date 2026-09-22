//! Optimizer module role: executable entrance. Explicit x86 conditional-branch relaxation and independent replay.

mod catalog;
mod compute;
mod error;
use machine_code::layout::evidence::relaxation_identity as identity;
mod validation;

pub use catalog::x86_rel8_selected;
pub use catalog::{
    FUNCTION_RELATIVE_LAYOUT_RULE_CATALOG, FunctionRelativeLayoutCatalogError,
    FunctionRelativeLayoutRuleCatalogEntry, ORDERED_FUNCTION_RELATIVE_LAYOUT_RULES,
};
pub use error::*;

use optimization_core::OptimizationWorkBudget;
use register_model::ValidatedPhysicalRegisterModel;
use selected_instructions_to_register_homes::ValidatedSelectedAnalysis;

use crate::{ResolvedSelectedFormLayoutIdentity, ResolvedSelectedFunctionLayout};
use crate::{
    StagedOptimizedResolvedSelectedFormLayout, validate_optimized_resolved_selected_form_layout,
};
use compute::{compute_relaxation, replay_relaxation};
pub use machine_code::layout::evidence::{
    X86BranchRelaxationAction, X86BranchRelaxationAttempt, X86BranchRelaxationAttemptOutcome,
    X86BranchRelaxationIdentity, X86BranchRelaxationPolicy, X86BranchRelaxationRevisionIdentity,
};
use optimization_core::OptimizationWorkUsage;
use post_allocation_machine_to_selected_form_encoding::StagedOptimizedSelectedFormEncoding;
use register_homes_to_post_allocation_machine::StagedOptimizedPostAllocationMachinePlan;
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

    /// Test-only authenticated corruption. This grants no production
    /// construction, validation, layout, emission, or publication authority.
    #[cfg(any(test, feature = "test-support"))]
    pub fn corrupt_first_action_bytes_and_reauthenticate_for_test(&mut self) {
        self.actions
            .first_mut()
            .expect("the rel8 corruption fixture must contain one action")
            .new_bytes[0] ^= 1;
        self.reauthenticate_for_test();
    }

    /// Test-only authenticated corruption of the recorded attempt roster: the
    /// first attempt reports the terminal decline instead of its recorded
    /// outcome. Independent replay still reconstructs the true roster.
    #[cfg(any(test, feature = "test-support"))]
    pub fn corrupt_first_attempt_outcome_and_reauthenticate_for_test(&mut self) {
        let attempt = self
            .attempts
            .first_mut()
            .expect("the rel8 corruption fixture must contain one attempt");
        attempt.outcome = match attempt.outcome {
            X86BranchRelaxationAttemptOutcome::SelectedForRelaxation => {
                X86BranchRelaxationAttemptOutcome::AlreadyShort
            }
            _ => X86BranchRelaxationAttemptOutcome::SelectedForRelaxation,
        };
        self.reauthenticate_for_test();
    }

    /// Test-only authenticated corruption of the retained layout: one byte in a
    /// row the rewrite never touched drifts, with the layout and artifact
    /// identities honestly recomputed over the drifted content. Independent
    /// replay reconstructs the untouched layout and rejects.
    #[cfg(any(test, feature = "test-support"))]
    pub fn corrupt_retained_layout_row_and_reauthenticate_for_test(&mut self) {
        let program = std::sync::Arc::make_mut(&mut self.layout);
        let row = program
            .functions
            .iter_mut()
            .flat_map(|function| function.blocks.iter_mut())
            .flat_map(|block| block.instructions.iter_mut())
            .find(|row| row.branch.is_none() && !row.bytes.is_empty())
            .expect("the rel8 corruption fixture must contain a non-branch row");
        row.bytes[0] ^= 1;
        program.identity = program.recomputed_identity();
        self.output = program.identity;
        self.reauthenticate_for_test();
    }

    /// Test-only authenticated corruption of the recorded budget: the artifact
    /// claims a legal budget one below its measured rule-evaluation usage.
    /// Independent replay checks usage-within-budget before replaying.
    #[cfg(any(test, feature = "test-support"))]
    pub fn corrupt_recorded_budget_and_reauthenticate_for_test(&mut self) {
        self.budget = OptimizationWorkBudget::new(
            self.usage
                .rule_evaluations
                .checked_sub(1)
                .expect("the rel8 corruption fixture must record rule evaluations"),
            self.usage.candidates,
            self.usage.validation_steps,
            self.usage.commits,
            self.usage.iterations,
        )
        .expect("the shrunk recorded budget must remain a legal budget");
        self.reauthenticate_for_test();
    }

    #[cfg(any(test, feature = "test-support"))]
    fn reauthenticate_for_test(&mut self) {
        let roots = self::identity::RevisionRoots {
            source: self.source,
            selected: self.selected,
            machine: self.machine,
            pre_layout: self.pre_layout,
            target: self.target,
        };
        self.identity = self::identity::artifact_identity(
            roots,
            self.policy,
            self.budget,
            self.usage,
            self.output,
            self.output_revision,
            &self.attempts,
            &self.actions,
            self.layout.functions(),
        );
    }
}
