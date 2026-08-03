use psi_core::{
    BlockId, ClaimId, ContentAlgebra, ContentAlgebraKind, ContentConservation, ContentDomainId,
    ContentPlaceSegment, ContentPlaceVersion, ContentProjectionIdentity, ContentStructuralPlace,
    ContentTerm, ContractId, EdgeId, IntegerSign, IntegerType, IntegerValue, MachineId,
    ObligationId, OperationId, PlaceId, Proposition, PropositionId, ScalarTerm, ScalarType,
    StructuralPlaceKind, ValueId,
};
use psi_terminal::{
    Block, ClaimContentProjection, ContentIdentityReshuffle, ContractClause, MachineContract,
    Operation, OperationKind, SemanticVersion, StructuralPlaceDeclaration, TerminalMachine,
    TerminalModule, Terminator, ValueDeclaration,
};
use psi_terminal_codec::{
    CodecError, decode_module, encode_module, migrate_module_to_current, semantic_fingerprint,
    terminal_psi_identity,
};

#[test]
fn current_vocabulary_has_one_stable_canonical_encoding_and_identity() {
    let module = fixture();
    let bytes = encode_module(&module).expect("fixture should encode");

    assert_eq!(&bytes[..8], b"PSITERM\0");
    assert_eq!(&bytes[8..10], 1_u16.to_le_bytes());
    assert_eq!(decode_module(&bytes), Ok(module.clone()));
    assert_eq!(encode_module(&decode_module(&bytes).unwrap()), Ok(bytes));

    let identity = terminal_psi_identity(&module).expect("fixture should have an identity");
    assert_eq!(identity.semantic_version, SemanticVersion::CURRENT);
    assert_eq!(
        identity.program_fingerprint.to_string(),
        "77ed3e95229377c7f9a5eb6f6cff7f19802a8d05e0f7c9170d46b8c1d6f554d8"
    );
    assert_eq!(
        identity.program_fingerprint,
        semantic_fingerprint(&module).unwrap()
    );
}

#[test]
fn v9_content_conservation_has_stable_canonical_bytes() {
    let module = content_conservation_fixture(SemanticVersion::V9);
    let bytes = encode_module(&module).expect("v9 content module should encode");

    assert_eq!(decode_module(&bytes), Ok(module.clone()));
    assert_eq!(encode_module(&decode_module(&bytes).unwrap()), Ok(bytes));
    assert_eq!(
        semantic_fingerprint(&module).unwrap().to_string(),
        "6ac425701a2d15d0531d97922f63bb85118a267d8a4d72c6f2bc787668540986"
    );
}

#[test]
fn v10_identity_reshuffle_has_stable_canonical_bytes() {
    let module = identity_reshuffle_fixture(SemanticVersion::V10);
    let bytes = encode_module(&module).expect("v10 identity reshuffle should encode");

    assert_eq!(decode_module(&bytes), Ok(module.clone()));
    assert_eq!(encode_module(&decode_module(&bytes).unwrap()), Ok(bytes));
    assert_eq!(
        semantic_fingerprint(&module).unwrap().to_string(),
        "fdc2ff2c3593417f7faafa02d171028f2d417b1f248694596fc015005ce9f28e"
    );
}

#[test]
fn identity_reshuffle_encoding_is_v10_and_canonically_ordered() {
    let old = identity_reshuffle_fixture(SemanticVersion::V9);
    assert!(matches!(
        encode_module(&old),
        Err(CodecError::InvalidModule(
            psi_terminal_verifier::ModuleError::ContentIdentityReshufflesRequireSemanticVersion {
                required: SemanticVersion::V10,
                actual: SemanticVersion::V9,
                ..
            }
        ))
    ));

    let mut projections = identity_reshuffle_fixture(SemanticVersion::V10);
    projections.machines[0].content_identity_reshuffles[0]
        .projections
        .swap(0, 1);
    assert_eq!(
        encode_module(&projections),
        Err(CodecError::NonCanonicalOrder(
            "claim content projections by identity and algebra"
        ))
    );
}

#[test]
fn v8_cannot_claim_structural_places_or_content_conservation() {
    let module = content_conservation_fixture(SemanticVersion::V8);
    assert!(matches!(
        encode_module(&module),
        Err(CodecError::InvalidModule(
            psi_terminal_verifier::ModuleError::StructuralPlacesRequireSemanticVersion {
                required: SemanticVersion::V9,
                actual: SemanticVersion::V8,
                ..
            }
        ))
    ));
}

#[test]
fn archived_v1_bytes_keep_their_original_identity_and_migrate_explicitly() {
    let mut archived = fixture();
    archived.semantic_version = SemanticVersion::V1;
    let archived_bytes = encode_module(&archived).expect("v1 remains encodable");
    let archived_fingerprint = semantic_fingerprint(&archived).unwrap();

    assert_eq!(decode_module(&archived_bytes), Ok(archived.clone()));
    assert_eq!(
        archived_fingerprint.to_string(),
        "bcb56768a31b4ddde394676892c42d702f18bad3a563457cd36fe73912f7e26f"
    );

    let migrated = migrate_module_to_current(&archived).expect("v1 migrates to current");
    assert_eq!(migrated.semantic_version, SemanticVersion::CURRENT);
    assert_eq!(migrated.entry, archived.entry);
    assert_eq!(migrated.machines, archived.machines);
    assert_ne!(
        semantic_fingerprint(&migrated).unwrap(),
        archived_fingerprint
    );
}

#[test]
fn archived_v2_bytes_keep_their_original_identity_and_migrate_explicitly() {
    let archived = boolean_fixture(SemanticVersion::V2);
    let archived_bytes = encode_module(&archived).expect("v2 remains encodable");
    let archived_fingerprint = semantic_fingerprint(&archived).unwrap();

    assert_eq!(decode_module(&archived_bytes), Ok(archived.clone()));
    assert_eq!(
        archived_fingerprint.to_string(),
        "5a4b9e8eb4fc5e3a5c90c635ea0da2b9d312d65fddaaa022fda4e300ff5f6fde"
    );

    let migrated = migrate_module_to_current(&archived).expect("v2 migrates to current");
    assert_eq!(migrated.semantic_version, SemanticVersion::CURRENT);
    assert_eq!(migrated.entry, archived.entry);
    assert_eq!(migrated.machines, archived.machines);
    assert_ne!(
        semantic_fingerprint(&migrated).unwrap(),
        archived_fingerprint
    );
}

#[test]
fn v1_cannot_claim_the_v2_boolean_operation() {
    let module = boolean_fixture(SemanticVersion::V1);

    assert!(matches!(
        encode_module(&module),
        Err(CodecError::InvalidModule(
            psi_terminal_verifier::ModuleError::OperationRequiresSemanticVersion {
                required: SemanticVersion::V2,
                actual: SemanticVersion::V1,
                ..
            }
        ))
    ));
}

#[test]
fn v2_boolean_operation_has_stable_canonical_bytes() {
    let module = boolean_fixture(SemanticVersion::V2);
    let bytes = encode_module(&module).expect("v2 Boolean module should encode");

    assert_eq!(decode_module(&bytes), Ok(module.clone()));
    assert_eq!(
        semantic_fingerprint(&module).unwrap().to_string(),
        "5a4b9e8eb4fc5e3a5c90c635ea0da2b9d312d65fddaaa022fda4e300ff5f6fde"
    );
}

#[test]
fn v2_cannot_claim_the_v3_wrapping_add_operation() {
    let module = wrapping_add_fixture(SemanticVersion::V2);

    assert!(matches!(
        encode_module(&module),
        Err(CodecError::InvalidModule(
            psi_terminal_verifier::ModuleError::OperationRequiresSemanticVersion {
                required: SemanticVersion::V3,
                actual: SemanticVersion::V2,
                ..
            }
        ))
    ));
}

#[test]
fn v2_cannot_claim_the_v3_wrapping_add_proposition_term() {
    let mut module = boolean_fixture(SemanticVersion::V2);
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let literal = |value| ScalarTerm::integer(integer, IntegerValue::Unsigned(value)).unwrap();
    let sum = ScalarTerm::wrapping_integer_add(integer, literal(200), literal(100)).unwrap();
    module.machines[0]
        .contract
        .requires
        .push(Proposition::Equal(literal(44), sum));

    assert!(matches!(
        encode_module(&module),
        Err(CodecError::InvalidModule(
            psi_terminal_verifier::ModuleError::PropositionRequiresSemanticVersion {
                required: SemanticVersion::V3,
                actual: SemanticVersion::V2,
                ..
            }
        ))
    ));
}

#[test]
fn v3_wrapping_add_has_stable_canonical_bytes() {
    let module = wrapping_add_fixture(SemanticVersion::V3);
    let bytes = encode_module(&module).expect("v3 wrapping-add module should encode");

    assert_eq!(decode_module(&bytes), Ok(module.clone()));
    assert_eq!(
        semantic_fingerprint(&module).unwrap().to_string(),
        "2c0e2777161b1a8840bb5f63e5b96bc6ecf0831bcbde76e4d218104f105e580d"
    );
}

#[test]
fn archived_v3_bytes_keep_their_original_identity_and_migrate_explicitly() {
    let archived = wrapping_add_fixture(SemanticVersion::V3);
    let archived_fingerprint = semantic_fingerprint(&archived).unwrap();

    assert_eq!(
        archived_fingerprint.to_string(),
        "2c0e2777161b1a8840bb5f63e5b96bc6ecf0831bcbde76e4d218104f105e580d"
    );
    let migrated = migrate_module_to_current(&archived).expect("v3 migrates to current");
    assert_eq!(migrated.semantic_version, SemanticVersion::CURRENT);
    assert_eq!(migrated.entry, archived.entry);
    assert_eq!(migrated.machines, archived.machines);
    assert_ne!(
        semantic_fingerprint(&migrated).unwrap(),
        archived_fingerprint
    );
}

#[test]
fn v3_cannot_claim_the_v4_saturating_add_operation() {
    let module = saturating_add_fixture(SemanticVersion::V3);

    assert!(matches!(
        encode_module(&module),
        Err(CodecError::InvalidModule(
            psi_terminal_verifier::ModuleError::OperationRequiresSemanticVersion {
                required: SemanticVersion::V4,
                actual: SemanticVersion::V3,
                ..
            }
        ))
    ));
}

#[test]
fn v3_cannot_claim_the_v4_saturating_add_proposition_term() {
    let mut module = wrapping_add_fixture(SemanticVersion::V3);
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let literal = |value| ScalarTerm::integer(integer, IntegerValue::Unsigned(value)).unwrap();
    let sum = ScalarTerm::saturating_integer_add(integer, literal(200), literal(100)).unwrap();
    module.machines[0]
        .contract
        .requires
        .push(Proposition::Equal(literal(255), sum));

    assert!(matches!(
        encode_module(&module),
        Err(CodecError::InvalidModule(
            psi_terminal_verifier::ModuleError::PropositionRequiresSemanticVersion {
                required: SemanticVersion::V4,
                actual: SemanticVersion::V3,
                ..
            }
        ))
    ));
}

#[test]
fn v4_saturating_add_has_stable_canonical_bytes() {
    let module = saturating_add_fixture(SemanticVersion::V4);
    let bytes = encode_module(&module).expect("v4 saturating-add module should encode");

    assert_eq!(decode_module(&bytes), Ok(module.clone()));
    assert_eq!(
        semantic_fingerprint(&module).unwrap().to_string(),
        "6f1bbce42a5a3e4632b99fc5bc1e528d4d9a18761bf12aa696be2a6073cda274"
    );
}

#[test]
fn archived_v4_bytes_keep_their_original_identity_and_migrate_explicitly() {
    let archived = saturating_add_fixture(SemanticVersion::V4);
    let archived_fingerprint = semantic_fingerprint(&archived).unwrap();

    assert_eq!(
        archived_fingerprint.to_string(),
        "6f1bbce42a5a3e4632b99fc5bc1e528d4d9a18761bf12aa696be2a6073cda274"
    );
    let migrated = migrate_module_to_current(&archived).expect("v4 migrates to current");
    assert_eq!(migrated.semantic_version, SemanticVersion::CURRENT);
    assert_eq!(migrated.entry, archived.entry);
    assert_eq!(migrated.machines, archived.machines);
    assert_ne!(
        semantic_fingerprint(&migrated).unwrap(),
        archived_fingerprint
    );
}

#[test]
fn v4_cannot_claim_the_v5_wrapping_subtract_operation() {
    let module = wrapping_subtract_fixture(SemanticVersion::V4);

    assert!(matches!(
        encode_module(&module),
        Err(CodecError::InvalidModule(
            psi_terminal_verifier::ModuleError::OperationRequiresSemanticVersion {
                required: SemanticVersion::V5,
                actual: SemanticVersion::V4,
                ..
            }
        ))
    ));
}

#[test]
fn v4_cannot_claim_the_v5_wrapping_subtract_proposition_term() {
    let mut module = saturating_add_fixture(SemanticVersion::V4);
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let literal = |value| ScalarTerm::integer(integer, IntegerValue::Unsigned(value)).unwrap();
    let difference =
        ScalarTerm::wrapping_integer_subtract(integer, literal(5), literal(10)).unwrap();
    module.machines[0]
        .contract
        .requires
        .push(Proposition::Equal(literal(251), difference));

    assert!(matches!(
        encode_module(&module),
        Err(CodecError::InvalidModule(
            psi_terminal_verifier::ModuleError::PropositionRequiresSemanticVersion {
                required: SemanticVersion::V5,
                actual: SemanticVersion::V4,
                ..
            }
        ))
    ));
}

#[test]
fn v5_wrapping_subtract_has_stable_canonical_bytes() {
    let module = wrapping_subtract_fixture(SemanticVersion::V5);
    let bytes = encode_module(&module).expect("v5 wrapping-subtract module should encode");

    assert_eq!(decode_module(&bytes), Ok(module.clone()));
    assert_eq!(
        semantic_fingerprint(&module).unwrap().to_string(),
        "3ea582aeca24f9003ec33687bdc5ae403de933ede6005e33141a024ff069f965"
    );
}

#[test]
fn archived_v5_bytes_keep_their_original_identity_and_migrate_explicitly() {
    let archived = wrapping_subtract_fixture(SemanticVersion::V5);
    let archived_fingerprint = semantic_fingerprint(&archived).unwrap();

    assert_eq!(
        archived_fingerprint.to_string(),
        "3ea582aeca24f9003ec33687bdc5ae403de933ede6005e33141a024ff069f965"
    );
    let migrated = migrate_module_to_current(&archived).expect("v5 migrates to current");
    assert_eq!(migrated.semantic_version, SemanticVersion::CURRENT);
    assert_eq!(migrated.entry, archived.entry);
    assert_eq!(migrated.machines, archived.machines);
    assert_ne!(
        semantic_fingerprint(&migrated).unwrap(),
        archived_fingerprint
    );
}

#[test]
fn v5_cannot_claim_the_v6_saturating_subtract_operation() {
    let module = saturating_subtract_fixture(SemanticVersion::V5);

    assert!(matches!(
        encode_module(&module),
        Err(CodecError::InvalidModule(
            psi_terminal_verifier::ModuleError::OperationRequiresSemanticVersion {
                required: SemanticVersion::V6,
                actual: SemanticVersion::V5,
                ..
            }
        ))
    ));
}

#[test]
fn v5_cannot_claim_the_v6_saturating_subtract_proposition_term() {
    let mut module = wrapping_subtract_fixture(SemanticVersion::V5);
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let literal = |value| ScalarTerm::integer(integer, IntegerValue::Unsigned(value)).unwrap();
    let difference =
        ScalarTerm::saturating_integer_subtract(integer, literal(5), literal(10)).unwrap();
    module.machines[0]
        .contract
        .requires
        .push(Proposition::Equal(literal(0), difference));

    assert!(matches!(
        encode_module(&module),
        Err(CodecError::InvalidModule(
            psi_terminal_verifier::ModuleError::PropositionRequiresSemanticVersion {
                required: SemanticVersion::V6,
                actual: SemanticVersion::V5,
                ..
            }
        ))
    ));
}

#[test]
fn v6_saturating_subtract_has_stable_canonical_bytes() {
    let module = saturating_subtract_fixture(SemanticVersion::V6);
    let bytes = encode_module(&module).expect("v6 saturating-subtract module should encode");

    assert_eq!(decode_module(&bytes), Ok(module.clone()));
    assert_eq!(
        semantic_fingerprint(&module).unwrap().to_string(),
        "c7eb235760b941c502c19090b929482d149a35db3b723396dbe0f9d0a534c9a0"
    );
}

#[test]
fn archived_v6_bytes_keep_their_original_identity_and_migrate_explicitly() {
    let archived = saturating_subtract_fixture(SemanticVersion::V6);
    let archived_fingerprint = semantic_fingerprint(&archived).unwrap();

    assert_eq!(
        archived_fingerprint.to_string(),
        "c7eb235760b941c502c19090b929482d149a35db3b723396dbe0f9d0a534c9a0"
    );
    let migrated = migrate_module_to_current(&archived).expect("v6 migrates to current");
    assert_eq!(migrated.semantic_version, SemanticVersion::CURRENT);
    assert_eq!(migrated.entry, archived.entry);
    assert_eq!(migrated.machines, archived.machines);
    assert_ne!(
        semantic_fingerprint(&migrated).unwrap(),
        archived_fingerprint
    );
}

#[test]
fn v6_cannot_claim_the_v7_wrapping_multiply_operation() {
    let module = wrapping_multiply_fixture(SemanticVersion::V6);

    assert!(matches!(
        encode_module(&module),
        Err(CodecError::InvalidModule(
            psi_terminal_verifier::ModuleError::OperationRequiresSemanticVersion {
                required: SemanticVersion::V7,
                actual: SemanticVersion::V6,
                ..
            }
        ))
    ));
}

#[test]
fn v6_cannot_claim_the_v7_wrapping_multiply_proposition_term() {
    let mut module = saturating_subtract_fixture(SemanticVersion::V6);
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let literal = |value| ScalarTerm::integer(integer, IntegerValue::Unsigned(value)).unwrap();
    let product = ScalarTerm::wrapping_integer_multiply(integer, literal(20), literal(13)).unwrap();
    module.machines[0]
        .contract
        .requires
        .push(Proposition::Equal(literal(4), product));

    assert!(matches!(
        encode_module(&module),
        Err(CodecError::InvalidModule(
            psi_terminal_verifier::ModuleError::PropositionRequiresSemanticVersion {
                required: SemanticVersion::V7,
                actual: SemanticVersion::V6,
                ..
            }
        ))
    ));
}

#[test]
fn v7_wrapping_multiply_has_stable_canonical_bytes() {
    let module = wrapping_multiply_fixture(SemanticVersion::V7);
    let bytes = encode_module(&module).expect("v7 wrapping-multiply module should encode");

    assert_eq!(decode_module(&bytes), Ok(module.clone()));
    assert_eq!(
        semantic_fingerprint(&module).unwrap().to_string(),
        "ba5decde144648c31baff22f9d536591cd1c2f9db39470e02e948ff14db53132"
    );
}

#[test]
fn archived_v7_bytes_keep_their_original_identity_and_migrate_explicitly() {
    let archived = wrapping_multiply_fixture(SemanticVersion::V7);
    let archived_fingerprint = semantic_fingerprint(&archived).unwrap();

    assert_eq!(
        archived_fingerprint.to_string(),
        "ba5decde144648c31baff22f9d536591cd1c2f9db39470e02e948ff14db53132"
    );
    let migrated = migrate_module_to_current(&archived).expect("v7 migrates to current");
    assert_eq!(migrated.semantic_version, SemanticVersion::CURRENT);
    assert_eq!(migrated.entry, archived.entry);
    assert_eq!(migrated.machines, archived.machines);
    assert_ne!(
        semantic_fingerprint(&migrated).unwrap(),
        archived_fingerprint
    );
}

#[test]
fn v7_cannot_claim_the_v8_saturating_multiply_operation() {
    let module = saturating_multiply_fixture(SemanticVersion::V7);

    assert!(matches!(
        encode_module(&module),
        Err(CodecError::InvalidModule(
            psi_terminal_verifier::ModuleError::OperationRequiresSemanticVersion {
                required: SemanticVersion::V8,
                actual: SemanticVersion::V7,
                ..
            }
        ))
    ));
}

#[test]
fn v7_cannot_claim_the_v8_saturating_multiply_proposition_term() {
    let mut module = wrapping_multiply_fixture(SemanticVersion::V7);
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let literal = |value| ScalarTerm::integer(integer, IntegerValue::Unsigned(value)).unwrap();
    let product =
        ScalarTerm::saturating_integer_multiply(integer, literal(20), literal(13)).unwrap();
    module.machines[0]
        .contract
        .requires
        .push(Proposition::Equal(literal(255), product));

    assert!(matches!(
        encode_module(&module),
        Err(CodecError::InvalidModule(
            psi_terminal_verifier::ModuleError::PropositionRequiresSemanticVersion {
                required: SemanticVersion::V8,
                actual: SemanticVersion::V7,
                ..
            }
        ))
    ));
}

#[test]
fn v8_saturating_multiply_has_stable_canonical_bytes() {
    let module = saturating_multiply_fixture(SemanticVersion::V8);
    let bytes = encode_module(&module).expect("v8 saturating-multiply module should encode");

    assert_eq!(decode_module(&bytes), Ok(module.clone()));
    assert_eq!(
        semantic_fingerprint(&module).unwrap().to_string(),
        "14c981af3621e87b8e7763821197cf7ea11c08e44dea712217ae1e45f407d1a6"
    );
}

#[test]
fn semantic_mutation_changes_the_program_fingerprint() {
    let original = fixture();
    let mut changed = original.clone();
    changed.machines[0].blocks[1].operations[0].kind = OperationKind::IntegerConstant {
        value: IntegerValue::Signed(-6),
    };

    assert_ne!(
        semantic_fingerprint(&original).unwrap(),
        semantic_fingerprint(&changed).unwrap()
    );
}

#[test]
fn decoder_rejects_noncanonical_or_ambiguous_bytes() {
    let bytes = encode_module(&fixture()).unwrap();

    let mut reordered_requirements = bytes.clone();
    let contract_prefix = [
        1, 0, 0, 0, 0, 0, 0, 0, // ContractId(1)
        8, 0, 0, 0, // eight requirements
        1, 2, 3, // Truth, Falsehood, Atom
    ];
    let contract_offset = reordered_requirements
        .windows(contract_prefix.len())
        .position(|window| window == contract_prefix)
        .expect("fixture contract prefix should be unique");
    reordered_requirements.swap(contract_offset + 12, contract_offset + 13);
    assert_eq!(
        decode_module(&reordered_requirements),
        Err(CodecError::NonCanonicalOrder("requires propositions"))
    );

    let mut trailing = bytes.clone();
    trailing.push(0);
    assert_eq!(decode_module(&trailing), Err(CodecError::TrailingBytes(1)));

    let mut future_format = bytes.clone();
    future_format[8..10].copy_from_slice(&2_u16.to_le_bytes());
    assert_eq!(
        decode_module(&future_format),
        Err(CodecError::UnsupportedFormatVersion(2))
    );

    assert_eq!(
        decode_module(&bytes[..bytes.len() - 1]),
        Err(CodecError::UnexpectedEnd)
    );
}

#[test]
fn encoder_refuses_noncanonical_semantic_ordering_and_forms() {
    let mut blocks = fixture();
    blocks.machines[0].blocks.swap(0, 1);
    assert_eq!(
        encode_module(&blocks),
        Err(CodecError::NonCanonicalOrder("blocks by BlockId"))
    );

    let mut requirements = fixture();
    requirements.machines[0].contract.requires.swap(0, 1);
    assert_eq!(
        encode_module(&requirements),
        Err(CodecError::NonCanonicalOrder("requires propositions"))
    );

    let mut equality = fixture();
    equality.machines[0].contract.ensures[0].proposition = Proposition::Equal(
        ScalarTerm::integer(i32_type(), IntegerValue::Signed(-7)).unwrap(),
        ScalarTerm::value(value_id(4), ScalarType::Integer(i32_type())),
    );
    assert_eq!(
        encode_module(&equality),
        Err(CodecError::NonCanonicalOrder("equality operands"))
    );

    let mut conjunction = fixture();
    conjunction.machines[0].contract.ensures[0].proposition = Proposition::Conjunction(vec![
        Proposition::Truth,
        Proposition::Conjunction(vec![Proposition::Truth, Proposition::Falsehood]),
    ]);
    assert_eq!(
        encode_module(&conjunction),
        Err(CodecError::NestedConjunction)
    );
}

#[test]
fn proposition_nesting_has_a_total_v1_bound() {
    let mut module = fixture();
    let mut proposition = Proposition::Truth;
    for _ in 0..257 {
        proposition = Proposition::Implication {
            premise: Box::new(Proposition::Truth),
            conclusion: Box::new(proposition),
        };
    }
    module.machines[0].contract.ensures[0].proposition = proposition;

    assert_eq!(
        encode_module(&module),
        Err(CodecError::PropositionNestingTooDeep)
    );
}

#[test]
fn scalar_term_nesting_has_a_total_bound() {
    let mut module = fixture();
    let integer = i32_type();
    let literal = || ScalarTerm::integer(integer, IntegerValue::Signed(1)).unwrap();
    let mut term = literal();
    for _ in 0..257 {
        term = ScalarTerm::wrapping_integer_add(integer, term, literal()).unwrap();
    }
    module.machines[0].contract.ensures[0].proposition = Proposition::Equal(literal(), term);

    assert_eq!(
        encode_module(&module),
        Err(CodecError::ScalarTermNestingTooDeep)
    );
}

fn fixture() -> TerminalModule {
    let integer = i32_type();
    let scalar_type = ScalarType::Integer(integer);
    let signed = |value| ScalarTerm::integer(integer, IntegerValue::Signed(value)).unwrap();
    let unsigned_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let unsigned =
        |value| ScalarTerm::integer(unsigned_type, IntegerValue::Unsigned(value)).unwrap();

    TerminalModule {
        semantic_version: SemanticVersion::CURRENT,
        entry: machine_id(1),
        machines: vec![TerminalMachine {
            id: machine_id(1),
            parameters: vec![ValueDeclaration {
                id: value_id(5),
                scalar_type: ScalarType::Boolean,
            }],
            result: ValueDeclaration {
                id: value_id(4),
                scalar_type,
            },
            structural_places: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            entry: block_id(1),
            blocks: vec![
                Block {
                    id: block_id(1),
                    parameters: Vec::new(),
                    operations: vec![Operation {
                        id: operation_id(1),
                        result: ValueDeclaration {
                            id: value_id(1),
                            scalar_type,
                        },
                        kind: OperationKind::IntegerConstant {
                            value: IntegerValue::Signed(-7),
                        },
                    }],
                    terminator: Terminator::Jump {
                        edge: edge_id(1),
                        target: block_id(2),
                        arguments: vec![value_id(1)],
                    },
                },
                Block {
                    id: block_id(2),
                    parameters: vec![ValueDeclaration {
                        id: value_id(2),
                        scalar_type,
                    }],
                    operations: vec![Operation {
                        id: operation_id(2),
                        result: ValueDeclaration {
                            id: value_id(3),
                            scalar_type,
                        },
                        kind: OperationKind::IntegerConstant {
                            value: IntegerValue::Signed(-7),
                        },
                    }],
                    terminator: Terminator::Return {
                        edge: edge_id(2),
                        value: value_id(3),
                    },
                },
            ],
            contract: MachineContract {
                id: contract_id(1),
                requires: vec![
                    Proposition::Truth,
                    Proposition::Falsehood,
                    Proposition::Atom(proposition_id(1)),
                    Proposition::Equal(ScalarTerm::boolean(false), ScalarTerm::boolean(true)),
                    Proposition::LessThan(signed(-8), signed(-7)),
                    Proposition::LessOrEqual(unsigned(1), unsigned(2)),
                    Proposition::Conjunction(vec![Proposition::Truth, Proposition::Falsehood]),
                    Proposition::Implication {
                        premise: Box::new(Proposition::Truth),
                        conclusion: Box::new(Proposition::Atom(proposition_id(2))),
                    },
                ],
                ensures: vec![
                    ContractClause {
                        obligation: obligation_id(1),
                        proposition: Proposition::Equal(
                            ScalarTerm::value(value_id(4), scalar_type),
                            signed(-7),
                        ),
                    },
                    ContractClause {
                        obligation: obligation_id(2),
                        proposition: Proposition::Truth,
                    },
                ],
            },
        }],
    }
}

fn boolean_fixture(semantic_version: SemanticVersion) -> TerminalModule {
    TerminalModule {
        semantic_version,
        entry: machine_id(10),
        machines: vec![TerminalMachine {
            id: machine_id(10),
            parameters: Vec::new(),
            result: ValueDeclaration {
                id: value_id(11),
                scalar_type: ScalarType::Boolean,
            },
            structural_places: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            entry: block_id(10),
            blocks: vec![Block {
                id: block_id(10),
                parameters: Vec::new(),
                operations: vec![Operation {
                    id: operation_id(10),
                    result: ValueDeclaration {
                        id: value_id(10),
                        scalar_type: ScalarType::Boolean,
                    },
                    kind: OperationKind::BooleanConstant { value: true },
                }],
                terminator: Terminator::Return {
                    edge: edge_id(10),
                    value: value_id(10),
                },
            }],
            contract: MachineContract {
                id: contract_id(10),
                requires: Vec::new(),
                ensures: Vec::new(),
            },
        }],
    }
}

fn content_conservation_fixture(semantic_version: SemanticVersion) -> TerminalModule {
    let parameter_place = place_id(1);
    let result_place = place_id(2);
    let projection = ContentProjectionIdentity {
        domain: ContentDomainId::new(7).expect("domain"),
        projection_fingerprint: 0x1234,
    };
    let projected = |version, root, field: Option<&str>| ContentTerm::Projection {
        projection,
        subject: ContentStructuralPlace {
            version,
            root,
            segments: field
                .map(|field| vec![ContentPlaceSegment::Field(field.to_owned())])
                .unwrap_or_default(),
        },
    };
    let entry = projected(ContentPlaceVersion::Entry, parameter_place, None);
    let left = projected(ContentPlaceVersion::Current, result_place, Some("left"));
    let right = projected(ContentPlaceVersion::Current, result_place, Some("right"));
    let proposition = Proposition::ContentConservation(ContentConservation::new(
        ContentAlgebra {
            kind: ContentAlgebraKind::IntervalSet,
            parameter: "Address".to_owned(),
        },
        entry,
        ContentTerm::separate([right, left]).expect("canonical separation"),
    ));
    TerminalModule {
        semantic_version,
        entry: machine_id(80),
        machines: vec![TerminalMachine {
            id: machine_id(80),
            parameters: vec![ValueDeclaration {
                id: value_id(80),
                scalar_type: ScalarType::Boolean,
            }],
            result: ValueDeclaration {
                id: value_id(81),
                scalar_type: ScalarType::Boolean,
            },
            structural_places: vec![
                StructuralPlaceDeclaration {
                    id: parameter_place,
                    kind: StructuralPlaceKind::Parameter {
                        position: 0,
                        is_self: false,
                    },
                },
                StructuralPlaceDeclaration {
                    id: result_place,
                    kind: StructuralPlaceKind::Result,
                },
            ],
            content_identity_reshuffles: Vec::new(),
            entry: block_id(80),
            blocks: vec![Block {
                id: block_id(80),
                parameters: Vec::new(),
                operations: Vec::new(),
                terminator: Terminator::Return {
                    edge: edge_id(80),
                    value: value_id(80),
                },
            }],
            contract: MachineContract {
                id: contract_id(80),
                requires: Vec::new(),
                ensures: vec![ContractClause {
                    obligation: obligation_id(80),
                    proposition,
                }],
            },
        }],
    }
}

fn identity_reshuffle_fixture(semantic_version: SemanticVersion) -> TerminalModule {
    let mut module = content_conservation_fixture(semantic_version);
    let input_root = module.machines[0].structural_places[0].id;
    let output_root = module.machines[0].structural_places[1].id;
    module.machines[0].content_identity_reshuffles = vec![ContentIdentityReshuffle {
        claim: claim_id(1),
        input: ContentStructuralPlace {
            version: ContentPlaceVersion::Entry,
            root: input_root,
            segments: vec![ContentPlaceSegment::Field("payload".to_owned())],
        },
        output: ContentStructuralPlace {
            version: ContentPlaceVersion::Current,
            root: output_root,
            segments: vec![ContentPlaceSegment::Field("payload".to_owned())],
        },
        projections: vec![
            ClaimContentProjection {
                projection: ContentProjectionIdentity {
                    domain: ContentDomainId::new(7).expect("domain"),
                    projection_fingerprint: 0x1234,
                },
                algebra: ContentAlgebra {
                    kind: ContentAlgebraKind::IntervalSet,
                    parameter: "Address".to_owned(),
                },
            },
            ClaimContentProjection {
                projection: ContentProjectionIdentity {
                    domain: ContentDomainId::new(8).expect("domain"),
                    projection_fingerprint: 0x5678,
                },
                algebra: ContentAlgebra {
                    kind: ContentAlgebraKind::CountedQuantity,
                    parameter: "Byte".to_owned(),
                },
            },
        ],
    }];
    module
}

fn wrapping_add_fixture(semantic_version: SemanticVersion) -> TerminalModule {
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let scalar_type = ScalarType::Integer(integer);
    TerminalModule {
        semantic_version,
        entry: machine_id(20),
        machines: vec![TerminalMachine {
            id: machine_id(20),
            parameters: Vec::new(),
            result: ValueDeclaration {
                id: value_id(23),
                scalar_type,
            },
            structural_places: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            entry: block_id(20),
            blocks: vec![Block {
                id: block_id(20),
                parameters: Vec::new(),
                operations: vec![
                    Operation {
                        id: operation_id(20),
                        result: ValueDeclaration {
                            id: value_id(20),
                            scalar_type,
                        },
                        kind: OperationKind::IntegerConstant {
                            value: IntegerValue::Unsigned(200),
                        },
                    },
                    Operation {
                        id: operation_id(21),
                        result: ValueDeclaration {
                            id: value_id(21),
                            scalar_type,
                        },
                        kind: OperationKind::IntegerConstant {
                            value: IntegerValue::Unsigned(100),
                        },
                    },
                    Operation {
                        id: operation_id(22),
                        result: ValueDeclaration {
                            id: value_id(22),
                            scalar_type,
                        },
                        kind: OperationKind::WrappingIntegerAdd {
                            left: value_id(20),
                            right: value_id(21),
                        },
                    },
                ],
                terminator: Terminator::Return {
                    edge: edge_id(20),
                    value: value_id(22),
                },
            }],
            contract: MachineContract {
                id: contract_id(20),
                requires: Vec::new(),
                ensures: Vec::new(),
            },
        }],
    }
}

fn saturating_add_fixture(semantic_version: SemanticVersion) -> TerminalModule {
    let mut module = wrapping_add_fixture(semantic_version);
    module.machines[0].blocks[0].operations[2].kind = OperationKind::SaturatingIntegerAdd {
        left: value_id(20),
        right: value_id(21),
    };
    module
}

fn wrapping_subtract_fixture(semantic_version: SemanticVersion) -> TerminalModule {
    let mut module = wrapping_add_fixture(semantic_version);
    module.machines[0].blocks[0].operations[0].kind = OperationKind::IntegerConstant {
        value: IntegerValue::Unsigned(5),
    };
    module.machines[0].blocks[0].operations[1].kind = OperationKind::IntegerConstant {
        value: IntegerValue::Unsigned(10),
    };
    module.machines[0].blocks[0].operations[2].kind = OperationKind::WrappingIntegerSubtract {
        left: value_id(20),
        right: value_id(21),
    };
    module
}

fn saturating_subtract_fixture(semantic_version: SemanticVersion) -> TerminalModule {
    let mut module = wrapping_subtract_fixture(semantic_version);
    module.machines[0].blocks[0].operations[2].kind = OperationKind::SaturatingIntegerSubtract {
        left: value_id(20),
        right: value_id(21),
    };
    module
}

fn wrapping_multiply_fixture(semantic_version: SemanticVersion) -> TerminalModule {
    let mut module = wrapping_add_fixture(semantic_version);
    module.machines[0].blocks[0].operations[0].kind = OperationKind::IntegerConstant {
        value: IntegerValue::Unsigned(20),
    };
    module.machines[0].blocks[0].operations[1].kind = OperationKind::IntegerConstant {
        value: IntegerValue::Unsigned(13),
    };
    module.machines[0].blocks[0].operations[2].kind = OperationKind::WrappingIntegerMultiply {
        left: value_id(20),
        right: value_id(21),
    };
    module
}

fn saturating_multiply_fixture(semantic_version: SemanticVersion) -> TerminalModule {
    let mut module = wrapping_multiply_fixture(semantic_version);
    module.machines[0].blocks[0].operations[2].kind = OperationKind::SaturatingIntegerMultiply {
        left: value_id(20),
        right: value_id(21),
    };
    module
}

fn i32_type() -> IntegerType {
    IntegerType::new(IntegerSign::Signed, 32).expect("i32")
}

macro_rules! id_constructor {
    ($function:ident, $type:ty) => {
        fn $function(raw: u64) -> $type {
            <$type>::new(raw).expect("test identities are nonzero")
        }
    };
}

id_constructor!(value_id, ValueId);
id_constructor!(machine_id, MachineId);
id_constructor!(block_id, BlockId);
id_constructor!(place_id, PlaceId);
id_constructor!(claim_id, ClaimId);
id_constructor!(operation_id, OperationId);
id_constructor!(edge_id, EdgeId);
id_constructor!(contract_id, ContractId);
id_constructor!(obligation_id, ObligationId);
id_constructor!(proposition_id, PropositionId);
