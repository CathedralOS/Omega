//! Incoming placements and established storage have distinct wire tags.
//! An aggregate home names its exact producer, not an incoming ABI placement or
//! a primitive referent; equal operation numbers must not equate those origins.
use semantic_vocabulary::{BlockId, OperationId, PlaceId};
use target_operations::TargetStructuralArgumentSource;

use super::calling::{decode_placement, encode_placement};
use crate::FixedViewCopyDecodeError;
use crate::rewrites::allocation_recovery::fixed_view_copy::codec::primitives::{Cursor, decode_id};

pub(super) fn encode_argument_source(bytes: &mut Vec<u8>, source: &TargetStructuralArgumentSource) {
    match source {
        TargetStructuralArgumentSource::Placement(placement) => {
            bytes.push(0);
            encode_placement(bytes, placement);
        }
        TargetStructuralArgumentSource::EstablishedByteView { psi_operation } => {
            bytes.push(1);
            bytes.extend_from_slice(&psi_operation.get().to_le_bytes());
        }
        TargetStructuralArgumentSource::BlockParameter { block, place } => {
            bytes.push(2);
            bytes.extend_from_slice(&block.get().to_le_bytes());
            bytes.extend_from_slice(&place.get().to_le_bytes());
        }
        TargetStructuralArgumentSource::EstablishedPrimitiveLocal { psi_operation } => {
            bytes.push(3);
            bytes.extend_from_slice(&psi_operation.get().to_le_bytes());
        }
        TargetStructuralArgumentSource::StructuralHome { psi_operation } => {
            bytes.push(4);
            bytes.extend_from_slice(&psi_operation.get().to_le_bytes());
        }
    }
}

pub(super) fn decode_argument_source(
    cursor: &mut Cursor<'_>,
) -> Result<TargetStructuralArgumentSource, FixedViewCopyDecodeError> {
    match cursor.byte()? {
        0 => Ok(TargetStructuralArgumentSource::Placement(decode_placement(
            cursor,
        )?)),
        1 => Ok(TargetStructuralArgumentSource::EstablishedByteView {
            psi_operation: decode_id(cursor, OperationId::new)?,
        }),
        2 => Ok(TargetStructuralArgumentSource::BlockParameter {
            block: decode_id(cursor, BlockId::new)?,
            place: decode_id(cursor, PlaceId::new)?,
        }),
        3 => Ok(TargetStructuralArgumentSource::EstablishedPrimitiveLocal {
            psi_operation: decode_id(cursor, OperationId::new)?,
        }),
        4 => Ok(TargetStructuralArgumentSource::StructuralHome {
            psi_operation: decode_id(cursor, OperationId::new)?,
        }),
        tag => Err(FixedViewCopyDecodeError::UnknownOption(tag)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use calling_conventions::{ValueClass, ValuePlacement, ValueShape};

    #[test]
    fn argument_source_round_trip_preserves_origin_and_establishment() {
        for source in [
            TargetStructuralArgumentSource::StructuralHome {
                psi_operation: OperationId::new(313).unwrap(),
            },
            TargetStructuralArgumentSource::EstablishedPrimitiveLocal {
                psi_operation: OperationId::new(313).unwrap(),
            },
            TargetStructuralArgumentSource::BlockParameter {
                block: BlockId::new(211).unwrap(),
                place: PlaceId::new(213).unwrap(),
            },
            TargetStructuralArgumentSource::Placement(ValuePlacement {
                shape: ValueShape {
                    byte_size: 16,
                    alignment: 8,
                    class: ValueClass::BorrowedReference,
                },
                locations: Vec::new(),
            }),
            TargetStructuralArgumentSource::EstablishedByteView {
                psi_operation: OperationId::new(313).unwrap(),
            },
        ] {
            let mut encoded = Vec::new();
            encode_argument_source(&mut encoded, &source);
            let mut cursor = Cursor::new(&encoded);
            assert_eq!(decode_argument_source(&mut cursor).unwrap(), source);
            assert_eq!(cursor.remaining(), 0);
            for end in 0..encoded.len() {
                assert!(decode_argument_source(&mut Cursor::new(&encoded[..end])).is_err());
            }
        }
    }

    #[test]
    fn argument_source_rejects_unknown_kind_and_absent_establishment() {
        assert!(decode_argument_source(&mut Cursor::new(&[5])).is_err());
        assert!(decode_argument_source(&mut Cursor::new(&[4, 0, 0, 0, 0, 0, 0, 0, 0])).is_err());
        assert!(decode_argument_source(&mut Cursor::new(&[3, 0, 0, 0, 0, 0, 0, 0, 0])).is_err());
        assert!(decode_argument_source(&mut Cursor::new(&[2])).is_err());
        assert!(decode_argument_source(&mut Cursor::new(&[1, 0, 0, 0, 0, 0, 0, 0, 0])).is_err());
        let mut first = Vec::new();
        let mut second = Vec::new();
        for (bytes, operation) in [(&mut first, 313), (&mut second, 317)] {
            encode_argument_source(
                bytes,
                &TargetStructuralArgumentSource::EstablishedByteView {
                    psi_operation: OperationId::new(operation).unwrap(),
                },
            );
        }
        assert_ne!(first, second);
        let mut local = Vec::new();
        encode_argument_source(
            &mut local,
            &TargetStructuralArgumentSource::EstablishedPrimitiveLocal {
                psi_operation: OperationId::new(313).unwrap(),
            },
        );
        assert_ne!(
            first, local,
            "identical producer ID cannot equate a view and primitive local"
        );
        let mut aggregate = Vec::new();
        encode_argument_source(
            &mut aggregate,
            &TargetStructuralArgumentSource::StructuralHome {
                psi_operation: OperationId::new(313).unwrap(),
            },
        );
        assert_ne!(aggregate, local);
        assert_ne!(aggregate, first);
    }
}
