//! Physical edge preparation retains one semantic edge and explicit implementation control.
use crate::selection::model::SelectedInstructionError;
use selected_instructions::{SelectedBlockOrigin, SelectedSuccessorRole, SelectedValueTransport};
use selected_instructions::{SelectedFunction, VirtualRegisterId};
mod addresses;
mod construction;
mod descriptors;
mod scalar_references;
pub(super) use construction::prepare;

/// Selection scratch owned by one selected edge. Address preparation precedes
/// every input snapshot; cleanup follows all snapshots and precedes writes.
/// Keeping both phases off the predecessor prevents speculative pointer loads
/// on an untaken edge and preserves the predecessor's branch flags.
pub(in crate::selection) struct EdgeWork {
    pub edge: semantic_vocabulary::EdgeId,
    pub preparation: Vec<selected_instructions::SelectedInstruction>,
    pub cleanup: Vec<selected_instructions::SelectedInstruction>,
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
    u32,
    selected_instructions::LocalStorageSlotId,
    u32,
    u16,
)> {
    match transport {
        // An address transport stores a pointer it forms, never bytes it reads.
        selected_instructions::SelectedStructuralTransport::Unused
        | selected_instructions::SelectedStructuralTransport::Address { .. }
        | selected_instructions::SelectedStructuralTransport::ScalarReference { .. } => None,
        selected_instructions::SelectedStructuralTransport::WholeValue {
            argument,
            source_byte_offset,
            destination,
            byte_size,
            alignment,
        } => Some((
            argument,
            source_byte_offset,
            destination,
            byte_size,
            alignment,
        )),
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
