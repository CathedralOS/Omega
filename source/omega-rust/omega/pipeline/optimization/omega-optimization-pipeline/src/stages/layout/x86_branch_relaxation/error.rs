use omega_isa_x86_64::X86_64SelectedFormEncodingError;
use omega_selected_instructions::{SelectedBlockId, SelectedInstructionId};
use omega_target::NativeTarget;

use crate::OptimizedResolvedSelectedFormLayoutError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum X86BranchRelaxationWorkAxis {
    RuleEvaluations,
    Candidates,
    ValidationSteps,
    Commits,
    Iterations,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizedX86BranchRelaxationError {
    Source(OptimizedResolvedSelectedFormLayoutError),
    UnsupportedTarget(NativeTarget),
    BudgetExceeded(X86BranchRelaxationWorkAxis),
    DuplicateInstruction(SelectedInstructionId),
    MissingTargetBlock(SelectedBlockId),
    OffsetOverflow,
    NonContiguousBlock(SelectedBlockId),
    BranchFallthroughMismatch(SelectedInstructionId),
    MalformedBranch(SelectedInstructionId),
    BranchEffectsMismatch(SelectedInstructionId),
    NonDecreasingByteMeasure,
    X86_64(X86_64SelectedFormEncodingError),
    ArtifactMismatch,
}

impl std::fmt::Display for OptimizedX86BranchRelaxationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "optimized x86 branch relaxation failed: {self:?}"
        )
    }
}

impl std::error::Error for OptimizedX86BranchRelaxationError {}
