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
    Builder, LegalizedScalarFunction, SelectedInstructionProvenance, SelectedMemoryAccessRole,
};
use crate::SelectedInstructionError;
use crate::selection::construction::scalar_graph::structural::transport_register;
use selected_instructions::{
    LocalStorageSlotId, SelectedLocalStorageSlot, SelectedMemoryAccessOrigin,
};

pub(in crate::selection::construction) fn block_entry(
    source: &LegalizedScalarFunction,
    block: &legalized_operations::LegalizedScalarBlock,
    builder: &mut Builder<'_>,
) -> Result<(), SelectedInstructionError> {
    for parameter in &block.structural_parameters {
        let joined = crate::selection::address_join_input::join(source, parameter.place).is_some();
        let shape = if joined {
            crate::selection::address_join_input::carrier_shape(source, parameter)
        } else {
            crate::selection::aggregate_result_input::block_parameter_shape(source, parameter)
                .ok_or(SelectedInstructionError::custody())?
        };
        if shape.byte_size == 0 {
            // Custody is still bound by the semantic edge; no payload address
            // or slot exists for an empty owned arrival.
            continue;
        }
        let slot = LocalStorageSlotId::StructuralBlockParameter {
            block: block.id,
            place: parameter.place,
        };
        builder
            .transport
            .local_slots
            .push(SelectedLocalStorageSlot {
                id: slot,
                byte_size: shape.byte_size,
                alignment: shape.alignment,
            });
        let pointer = transport_register(builder, parameter.place, 0)?;
        super::memory_at_origin(
            builder,
            SelectedMemoryAccessOrigin::Block(block.id),
            parameter.place,
            0,
            shape.byte_size,
            SelectedMemoryAccessRole::AddressLocal { slot },
        )?;
        builder.emit_local_frame_address(
            slot,
            pointer,
            SelectedInstructionProvenance::default(),
        )?;
        let pointer = if joined && shape.byte_size == 8 {
            let referent = transport_register(builder, parameter.place, 0)?;
            super::memory_at_origin(
                builder,
                SelectedMemoryAccessOrigin::Block(block.id),
                parameter.place,
                0,
                shape.byte_size,
                SelectedMemoryAccessRole::ReadPlace,
            )?;
            builder.emit_load64(
                0,
                pointer,
                referent,
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
