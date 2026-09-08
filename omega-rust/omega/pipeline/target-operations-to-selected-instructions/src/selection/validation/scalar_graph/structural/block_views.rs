//! Rejoin every descriptor destination address to its exact block declaration.
use super::*;
use selected_instructions::{
    FrameStorageSlotId, LocalStorageSlotId, SelectedLocalStorageSlot, SelectedMemoryAccessOrigin,
};

pub(super) fn entry(
    source: &LegalizedScalarFunction,
    replay: &mut Replay<'_>,
) -> Result<(), SelectedInstructionError> {
    for block in &source.blocks {
        for parameter in &block.structural_parameters {
            let slot = LocalStorageSlotId::StructuralBlockParameter {
                block: block.id,
                place: parameter.place,
            };
            replay.transport.local_slots.push(SelectedLocalStorageSlot {
                id: slot,
                byte_size: 16,
                alignment: 8,
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
                byte_count: 16,
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
    }
    Ok(())
}
