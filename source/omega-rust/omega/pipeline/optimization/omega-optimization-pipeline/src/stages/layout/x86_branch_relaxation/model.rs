use omega_optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};
use omega_selected_instructions::SelectedInstructionId;
use omega_target::NativeTarget;

use crate::{
    ResolvedSelectedFormLayoutIdentity, ResolvedSelectedFunctionLayout,
    StagedOptimizedResolvedSelectedFormLayout,
};

/// Explicit post-layout optimization policy. It is neither part of the
/// required baseline layout nor an encoder heuristic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum X86BranchRelaxationPolicy {
    X86RelaxConditionalBranchesToRel8V1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct X86BranchRelaxationIdentity(pub(super) [u8; 32]);

impl X86BranchRelaxationIdentity {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct X86BranchRelaxationRevisionIdentity(pub(super) [u8; 32]);

impl X86BranchRelaxationRevisionIdentity {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum X86BranchRelaxationAttemptOutcome {
    AlreadyShort,
    NearDisplacementOutsideI8,
    SelectedForRelaxation,
}

/// One branch inspected in deterministic function/block/instruction order.
/// Attempts stop at the selected branch in a mutating iteration; the terminal
/// no-change iteration records the complete remaining scan.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct X86BranchRelaxationAttempt {
    pub iteration: u64,
    pub input: X86BranchRelaxationRevisionIdentity,
    pub instruction: SelectedInstructionId,
    pub offset: u64,
    pub byte_displacement: i64,
    pub encoded_bytes: u8,
    pub outcome: X86BranchRelaxationAttemptOutcome,
}

/// Exact evidence for one monotone six-byte-near to two-byte-short rewrite.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct X86BranchRelaxationAction {
    pub iteration: u64,
    pub input: X86BranchRelaxationRevisionIdentity,
    pub output: X86BranchRelaxationRevisionIdentity,
    pub instruction: SelectedInstructionId,
    pub old_offset: u64,
    pub new_offset: u64,
    pub old_displacement: i64,
    pub new_displacement: i64,
    pub old_bytes: Vec<u8>,
    pub new_bytes: Vec<u8>,
}

/// Immutable result of the explicit post-layout fixed point. The baseline
/// layout remains retained by identity; this carrier owns only the rewritten
/// function-relative roster and grants no baseline-layout, emission, or
/// publication authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedOptimizedX86BranchRelaxation {
    pub(super) source: ResolvedSelectedFormLayoutIdentity,
    pub(super) selected: omega_selected_instructions::SelectedInstructionPlanIdentity,
    pub(super) machine: omega_machine_optimizer::PostAllocationMachineIdentity,
    pub(super) pre_layout: crate::SelectedFormEncodingIdentity,
    pub(super) target: NativeTarget,
    pub(super) policy: X86BranchRelaxationPolicy,
    pub(super) budget: OptimizationWorkBudget,
    pub(super) usage: OptimizationWorkUsage,
    pub(super) output: ResolvedSelectedFormLayoutIdentity,
    pub(super) output_revision: X86BranchRelaxationRevisionIdentity,
    pub(super) identity: X86BranchRelaxationIdentity,
    pub(super) attempts: Vec<X86BranchRelaxationAttempt>,
    pub(super) actions: Vec<X86BranchRelaxationAction>,
    pub(super) layout: StagedOptimizedResolvedSelectedFormLayout,
}

impl StagedOptimizedX86BranchRelaxation {
    pub const fn source(&self) -> ResolvedSelectedFormLayoutIdentity {
        self.source
    }

    pub const fn selected(&self) -> omega_selected_instructions::SelectedInstructionPlanIdentity {
        self.selected
    }

    pub const fn machine(&self) -> omega_machine_optimizer::PostAllocationMachineIdentity {
        self.machine
    }

    pub const fn pre_layout(&self) -> crate::SelectedFormEncodingIdentity {
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

    pub const fn layout(&self) -> &StagedOptimizedResolvedSelectedFormLayout {
        &self.layout
    }
}
