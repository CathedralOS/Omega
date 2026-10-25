//! The atomic access row: operation tag 93, the accessed root, its carrier
//! path and scalar field, then one closed event sub-tag and that event's
//! operands and orderings.
//!
//! Each field is an exact closed `u8` space so a decoded row names one event
//! with no default: an unknown event, fetch operation, or ordering tag
//! rejects rather than decoding as a sibling. Whether the orderings are legal
//! for the event, whether the path is static, and whether the root grants the
//! access are verification's judgments, not the codec's.

use super::super::CodecError;
use super::super::wire::{Reader, Writer};
use super::operation_tags;
use crate::sections::semantic_module::structural_place_wire::{
    decode_structural_path, encode_structural_path,
};
use semantic_vocabulary::{PlaceId, StructuralFieldId};
use terminal_psi::{
    AtomicAccessEvent, AtomicReadModifyWrite, MemoryOrdering, OperationKind, StructuralPathSegment,
};

const LOAD: u8 = 1;
const STORE: u8 = 2;
const READ_MODIFY_WRITE: u8 = 3;
const SWAP: u8 = 4;
const COMPARE_EXCHANGE: u8 = 5;
const COMPARE_EXCHANGE_ONCE: u8 = 6;

pub(super) fn encode_atomic_access(
    writer: &mut Writer,
    place: PlaceId,
    path: &[StructuralPathSegment],
    field: StructuralFieldId,
    event: AtomicAccessEvent,
    resident_obligation: Option<semantic_vocabulary::ObligationId>,
) -> Result<(), CodecError> {
    writer.u8(if resident_obligation.is_some() { operation_tags::RESTRICTED_ATOMIC_ACCESS } else { operation_tags::ATOMIC_ACCESS });
    if let Some(obligation) = resident_obligation { writer.id(obligation); }
    writer.id(place);
    encode_structural_path(writer, "atomic access path", path)?;
    writer.id(field);
    match event {
        AtomicAccessEvent::Load { ordering } => {
            writer.u8(LOAD);
            encode_ordering(writer, ordering);
        }
        AtomicAccessEvent::Store { value, ordering } => {
            writer.u8(STORE);
            writer.id(value);
            encode_ordering(writer, ordering);
        }
        AtomicAccessEvent::ReadModifyWrite {
            operation,
            operand,
            ordering,
        } => {
            writer.u8(READ_MODIFY_WRITE);
            writer.u8(match operation {
                AtomicReadModifyWrite::FetchAdd => 1,
                AtomicReadModifyWrite::FetchSub => 2,
                AtomicReadModifyWrite::FetchAnd => 3,
                AtomicReadModifyWrite::FetchOr => 4,
                AtomicReadModifyWrite::FetchXor => 5,
            });
            writer.id(operand);
            encode_ordering(writer, ordering);
        }
        AtomicAccessEvent::Swap { value, ordering } => {
            writer.u8(SWAP);
            writer.id(value);
            encode_ordering(writer, ordering);
        }
        AtomicAccessEvent::CompareExchange {
            expected,
            replacement,
            success,
            failure,
        } => {
            writer.u8(COMPARE_EXCHANGE);
            writer.id(expected);
            writer.id(replacement);
            encode_ordering(writer, success);
            encode_ordering(writer, failure);
        }
        AtomicAccessEvent::CompareExchangeOnce {
            expected,
            replacement,
            success,
            failure,
        } => {
            writer.u8(COMPARE_EXCHANGE_ONCE);
            writer.id(expected);
            writer.id(replacement);
            encode_ordering(writer, success);
            encode_ordering(writer, failure);
        }
    }
    Ok(())
}

pub(super) fn decode_atomic_access(reader: &mut Reader<'_>, restricted: bool) -> Result<OperationKind, CodecError> {
    let resident_obligation = if restricted { Some(reader.id("ObligationId")?) } else { None };
    let place = reader.id("PlaceId")?;
    let path = decode_structural_path(reader)?;
    let field = reader.id("StructuralFieldId")?;
    let event = match reader.u8()? {
        LOAD => AtomicAccessEvent::Load {
            ordering: decode_ordering(reader)?,
        },
        STORE => AtomicAccessEvent::Store {
            value: reader.id("ValueId")?,
            ordering: decode_ordering(reader)?,
        },
        READ_MODIFY_WRITE => {
            let operation = match reader.u8()? {
                1 => AtomicReadModifyWrite::FetchAdd,
                2 => AtomicReadModifyWrite::FetchSub,
                3 => AtomicReadModifyWrite::FetchAnd,
                4 => AtomicReadModifyWrite::FetchOr,
                5 => AtomicReadModifyWrite::FetchXor,
                tag => return Err(CodecError::InvalidTag("AtomicReadModifyWrite", tag)),
            };
            AtomicAccessEvent::ReadModifyWrite {
                operation,
                operand: reader.id("ValueId")?,
                ordering: decode_ordering(reader)?,
            }
        }
        SWAP => AtomicAccessEvent::Swap {
            value: reader.id("ValueId")?,
            ordering: decode_ordering(reader)?,
        },
        COMPARE_EXCHANGE => AtomicAccessEvent::CompareExchange {
            expected: reader.id("ValueId")?,
            replacement: reader.id("ValueId")?,
            success: decode_ordering(reader)?,
            failure: decode_ordering(reader)?,
        },
        COMPARE_EXCHANGE_ONCE => AtomicAccessEvent::CompareExchangeOnce {
            expected: reader.id("ValueId")?,
            replacement: reader.id("ValueId")?,
            success: decode_ordering(reader)?,
            failure: decode_ordering(reader)?,
        },
        tag => return Err(CodecError::InvalidTag("AtomicAccessEvent", tag)),
    };
    Ok(OperationKind::AtomicAccess {
        place,
        path,
        field,
        event,
        resident_obligation,
    })
}

fn encode_ordering(writer: &mut Writer, ordering: MemoryOrdering) {
    writer.u8(match ordering {
        MemoryOrdering::NoOrdering => 1,
        MemoryOrdering::Receive => 2,
        MemoryOrdering::Publish => 3,
        MemoryOrdering::ReceivePublish => 4,
        MemoryOrdering::GlobalOrder => 5,
    });
}

fn decode_ordering(reader: &mut Reader<'_>) -> Result<MemoryOrdering, CodecError> {
    Ok(match reader.u8()? {
        1 => MemoryOrdering::NoOrdering,
        2 => MemoryOrdering::Receive,
        3 => MemoryOrdering::Publish,
        4 => MemoryOrdering::ReceivePublish,
        5 => MemoryOrdering::GlobalOrder,
        tag => return Err(CodecError::InvalidTag("MemoryOrdering", tag)),
    })
}

/// Fence tags are dedicated: the access-ordering space is not interchangeable.
pub(super) fn encode_atomic_fence(writer: &mut Writer, ordering: terminal_psi::FenceOrdering) {
    use terminal_psi::FenceOrdering;
    writer.u8(operation_tags::ATOMIC_FENCE);
    writer.u8(match ordering {
        FenceOrdering::Receive => 1,
        FenceOrdering::Publish => 2,
        FenceOrdering::ReceivePublish => 3,
    });
}

pub(super) fn decode_atomic_fence(reader: &mut Reader<'_>) -> Result<OperationKind, CodecError> {
    use terminal_psi::FenceOrdering;
    let ordering = match reader.u8()? {
        1 => FenceOrdering::Receive,
        2 => FenceOrdering::Publish,
        3 => FenceOrdering::ReceivePublish,
        tag => return Err(CodecError::InvalidTag("FenceOrdering", tag)),
    };
    Ok(OperationKind::AtomicFence { ordering })
}
