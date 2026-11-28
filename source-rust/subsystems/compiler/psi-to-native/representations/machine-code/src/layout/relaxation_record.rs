//! Completed branch-relaxation evidence. This record is not an admission
//! seal: explicit verification checks its roots, budget, content identity and
//! layout against the current program or the requested original baseline.
//! The record names its output by holding that layout; the baseline it started
//! from is checked against the first sweep's input revision.

use super::evidence::{
    X86BranchRelaxationAction, X86BranchRelaxationAttempt, X86BranchRelaxationIdentity,
    X86BranchRelaxationPolicy, X86BranchRelaxationRevisionIdentity,
};
use crate::{ResolvedMachineLayout, ResolvedSelectedFunctionLayout};
use optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};
use std::sync::Arc;
use target::NativeTarget;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct X86BranchRelaxationRecord {
    pub machine: Arc<post_allocation_machine::PostAllocationMachinePlan>, // Upstream post-allocation machine plan.
    pub pre_layout: crate::SelectedFormEncodingHandle, // Upstream pre-layout encoding.
    pub target: NativeTarget, // Target platform for which relaxation was performed.
    pub policy: X86BranchRelaxationPolicy, // Relaxation policy applied during the sweep.
    pub budget: OptimizationWorkBudget, // Work budget governing the relaxation sweep.
    pub usage: OptimizationWorkUsage, // Actual work consumed across all sweeps.
    pub output_revision: X86BranchRelaxationRevisionIdentity, // Final revision hash of the relaxed function roster.
    pub identity: X86BranchRelaxationIdentity, // Content identity of the complete relaxation artifact.
    pub attempts: Vec<X86BranchRelaxationAttempt>, // Per-branch inspection attempts recorded in sweep order.
    pub actions: Vec<X86BranchRelaxationAction>,   // Committed near-to-short branch rewrites.
    pub layout: Arc<ResolvedMachineLayout>,        // Rewritten function-relative machine layout.
}

impl X86BranchRelaxationRecord {
    pub fn machine(&self) -> &Arc<post_allocation_machine::PostAllocationMachinePlan> {
        &self.machine
    }

    pub fn pre_layout(&self) -> &crate::SelectedFormEncodingHandle {
        &self.pre_layout
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

    pub fn layout(&self) -> &ResolvedMachineLayout {
        &self.layout
    }

    pub fn shared_layout(&self) -> Arc<ResolvedMachineLayout> {
        Arc::clone(&self.layout)
    }
}
