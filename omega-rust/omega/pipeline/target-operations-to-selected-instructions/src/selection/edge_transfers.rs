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
#[cfg(test)]
mod whole_value_tests;
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
        SelectedTerminator::Return { .. } | SelectedTerminator::HostedExitProcess { .. } => {
            Vec::new()
        }
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

fn stored_transport(
    transport: selected_instructions::SelectedStructuralTransport,
) -> Option<(
    VirtualRegisterId,
    selected_instructions::LocalStorageSlotId,
    u32,
    bool,
)> {
    match transport {
        selected_instructions::SelectedStructuralTransport::Unused => None,
        selected_instructions::SelectedStructuralTransport::Descriptor {
            argument,
            destination,
        } => Some((argument, destination, 16, false)),
        selected_instructions::SelectedStructuralTransport::WholeValue {
            argument,
            destination,
            byte_size,
            ..
        } => Some((argument, destination, u32::from(byte_size), true)),
    }
}

fn chunks(byte_size: u32) -> Vec<(u32, u8)> {
    let mut offset = 0;
    let mut chunks = Vec::new();
    while offset < byte_size {
        let width = [8, 4, 2, 1]
            .into_iter()
            .find(|width| *width <= byte_size - offset)
            .expect("positive remainder");
        chunks.push((offset, width as u8));
        offset += width;
    }
    chunks
}
