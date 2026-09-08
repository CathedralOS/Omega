use super::*;

#[test]
fn decoding_rejects_nonexistent_fields_in_write_only_contracts() {
    for is_requirement in [true, false] {
        let mut module = unit_fixture();
        // This identity is unique to the declaration and the two contract terms.
        let field_identity = 0x7182_93a4_b5c6_d7e8_u64;
        let root = place_id(930);
        let structural_type = structural_type_id(930);
        let integer_type = IntegerType::new(IntegerSign::Signed, 32).unwrap();
        module.structural_types.push(StructuralTypeDeclaration {
            id: structural_type,
            identity: "Input".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![StructuralFieldDeclaration {
                    id: structural_field_id(field_identity),
                    identity: "value".into(),
                    field_type: StructuralFieldType::Scalar(ScalarType::Integer(integer_type)),
                    relevance: BindingRelevance::Relevant,
                }],
            },
        });
        let machine = &mut module.machines[0];
        machine
            .structural_parameters
            .push(StructuralParameterDeclaration {
                place: root,
                position: 0,
                is_self: false,
                structural_type,
                multiplicity: StructuralMultiplicity::Unrestricted,
                access: StructuralAccess::WriteOnlyBorrow,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
            });
        machine.structural_places.push(StructuralPlaceDeclaration {
            id: root,
            kind: StructuralPlaceKind::Parameter {
                position: 0,
                is_self: false,
            },
        });
        let field = ScalarTerm::integer_field_path(
            root,
            vec![CanonicalStructuralPathSegment::Field(structural_field_id(
                field_identity,
            ))],
            integer_type,
        );
        let proposition = Proposition::Equal(field.clone(), field);
        if is_requirement {
            machine.contract.requires.push(proposition);
        } else {
            machine.contract.ensures.push(ContractClause {
                obligation: obligation_id(930),
                proposition,
            });
        }
        let mut encoded = encode_module(&module).expect("well-formed unused field clause encodes");
        assert_eq!(decode_module(&encoded), Ok(module));
        let occurrences = encoded
            .windows(8)
            .enumerate()
            .filter_map(|(offset, bytes)| (bytes == field_identity.to_le_bytes()).then_some(offset))
            .collect::<Vec<_>>();
        assert_eq!(occurrences.len(), 3, "one declaration and two field terms");
        // Preserve the declaration; change both sides of the tautology to an
        // undeclared identity so the public decoder must reconstruct the path.
        for offset in &occurrences[1..] {
            encoded[*offset..*offset + 8].copy_from_slice(&(field_identity + 1).to_le_bytes());
        }
        assert!(matches!(
            decode_module(&encoded),
            Err(CodecError::InvalidModule(
                terminal_verifier::ModuleError::InvalidIntegerFieldTerm { .. }
            ))
        ));
    }
}
