//! Exact selected-control projection shared by liveness transfer and evidence.

use selected_instructions::{SelectedInstruction, SelectedSuccessor, SelectedTerminator};

pub(super) fn instruction(terminator: &SelectedTerminator) -> &SelectedInstruction {
    match terminator {
        SelectedTerminator::ConditionalBranch { instruction, .. }
        | SelectedTerminator::ConditionalBranchU64LessThan { instruction, .. }
        | SelectedTerminator::ConditionalBranchI64LessThan { instruction, .. }
        | SelectedTerminator::Jump { instruction, .. }
        | SelectedTerminator::Return { instruction, .. }
        | SelectedTerminator::HostedExitProcess { instruction, .. } => instruction,
    }
}

/// Successors retain semantic polarity order: nonzero/zero or less/not-less.
pub(super) fn successors(
    terminator: &SelectedTerminator,
) -> impl DoubleEndedIterator<Item = &SelectedSuccessor> {
    let successors = match terminator {
        SelectedTerminator::ConditionalBranch {
            when_nonzero,
            when_zero,
            ..
        } => [Some(when_nonzero), Some(when_zero)],
        SelectedTerminator::ConditionalBranchU64LessThan {
            when_less,
            when_not_less,
            ..
        }
        | SelectedTerminator::ConditionalBranchI64LessThan {
            when_less,
            when_not_less,
            ..
        } => [Some(when_less), Some(when_not_less)],
        SelectedTerminator::Jump { successor, .. } => [Some(successor), None],
        SelectedTerminator::Return { .. } | SelectedTerminator::HostedExitProcess { .. } => {
            [None, None]
        }
    };
    successors.into_iter().flatten()
}
