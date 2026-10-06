//! Physical edge preparation retains one semantic edge and explicit implementation control.
use crate::selection::model::SelectedInstructionError;
use selected_instructions::{SelectedBlockOrigin, SelectedSuccessorRole, SelectedValueTransport};
use selected_instructions::{
    SelectedFunction, SelectedSuccessor, SelectedTerminator, VirtualRegisterId,
};
mod addresses;
mod construction;
mod descriptors;
pub(super) use construction::prepare;

/// Selection scratch: edge-owned calls are materialized with the caller's
/// storage facts, then installed only in the selected edge's transfer block.
pub(in crate::selection) struct EdgeCleanup {
    pub edge: semantic_vocabulary::EdgeId,
    pub instructions: Vec<selected_instructions::SelectedInstruction>,
}

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
        SelectedTerminator::Return { .. }
        | SelectedTerminator::Crash { .. }
        | SelectedTerminator::HostedExitProcess { .. } => Vec::new(),
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
    u16,
)> {
    match transport {
        // An address transport stores a pointer it forms, never bytes it reads.
        selected_instructions::SelectedStructuralTransport::Unused
        | selected_instructions::SelectedStructuralTransport::Address { .. } => None,
        selected_instructions::SelectedStructuralTransport::WholeValue {
            argument,
            destination,
            byte_size,
            alignment,
        } => Some((argument, destination, byte_size, alignment)),
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
