use super::{
    BindingRelevance, IntegerValue, StructuralCaseDeclaration, StructuralCaseId,
    StructuralFieldDeclaration, StructuralFieldType, StructuralTypeDeclaration,
    StructuralTypeShape, TerminalModule, encode_module, i32_type, structural_field_id,
    structural_type_id, unit_fixture,
};
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
    // Header, entry machine, five empty scalar-qualification catalog counts
    // (domains, sets, coercions, float entry ranges, integer entry ranges), and
    // the structural-declaration count.
    let declaration_start = 8 + 2 + 2 + 8 + 4 * 5 + 4;
    assert_eq!(
        declaration,
        module_bytes[declaration_start..declaration_start + declaration.len()]
    );
}
