use super::*;
use semantic_vocabulary::ScalarType;
fn successor() -> SelectedSuccessor {
    SelectedSuccessor {
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
    let golden = "01000000000000000200000003000000000000000100000000000000040000000000000005000000000000000001110000001d0000000000000000000000";
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
    assert_eq!(unused_encoded[45], 0);
    assert_eq!(
        decode_successor(&mut Cursor::new(&unused_encoded)).unwrap(),
        unused
    );
}
#[test]
fn successor_rejects_unknown_transport_and_incomplete_register_pair() {
    let mut encoded = Vec::new();
    encode_successor(&mut encoded, &successor());
    encoded[45] = 2;
    assert_eq!(
        decode_successor(&mut Cursor::new(&encoded)),
        Err(FixedViewCopyDecodeError::UnknownValueTransport(2))
    );
    encoded[45] = 1;
    for length in 46..54 {
        assert_eq!(
            decode_successor(&mut Cursor::new(&encoded[..length])),
            Err(FixedViewCopyDecodeError::Truncated)
        );
    }
}
