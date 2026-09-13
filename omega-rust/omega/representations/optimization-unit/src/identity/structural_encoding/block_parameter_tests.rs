use super::*;

#[test]
fn retained_block_place_identity_binds_block_and_position() {
    let encode = |block, position| {
        let mut bytes = CanonicalBytes::collect();
        encode_place_declaration(
            &mut bytes,
            StructuralPlaceDeclaration {
                id: semantic_vocabulary::PlaceId::new(1).unwrap(),
                kind: StructuralPlaceKind::BlockParameter {
                    block: semantic_vocabulary::BlockId::new(block).unwrap(),
                    position,
                },
            },
        );
        bytes.finish()
    };
    assert_ne!(encode(2, 0), encode(3, 0));
    assert_ne!(encode(2, 0), encode(2, 1));
    assert_eq!(
        encode(2, 0),
        [
            &1_u64.to_le_bytes()[..],
            &[8],
            &2_u64.to_le_bytes(),
            &0_u32.to_le_bytes()
        ]
        .concat()
    );
}
