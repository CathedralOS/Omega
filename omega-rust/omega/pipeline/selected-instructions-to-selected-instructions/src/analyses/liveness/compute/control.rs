//! Exact selected-control projection shared by liveness transfer and evidence.

use selected_instructions::{SelectedInstruction, SelectedSuccessor, SelectedTerminator};

pub(super) fn instruction(terminator: &SelectedTerminator) -> &SelectedInstruction {
    match terminator {
        SelectedTerminator::ConditionalBranch { instruction, .. }
        | SelectedTerminator::ConditionalBranchU64LessThan { instruction, .. }
        | SelectedTerminator::ConditionalBranchI64LessThan { instruction, .. }
        | SelectedTerminator::Jump { instruction, .. }
        | SelectedTerminator::Return { instruction, .. } => instruction,
    }
}

/// Successors retain semantic polarity order: nonzero/zero or less/not-less.
pub(super) fn successors(terminator: &SelectedTerminator) -> Vec<&SelectedSuccessor> {
    match terminator {
        SelectedTerminator::ConditionalBranch {
            when_nonzero,
            when_zero,
            ..
        } => vec![when_nonzero, when_zero],
        SelectedTerminator::ConditionalBranchU64LessThan {
            when_less,
            when_not_less,
            ..
        }
        | SelectedTerminator::ConditionalBranchI64LessThan {
            when_less,
            when_not_less,
            ..
        } => vec![when_less, when_not_less],
        SelectedTerminator::Jump { successor, .. } => vec![successor],
        SelectedTerminator::Return { .. } => Vec::new(),
    }
}
