//! Invocation-local descriptor destinations; addresses do not read their contents.
use super::*;
use selected_instructions::{
    FrameStorageSlotId, LocalStorageSlotId, SelectedLocalStorageSlot, SelectedMemoryAccessOrigin,
};

pub(super) fn entry(
    source: &LegalizedScalarFunction,
    builder: &mut Builder<'_>,
) -> Result<(), SelectedInstructionError> {
    for block in &source.blocks {
        for parameter in &block.structural_parameters {
            let slot = LocalStorageSlotId::StructuralBlockParameter {
                block: block.id,
                place: parameter.place,
            };
            builder
                .transport
                .local_slots
                .push(SelectedLocalStorageSlot {
                    id: slot,
                    byte_size: 16,
                    alignment: 8,
                });
            let pointer = transport_register(builder, parameter.place, 0)?;
            builder.transport.memory.push(SelectedMemoryAccess {
                instruction: SelectedInstructionId(
                    builder
                        .instructions
                        .len()
                        .try_into()
                        .map_err(|_| invalid())?,
                ),
                origin: SelectedMemoryAccessOrigin::Block(block.id),
                place: parameter.place,
                byte_offset: 0,
                byte_count: 16,
                role: SelectedMemoryAccessRole::AddressLocal { slot },
            });
            builder.emit(
                SelectedInstructionKind::FrameAddress {
                    slot: FrameStorageSlotId::Local(slot),
                    byte_offset: 0,
                },
                builder.constraints.keys.frame_address.ok_or_else(invalid)?,
                &[pointer],
                SelectedInstructionProvenance::default(),
            )?;
            builder.transport.pointers.push((parameter.place, pointer));
        }
    }
    Ok(())
}
