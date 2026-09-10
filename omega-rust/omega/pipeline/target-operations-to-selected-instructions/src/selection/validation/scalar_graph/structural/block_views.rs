//! Replay each descriptor address at its owning block's entry.
//! Destination edge copies initialize the slot independently of this address;
//! exact block placement keeps the shorter pointer lifetime independently checked.
use super::*;
use selected_instructions::{
    FrameStorageSlotId, LocalStorageSlotId, SelectedLocalStorageSlot, SelectedMemoryAccessOrigin,
};

pub(in crate::selection::validation) fn block_entry(
    source: &LegalizedScalarFunction,
    block: &legalized_operations::LegalizedScalarBlock,
    replay: &mut Replay<'_>,
) -> Result<(), SelectedInstructionError> {
    for parameter in &block.structural_parameters {
        let shape = if parameter.access == terminal_psi::StructuralAccess::Owned {
            crate::selection::aggregate_result_input::block_parameter_shape(source, parameter)
                .ok_or(SelectedInstructionError::SourceCustodyMismatch)?
        } else {
            calling_conventions::ValueShape::integer(16, 8)
        };
        let slot = LocalStorageSlotId::StructuralBlockParameter {
            block: block.id,
            place: parameter.place,
        };
        replay.transport.local_slots.push(SelectedLocalStorageSlot {
            id: slot,
            byte_size: u32::from(shape.byte_size),
            alignment: shape.alignment,
        });
        let pointer = result(replay, parameter.place, 0)?;
        replay.transport.memory.push(SelectedMemoryAccess {
            instruction: SelectedInstructionId(
                replay
                    .instruction_cursor
                    .try_into()
                    .map_err(|_| replay.invalid())?,
            ),
            origin: SelectedMemoryAccessOrigin::Block(block.id),
            place: parameter.place,
            byte_offset: 0,
            byte_count: u32::from(shape.byte_size),
            role: SelectedMemoryAccessRole::AddressLocal { slot },
        });
        replay.check_instruction(
            SelectedInstructionKind::FrameAddress {
                slot: FrameStorageSlotId::Local(slot),
                byte_offset: 0,
            },
            replay
                .constraints
                .keys
                .frame_address
                .ok_or_else(|| replay.invalid())?,
            &[pointer],
            &SelectedInstructionProvenance::default(),
        )?;
        replay.transport.pointers.push((parameter.place, pointer));
    }
    Ok(())
}
