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

pub(super) fn encode_atomic_access(
    writer: &mut Writer,
    place: PlaceId,
    path: &[StructuralPathSegment],
    field: StructuralFieldId,
    event: AtomicAccessEvent,
) -> Result<(), CodecError> {
    writer.u8(operation_tags::ATOMIC_ACCESS);
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
    }
    Ok(())
}

pub(super) fn decode_atomic_access(reader: &mut Reader<'_>) -> Result<OperationKind, CodecError> {
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
        tag => return Err(CodecError::InvalidTag("AtomicAccessEvent", tag)),
    };
    Ok(OperationKind::AtomicAccess {
        place,
        path,
        field,
        event,
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

#[cfg(test)]
mod tests {
    use semantic_vocabulary::{
        BlockId, EdgeId, IntegerSign, IntegerType, OperationId, PlaceId, ScalarType,
        StructuralFieldId, ValueId,
    };
    use terminal_psi::{
        AtomicAccessEvent, AtomicReadModifyWrite, Block, MemoryOrdering, Operation, OperationKind,
        OperationResult, StructuralPathSegment, Terminator, ValueDeclaration,
    };

    use super::super::super::CodecError;
    use super::super::super::wire::{Reader, Writer};
    use super::super::{decode_block, encode_block};

    fn id<T: semantic_vocabulary::PsiSemanticId>(raw: u64) -> T {
        T::new(raw).expect("test ids are nonzero")
    }

    fn block(result: OperationResult, event: AtomicAccessEvent) -> Block {
        Block {
            erased_scalar_formals: Vec::new(),
            erased_proof_formals: Vec::new(),
            structural_parameters: Vec::new(),
            id: id::<BlockId>(1),
            parameters: Vec::new(),
            operations: vec![Operation {
                static_reach_binding: None,
                suspension_crossing: None,
                id: id::<OperationId>(2),
                result,
                kind: OperationKind::AtomicAccess {
                    place: id::<PlaceId>(3),
                    path: Vec::new(),
                    field: id::<StructuralFieldId>(6),
                    event,
                },
            }],
            terminator: Terminator::ReturnUnit {
                edge: id::<EdgeId>(5),
                trivial_affine_discards: Vec::new(),
            },
        }
    }

    fn prior() -> OperationResult {
        OperationResult::Scalar(ValueDeclaration {
            qualifications: Default::default(),
            id: id::<ValueId>(9),
            scalar_type: ScalarType::Integer(
                IntegerType::new(IntegerSign::Unsigned, 32).expect("u32"),
            ),
        })
    }

    fn round_trip(block: &Block) -> Vec<u8> {
        let mut writer = Writer::default();
        encode_block(&mut writer, block).expect("atomic block encodes");
        let bytes = writer.finish();
        let mut reader = Reader::new(&bytes);
        assert_eq!(decode_block(&mut reader).as_ref(), Ok(block));
        assert_eq!(reader.remaining(), 0);
        bytes
    }

    #[test]
    fn every_atomic_event_round_trips_with_its_orderings_and_projection() {
        let fetches = [
            AtomicReadModifyWrite::FetchAdd,
            AtomicReadModifyWrite::FetchSub,
            AtomicReadModifyWrite::FetchAnd,
            AtomicReadModifyWrite::FetchOr,
            AtomicReadModifyWrite::FetchXor,
        ];
        let mut events = vec![
            AtomicAccessEvent::Load {
                ordering: MemoryOrdering::GlobalOrder,
            },
            AtomicAccessEvent::Swap {
                value: id(4),
                ordering: MemoryOrdering::ReceivePublish,
            },
            AtomicAccessEvent::CompareExchange {
                expected: id(4),
                replacement: id(5),
                success: MemoryOrdering::ReceivePublish,
                failure: MemoryOrdering::Receive,
            },
        ];
        events.extend(
            fetches
                .into_iter()
                .map(|operation| AtomicAccessEvent::ReadModifyWrite {
                    operation,
                    operand: id(4),
                    ordering: MemoryOrdering::Publish,
                }),
        );
        for event in events {
            let mut observing = block(prior(), event);
            round_trip(&observing);
            // The projection travels with the event.
            let OperationKind::AtomicAccess { path, .. } = &mut observing.operations[0].kind else {
                unreachable!()
            };
            path.push(StructuralPathSegment::Field(String::from("counter")));
            round_trip(&observing);
        }
    }

    #[test]
    fn a_store_row_uses_exact_stable_fields_and_rejects_unknown_tags() {
        let store = block(
            OperationResult::Unit,
            AtomicAccessEvent::Store {
                value: id(4),
                ordering: MemoryOrdering::NoOrdering,
            },
        );
        let bytes = round_trip(&store);
        assert_eq!(bytes[37], 0, "Unit OperationResult wire tag");
        assert_eq!(bytes[38], 93, "AtomicAccess wire tag");
        assert_eq!(&bytes[39..47], &id::<PlaceId>(3).get().to_le_bytes());
        assert_eq!(&bytes[47..51], &0_u32.to_le_bytes(), "empty carrier path");
        assert_eq!(
            &bytes[51..59],
            &id::<StructuralFieldId>(6).get().to_le_bytes()
        );
        assert_eq!(bytes[59], 2, "store event sub-tag");
        assert_eq!(&bytes[60..68], &id::<ValueId>(4).get().to_le_bytes());
        assert_eq!(bytes[68], 1, "NoOrdering tag");

        let mut unknown_event = bytes.clone();
        unknown_event[59] = 9;
        assert_eq!(
            decode_block(&mut Reader::new(&unknown_event)),
            Err(CodecError::InvalidTag("AtomicAccessEvent", 9)),
        );
        let mut unknown_ordering = bytes.clone();
        unknown_ordering[68] = 6;
        assert_eq!(
            decode_block(&mut Reader::new(&unknown_ordering)),
            Err(CodecError::InvalidTag("MemoryOrdering", 6)),
        );
        // A substituted event tag decodes as a different event only when its
        // fields line up; a load where a store stood consumes the value's
        // first byte as its ordering and leaves trailing bytes behind.
        let mut substituted = bytes;
        substituted[59] = 1;
        assert!(decode_block(&mut Reader::new(&substituted)).is_err());
    }

    #[test]
    fn an_unknown_fetch_operation_rejects() {
        let fetch = block(
            prior(),
            AtomicAccessEvent::ReadModifyWrite {
                operation: AtomicReadModifyWrite::FetchXor,
                operand: id(4),
                ordering: MemoryOrdering::NoOrdering,
            },
        );
        let mut bytes = round_trip(&fetch);
        let tag = bytes
            .windows(2)
            .position(|window| window == [3, 5])
            .expect("read-modify-write sub-tag followed by the xor operation");
        bytes[tag + 1] = 6;
        assert_eq!(
            decode_block(&mut Reader::new(&bytes)),
            Err(CodecError::InvalidTag("AtomicReadModifyWrite", 6)),
        );
    }
}
