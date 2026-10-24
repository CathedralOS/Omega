//! Snapshot both words before any edge destination is installed.
//! Loans preserve backing addresses; no scalar contents are copied or written.

use super::emission::Emission;
use crate::SelectedInstructionError;
use crate::selection::edge_transfers::invalid;
use selected_instructions::{
    FrameStorageSlotId, LocalStorageSlotId, SelectedAddressBase, SelectedInstructionKind,
    SelectedMemoryAccessRole, SelectedScalarReferenceSource, SelectedStructuralBinding,
    SelectedStructuralTransport, VirtualRegisterId,
};
use semantic_vocabulary::{IntegerValue, PlaceId};

pub(super) struct Snapshot {
    destination: LocalStorageSlotId,
    place: PlaceId,
    address: VirtualRegisterId,
    codec: VirtualRegisterId,
}

pub(super) fn snapshot(
    emission: &mut Emission<'_>,
    bindings: &[SelectedStructuralBinding],
) -> Result<Vec<Snapshot>, SelectedInstructionError> {
    let function = emission.function;
    let error = || invalid(function);
    let mut snapshots = Vec::new();
    for binding in bindings {
        let SelectedStructuralTransport::ScalarReference {
            base,
            byte_offset,
            source,
            destination,
        } = binding.transport
        else {
            continue;
        };
        let place = binding.semantic.argument.place;
        let source_extent = match source {
            SelectedScalarReferenceSource::Descriptor => 16,
            SelectedScalarReferenceSource::Canonical { codec } => u32::from(
                runtime_abi::ScalarStorageCodec::decode(codec)
                    .ok_or_else(error)?
                    .byte_size(),
            ),
        };
        let pointer = match base {
            SelectedAddressBase::Local(slot) => emission.emit(
                SelectedInstructionKind::FrameAddress {
                    slot: FrameStorageSlotId::Local(slot),
                    byte_offset,
                },
                emission.constraints.keys.frame_address.ok_or_else(error)?,
                &[],
                place,
                byte_offset,
                Some((
                    source_extent,
                    SelectedMemoryAccessRole::AddressLocal { slot },
                )),
            )?,
            SelectedAddressBase::Register(register) => emission.emit(
                if byte_offset == 0 {
                    SelectedInstructionKind::CopyI64
                } else {
                    SelectedInstructionKind::AddressOffset { byte_offset }
                },
                if byte_offset == 0 {
                    emission.constraints.keys.copy_i64
                } else {
                    emission.constraints.keys.address_offset.ok_or_else(error)?
                },
                &[register],
                place,
                byte_offset,
                None,
            )?,
        };
        let (address, codec) = match source {
            SelectedScalarReferenceSource::Descriptor => {
                let address = emission.emit(
                    SelectedInstructionKind::Load64 { byte_offset: 0 },
                    emission.constraints.keys.load64.ok_or_else(error)?,
                    &[pointer],
                    place,
                    byte_offset,
                    None,
                )?;
                emission.source_reads(binding, byte_offset)?;
                let codec_offset = byte_offset.checked_add(8).ok_or_else(error)?;
                let codec = emission.emit(
                    SelectedInstructionKind::Load64 { byte_offset: 8 },
                    emission.constraints.keys.load64.ok_or_else(error)?,
                    &[pointer],
                    place,
                    codec_offset,
                    None,
                )?;
                emission.source_reads(binding, codec_offset)?;
                (address, codec)
            }
            SelectedScalarReferenceSource::Canonical { codec } => {
                let codec = emission.emit(
                    SelectedInstructionKind::MaterializeI64 {
                        value: IntegerValue::Unsigned(u128::from(codec)),
                    },
                    emission.constraints.keys.materialize_i64,
                    &[],
                    place,
                    byte_offset,
                    None,
                )?;
                (pointer, codec)
            }
        };
        snapshots.push(Snapshot {
            destination,
            place: binding.semantic.parameter,
            address,
            codec,
        });
    }
    Ok(snapshots)
}

pub(super) fn store(
    emission: &mut Emission<'_>,
    snapshots: &[Snapshot],
) -> Result<(), SelectedInstructionError> {
    for snapshot in snapshots {
        for (byte_offset, register) in [(0, snapshot.address), (8, snapshot.codec)] {
            emission.store_local_word(
                snapshot.destination,
                byte_offset,
                register,
                snapshot.place,
            )?;
        }
    }
    Ok(())
}
