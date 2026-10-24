//! Address transports lend a referent's address on the edge that binds it.
//!
//! An address join's slot receives one pointer, never referent bytes. The
//! bridge forms that pointer from the transport's base after every scalar and
//! descriptor snapshot: a frame-resident home through `FrameAddress`, a live
//! pointer register directly or through `AddressOffset`. Neither form reads
//! memory, so no destination write can disturb another binding's source. The
//! pointer is then stored whole into the join's own block slot.
use super::emission::Emission;
use crate::SelectedInstructionError;
use crate::selection::edge_transfers::invalid;
use selected_instructions::{
    SelectedAddressBase, SelectedInstructionKind, SelectedMemoryAccessRole,
    SelectedStructuralBinding, SelectedStructuralTransport,
};

/// One address transport: its base, displacement, lent span and join slot.
type Address = (
    SelectedAddressBase,
    u32,
    u32,
    selected_instructions::LocalStorageSlotId,
);

fn address(transport: SelectedStructuralTransport) -> Option<Address> {
    match transport {
        SelectedStructuralTransport::Address {
            base,
            byte_offset,
            byte_count,
            destination,
        } => Some((base, byte_offset, byte_count, destination)),
        SelectedStructuralTransport::Unused
        | SelectedStructuralTransport::WholeValue { .. }
        | SelectedStructuralTransport::ScalarReference { .. } => None,
    }
}

/// Whether forming this address needs its own instruction and register.
fn computed(base: SelectedAddressBase, byte_offset: u32) -> bool {
    matches!(base, SelectedAddressBase::Local(_)) || byte_offset != 0
}

pub(super) fn store(
    emission: &mut Emission<'_>,
    bindings: &[SelectedStructuralBinding],
) -> Result<(), SelectedInstructionError> {
    let function = emission.function;
    let error = || invalid(function);
    for binding in bindings {
        let Some((base, byte_offset, byte_count, destination)) = address(binding.transport) else {
            continue;
        };
        let place = binding.semantic.argument.place;
        let pointer = if computed(base, byte_offset) {
            let (kind, key, inputs, access) = match base {
                SelectedAddressBase::Local(slot) => (
                    SelectedInstructionKind::FrameAddress {
                        slot: selected_instructions::FrameStorageSlotId::Local(slot),
                        byte_offset,
                    },
                    emission.constraints.keys.frame_address.ok_or_else(error)?,
                    Vec::new(),
                    Some((byte_count, SelectedMemoryAccessRole::AddressLocal { slot })),
                ),
                SelectedAddressBase::Register(register) => (
                    SelectedInstructionKind::AddressOffset { byte_offset },
                    emission.constraints.keys.address_offset.ok_or_else(error)?,
                    vec![register],
                    None,
                ),
            };
            emission.emit(kind, key, &inputs, place, byte_offset, access)?
        } else {
            let SelectedAddressBase::Register(register) = base else {
                return Err(error());
            };
            register
        };
        emission.store_local_word(destination, 0, pointer, binding.semantic.parameter)?;
    }
    Ok(())
}
