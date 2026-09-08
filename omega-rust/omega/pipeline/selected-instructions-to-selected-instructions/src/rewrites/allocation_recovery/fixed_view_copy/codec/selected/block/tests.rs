use super::*;
use semantic_vocabulary::ScalarType;
fn successor() -> SelectedSuccessor {
    SelectedSuccessor {
        role: SelectedSuccessorRole::Semantic,
        psi_edge: EdgeId::new(1).unwrap(),
        block: SelectedBlockId(2),
        source_target: BlockId::new(3).unwrap(),
        bindings: vec![SelectedValueBinding {
            semantic: ValueBinding {
                parameter: ValueId::new(4).unwrap(),
                argument: ValueId::new(5).unwrap(),
                scalar_type: ScalarType::Boolean,
            },
            transport: SelectedValueTransport::Registers {
                argument: VirtualRegisterId(17),
                parameter: VirtualRegisterId(29),
            },
        }],
        fuel: vec![],
    }
}
#[test]
fn successor_register_transport_has_exact_canonical_bytes() {
    let original = successor();
    let mut encoded = Vec::new();
    encode_successor(&mut encoded, &original);
    let golden = "0001000000000000000200000003000000000000000100000000000000040000000000000005000000000000000001110000001d0000000000000000000000";
    let expected = (0..golden.len())
        .step_by(2)
        .map(|offset| u8::from_str_radix(&golden[offset..offset + 2], 16).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(encoded, expected);
    let mut cursor = Cursor::new(&encoded);
    assert_eq!(decode_successor(&mut cursor).unwrap(), original);
    assert_eq!(cursor.remaining(), 0);
    let mut unused = original;
    unused.bindings[0].transport = SelectedValueTransport::Unused;
    let mut unused_encoded = Vec::new();
    encode_successor(&mut unused_encoded, &unused);
    assert_eq!(unused_encoded.len(), encoded.len() - 8);
    assert_eq!(unused_encoded[46], 0);
    assert_eq!(
        decode_successor(&mut Cursor::new(&unused_encoded)).unwrap(),
        unused
    );
}
#[test]
fn successor_rejects_unknown_transport_and_incomplete_register_pair() {
    let mut encoded = Vec::new();
    encode_successor(&mut encoded, &successor());
    encoded[46] = 2;
    assert_eq!(
        decode_successor(&mut Cursor::new(&encoded)),
        Err(FixedViewCopyDecodeError::UnknownValueTransport(2))
    );
    encoded[46] = 1;
    for length in 47..55 {
        assert_eq!(
            decode_successor(&mut Cursor::new(&encoded[..length])),
            Err(FixedViewCopyDecodeError::Truncated)
        );
    }
}

#[test]
fn successor_role_round_trips_and_unknown_roles_reject() {
    let mut original = successor();
    original.role = SelectedSuccessorRole::EdgeTransferContinuation;
    let mut encoded = Vec::new();
    encode_successor(&mut encoded, &original);
    assert_eq!(encoded[0], 1);
    assert_eq!(
        decode_successor(&mut Cursor::new(&encoded)).unwrap(),
        original
    );
    encoded[0] = 2;
    assert_eq!(
        decode_successor(&mut Cursor::new(&encoded)),
        Err(FixedViewCopyDecodeError::UnknownSuccessorRole(2))
    );
}

#[test]
fn implementation_block_origin_round_trips_without_a_fabricated_source_block() {
    let mut block = crate::tests::function_with_operand(register_model::RegisterOperandAccess::Use)
        .blocks
        .remove(0);
    block.origin = SelectedBlockOrigin::EdgeTransfer {
        edge: EdgeId::new(71).unwrap(),
        target: BlockId::new(91).unwrap(),
    };
    let mut encoded = Vec::new();
    encode_block(&mut encoded, &block);
    let mut cursor = Cursor::new(&encoded);
    assert_eq!(decode_block(&mut cursor).unwrap(), block);
    assert_eq!(cursor.remaining(), 0);
    encoded[4] = 2;
    assert_eq!(
        decode_block(&mut Cursor::new(&encoded)),
        Err(FixedViewCopyDecodeError::UnknownBlockOrigin(2))
    );
}
