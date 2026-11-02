//! Exact selected-control projection shared by liveness transfer and evidence.

use selected_instructions::{SelectedSuccessor, SelectedTerminator};

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
        selected_instructions::conditional_branch_less_pair_pattern!(
            when_less,
            when_not_less,
            ..
        ) => [Some(when_less), Some(when_not_less)],
        SelectedTerminator::Jump { successor, .. } => [Some(successor), None],
        selected_instructions::terminator_no_successor_pattern!(..) => [None, None],
    };
    successors.into_iter().flatten()
}
