//! Form descriptor addresses in their owning blocks, not at invocation entry.
//! Edge bridges write the destination slot directly. Delaying this pure address
//! calculation avoids keeping every future descriptor pointer live across calls;
//! it neither moves the slot nor reads its not-yet-initialized contents.
//!
//! An address join's slot instead holds its referent's address, which every
//! incoming edge bridge has written before control reaches the block. The entry
//! loads that address once, so later consumers receive the referent pointer an
//! incoming borrowed parameter would supply, never a pointer to the carrier.
use super::{
    Builder, LegalizedScalarFunction, SelectedInstructionId, SelectedInstructionKind,
    SelectedInstructionProvenance, SelectedMemoryAccess, SelectedMemoryAccessRole,
};
use crate::SelectedInstructionError;
use crate::selection::construction::scalar_graph::structural::invalid;
use crate::selection::construction::scalar_graph::structural::transport_register;
use selected_instructions::{
    FrameStorageSlotId, LocalStorageSlotId, SelectedLocalStorageSlot, SelectedMemoryAccessOrigin,
};

pub(in crate::selection::construction) fn block_entry(
    source: &LegalizedScalarFunction,
    block: &legalized_operations::LegalizedScalarBlock,
    builder: &mut Builder<'_>,
) -> Result<(), SelectedInstructionError> {
    for parameter in &block.structural_parameters {
        let joined = crate::selection::address_join_input::join(source, parameter.place).is_some();
        let shape = if joined {
            crate::selection::address_join_input::carrier_shape()
        } else if parameter.access == terminal_psi::StructuralAccess::Owned {
            crate::selection::aggregate_result_input::block_parameter_shape(source, parameter)
                .ok_or(SelectedInstructionError::custody())?
        } else {
            calling_conventions::ValueShape::integer(16, 8)
        };
        let slot = LocalStorageSlotId::StructuralBlockParameter {
            block: block.id,
            place: parameter.place,
        };
        builder
            .transport
            .local_slots
            .push(SelectedLocalStorageSlot {
                id: slot,
                byte_size: u32::from(shape.byte_size),
                alignment: shape.alignment,
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
            byte_count: u32::from(shape.byte_size),
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
        let pointer = if joined {
            let referent = transport_register(builder, parameter.place, 0)?;
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
                byte_count: u32::from(shape.byte_size),
                role: SelectedMemoryAccessRole::ReadPlace,
            });
            builder.emit(
                SelectedInstructionKind::Load64 { byte_offset: 0 },
                builder.constraints.keys.load64.ok_or_else(invalid)?,
                &[pointer, referent],
                SelectedInstructionProvenance::default(),
            )?;
            referent
        } else {
            pointer
        };
        builder.transport.pointers.push((parameter.place, pointer));
    }
    Ok(())
}
