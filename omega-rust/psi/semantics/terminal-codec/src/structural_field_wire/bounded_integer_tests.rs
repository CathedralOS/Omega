use super::*;
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue, StructuralFieldId};

fn field(bounds: BoundedIntegerType) -> StructuralFieldDeclaration {
    StructuralFieldDeclaration {
        id: StructuralFieldId::new(1).unwrap(),
        identity: "value".into(),
        relevance: BindingRelevance::Relevant,
        field_type: StructuralFieldType::BoundedInteger(bounds),
    }
}

fn encoded(field: &StructuralFieldDeclaration) -> Vec<u8> {
    let mut writer = Writer::default();
    encode_structural_field(&mut writer, field).unwrap();
    writer.finish()
}

#[test]
fn bounded_integer_field_wire_preserves_full_width_signed_and_unsigned_endpoints() {
    for (sign, minimum, maximum) in [
        (
            IntegerSign::Signed,
            IntegerValue::Signed(i128::MIN),
            IntegerValue::Signed(i128::MAX),
        ),
        (
            IntegerSign::Unsigned,
            IntegerValue::Unsigned(1 << 127),
            IntegerValue::Unsigned(u128::MAX),
        ),
    ] {
        let field = field(
            BoundedIntegerType::new(IntegerType::new(sign, 128).unwrap(), minimum, maximum)
                .unwrap(),
        );
        let bytes = encoded(&field);
        // Eight-byte identity, four-byte string length, five-byte identity,
        // relevance, field tag, carrier/sign, width, then two tagged i128/u128s.
        assert_eq!(bytes.len(), 56);
        assert_eq!(bytes[18], 6);
        assert_eq!(
            decode_structural_field(&mut Reader::new(&bytes)).unwrap(),
            field
        );
    }
}

#[test]
fn bounded_integer_field_wire_rejects_malformed_bounds_and_truncation() {
    let original = encoded(&field(
        BoundedIntegerType::new(
            IntegerType::new(IntegerSign::Signed, 32).unwrap(),
            IntegerValue::Signed(0),
            IntegerValue::Signed(255),
        )
        .unwrap(),
    ));
    assert_eq!(&original[18..23], &[6, 1, 32, 0, 1]);
    assert_eq!(&original[23..39], &0_i128.to_le_bytes());
    assert_eq!(original[39], 1);
    assert_eq!(&original[40..56], &255_i128.to_le_bytes());
    for mutation in [
        "order",
        "minimum carrier",
        "maximum carrier",
        "signedness",
        "address",
        "tag",
    ] {
        let mut bytes = original.clone();
        match mutation {
            "order" => bytes[23..39].copy_from_slice(&256_i128.to_le_bytes()),
            "minimum carrier" => {
                bytes[23..39].copy_from_slice(&(i128::from(i32::MIN) - 1).to_le_bytes())
            }
            "maximum carrier" => {
                bytes[40..56].copy_from_slice(&(i128::from(i32::MAX) + 1).to_le_bytes())
            }
            "signedness" => bytes[39] = 2,
            "address" => bytes[19] = 3,
            "tag" => bytes[18] = 255,
            _ => unreachable!(),
        }
        assert!(
            decode_structural_field(&mut Reader::new(&bytes)).is_err(),
            "{mutation}"
        );
    }
    for length in 0..original.len() {
        assert!(
            decode_structural_field(&mut Reader::new(&original[..length])).is_err(),
            "truncation at {length}"
        );
    }
}
