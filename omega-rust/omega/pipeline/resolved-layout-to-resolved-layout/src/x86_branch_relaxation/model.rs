use optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};
use target::NativeTarget;

use crate::{ResolvedSelectedFormLayoutIdentity, ResolvedSelectedFunctionLayout};

pub use machine_code::layout::evidence::{
    X86BranchRelaxationAction, X86BranchRelaxationAttempt, X86BranchRelaxationAttemptOutcome,
    X86BranchRelaxationIdentity, X86BranchRelaxationPolicy, X86BranchRelaxationRevisionIdentity,
};

/// Immutable result of the explicit post-layout fixed point. The baseline
/// layout remains retained by identity; this carrier owns only the rewritten
/// function-relative roster and grants no baseline-layout, emission, or
/// publication authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedOptimizedX86BranchRelaxation {
    pub(super) source: ResolvedSelectedFormLayoutIdentity,
    pub(super) selected: selected_instructions::SelectedInstructionPlanIdentity,
    pub(super) machine: physical_instructions::PostAllocationMachineIdentity,
    pub(super) pre_layout: machine_code::SelectedFormEncodingIdentity,
    pub(super) target: NativeTarget,
    pub(super) policy: X86BranchRelaxationPolicy,
    pub(super) budget: OptimizationWorkBudget,
    pub(super) usage: OptimizationWorkUsage,
    pub(super) output: ResolvedSelectedFormLayoutIdentity,
    pub(super) output_revision: X86BranchRelaxationRevisionIdentity,
    pub(super) identity: X86BranchRelaxationIdentity,
    pub(super) attempts: Vec<X86BranchRelaxationAttempt>,
    pub(super) actions: Vec<X86BranchRelaxationAction>,
    pub(super) layout: std::sync::Arc<machine_code::ResolvedMachineLayout>,
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
        let roots = super::identity::RevisionRoots {
            source: self.source,
            selected: self.selected,
            machine: self.machine,
            pre_layout: self.pre_layout,
            target: self.target,
        };
        self.identity = super::identity::artifact_identity(
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
