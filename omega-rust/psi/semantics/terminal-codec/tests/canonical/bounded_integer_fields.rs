use super::*;
use semantic_vocabulary::BoundedIntegerType;

fn fixture(maximum: i128) -> TerminalModule {
    let mut module = unit_fixture();
    module.structural_types.push(StructuralTypeDeclaration {
        id: structural_type_id(1),
        identity: "ReadResult".into(),
        shape: StructuralTypeShape::Sum {
            cases: vec![StructuralCaseDeclaration {
                id: StructuralCaseId::new(1).unwrap(),
                identity: "Byte".into(),
                fields: vec![StructuralFieldDeclaration {
                    id: structural_field_id(1),
                    identity: "value".into(),
                    relevance: BindingRelevance::Relevant,
                    field_type: StructuralFieldType::BoundedInteger(
                        BoundedIntegerType::new(
                            i32_type(),
                            IntegerValue::Signed(0),
                            IntegerValue::Signed(maximum),
                        )
                        .unwrap(),
                    ),
                }],
            }],
        },
    });
    module
}

#[test]
fn bounded_integer_field_bounds_round_trip_and_bind_semantic_identity() {
    let original = fixture(255);
    let bytes = encode_module(&original).unwrap();
    assert_eq!(&bytes[8..12], &[87, 0, 96, 0]);
    assert_eq!(decode_module(&bytes).unwrap(), original);
    assert_eq!(
        encode_module(&decode_module(&bytes).unwrap()).unwrap(),
        bytes
    );
    let mut raw = original.clone();
    let StructuralTypeShape::Sum { cases } = &mut raw.structural_types[0].shape else {
        panic!("sum")
    };
    cases[0].fields[0].field_type = StructuralFieldType::Scalar(ScalarType::Integer(i32_type()));
    for different in [fixture(254), fixture(256), raw] {
        assert_ne!(
            semantic_fingerprint(&original).unwrap(),
            semantic_fingerprint(&different).unwrap()
        );
    }
    let mut stale = bytes;
    stale[8..10].copy_from_slice(&84_u16.to_le_bytes());
    assert_eq!(
        decode_module(&stale),
        Err(CodecError::UnsupportedFormatMarker(84))
    );
}

#[test]
fn bounded_integer_field_declared_relevance_is_not_erased() {
    let mut module = fixture(255);
    let StructuralTypeShape::Sum { cases } = &mut module.structural_types[0].shape else {
        panic!("sum")
    };
    cases[0].fields[0].relevance = BindingRelevance::Erased;
    assert!(encode_module(&module).is_err());
}

#[test]
fn structural_declaration_encoder_matches_the_existing_module_section() {
    let module = fixture(255);
    let declaration =
        terminal_codec::encode_structural_type_declaration(&module.structural_types[0]).unwrap();
    let module_bytes = encode_module(&module).unwrap();
    // Header, entry machine, three empty scalar-qualification catalog counts,
    // and the structural-declaration count.
    let declaration_start = 8 + 2 + 2 + 8 + 3 * 4 + 4;
    assert_eq!(
        declaration,
        module_bytes[declaration_start..declaration_start + declaration.len()]
    );
}
