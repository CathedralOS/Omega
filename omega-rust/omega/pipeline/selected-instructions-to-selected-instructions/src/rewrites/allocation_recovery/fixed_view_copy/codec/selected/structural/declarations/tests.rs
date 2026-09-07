use super::*;

#[test]
fn block_parameter_place_round_trip_retains_exact_tag_block_and_position() {
    for (block, position) in [(1, 0), (2, 0), (1, 1), (u64::MAX, u32::MAX)] {
        let place = StructuralPlaceDeclaration {
            id: PlaceId::new(7).unwrap(),
            kind: StructuralPlaceKind::BlockParameter {
                block: BlockId::new(block).unwrap(),
                position,
            },
        };
        let mut encoded = Vec::new();
        encode_place(&mut encoded, place);
        let mut expected = 7_u64.to_le_bytes().to_vec();
        expected.push(8);
        expected.extend_from_slice(&block.to_le_bytes());
        expected.extend_from_slice(&position.to_le_bytes());
        assert_eq!(encoded, expected);
        let mut cursor = Cursor::new(&encoded);
        assert_eq!(decode_place(&mut cursor).unwrap(), place);
        assert_eq!(cursor.remaining(), 0);
    }
}

#[test]
fn block_parameter_place_rejects_zero_block_and_truncated_coordinates() {
    let mut encoded = Vec::new();
    encode_place(
        &mut encoded,
        StructuralPlaceDeclaration {
            id: PlaceId::new(7).unwrap(),
            kind: StructuralPlaceKind::BlockParameter {
                block: BlockId::new(3).unwrap(),
                position: 0,
            },
        },
    );
    for length in 0..encoded.len() {
        assert!(matches!(
            decode_place(&mut Cursor::new(&encoded[..length])),
            Err(FixedViewCopyDecodeError::Truncated)
        ));
    }
    encoded[9..17].copy_from_slice(&0_u64.to_le_bytes());
    assert!(matches!(
        decode_place(&mut Cursor::new(&encoded)),
        Err(FixedViewCopyDecodeError::InvalidSemanticId(0))
    ));
}
