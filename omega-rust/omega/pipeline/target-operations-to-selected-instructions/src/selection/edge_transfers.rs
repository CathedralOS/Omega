//! Physical edge preparation retains one semantic edge and explicit implementation control.
use super::shared::*;
use selected_instructions::{SelectedBlockOrigin, SelectedSuccessorRole, SelectedValueTransport};
mod construction;
#[cfg(test)]
mod descriptor_tests;
mod descriptor_validation;
mod descriptors;
#[cfg(test)]
mod tests;
mod validation;
pub(super) use construction::prepare;
pub(super) use validation::project;

fn successors_mut(terminator: &mut SelectedTerminator) -> Vec<&mut SelectedSuccessor> {
    match terminator {
        SelectedTerminator::Jump { successor, .. } => vec![successor],
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
        SelectedTerminator::Return { .. } => Vec::new(),
    }
}

fn invalid(function: usize) -> SelectedInstructionError {
    SelectedInstructionError::FunctionProjectionMismatch { function }
}

fn instruction_count(function: &SelectedFunction) -> usize {
    function
        .blocks
        .iter()
        .map(|block| block.instructions.len() + 1)
        .sum()
}
