use psi_core::{
    BlockId, ClaimId, ContentAlgebra, ContentAlgebraKind, ContentConservation, ContentDomainId,
    ContentPlaceSegment, ContentPlaceVersion, ContentProjectionIdentity, ContentStructuralPlace,
    ContentTerm, ContractId, EdgeId, IntegerSign, IntegerType, IntegerValue, MachineId,
    ObligationId, OperationId, PlaceId, Proposition, PropositionId, ScalarTerm, ScalarType,
    StructuralPlaceKind, ValueId,
};
use psi_terminal::{
    Block, ClaimContentProjection, ContentEntryClaim, ContentIdentityReshuffle,
    ContentPartitionComposition, ContentPlaceSubstitution, ContractClause, CrashCause,
    MachineContract, Operation, OperationKind, PropositionApplicationIdentity,
    PropositionBinderArgumentIdentity, PropositionBinderArgumentKind, PropositionBinderDeclaration,
    PropositionBinderKind, PropositionDeclaration, PropositionEvidence, SemanticVersion,
    StructuralPlaceDeclaration, TerminalMachine, TerminalModule, Terminator, ValueDeclaration,
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
        "38a4a0f46fb88571b5425870bf2fbacb7c256593042b24ee4ac3137b4a9d9fbc"
    );
    assert_eq!(
        identity.program_fingerprint,
        semantic_fingerprint(&module).unwrap()
    );
}

#[test]
fn v24_crash_round_trips_and_every_semantic_field_enters_identity() {
    let mut module = fixture();
    module.machines[0].contract.crash_context = psi_terminal::CrashContextMaximum::portable_root();
    module.machines[0].blocks[1].terminator = Terminator::Crash {
        edge: edge_id(2),
        cause: CrashCause::Trap,
        damage_minimum: "Activation".to_owned(),
        containment_demand: "ExecutionDomain".to_owned(),
        frontier_lower_bound: Vec::new(),
    };
    let bytes = encode_module(&module).expect("v23 crash encodes");
    assert_eq!(decode_module(&bytes), Ok(module.clone()));

    let baseline = semantic_fingerprint(&module).expect("crash identity");
    if let Terminator::Crash { cause, .. } = &mut module.machines[0].blocks[1].terminator {
        *cause = CrashCause::Abort;
    } else {
        unreachable!()
    }
    assert_ne!(
        semantic_fingerprint(&module).expect("changed cause identity"),
        baseline
    );
    if let Terminator::Crash {
        cause,
        damage_minimum,
        ..
    } = &mut module.machines[0].blocks[1].terminator
    {
        *cause = CrashCause::Trap;
        *damage_minimum = "ExecutionDomain".to_owned();
    } else {
        unreachable!()
    }
    assert_ne!(
        semantic_fingerprint(&module).expect("changed minimum identity"),
        baseline
    );
    if let Terminator::Crash {
        damage_minimum,
        containment_demand,
        ..
    } = &mut module.machines[0].blocks[1].terminator
    {
        *damage_minimum = "Activation".to_owned();
        *containment_demand = "Activation".to_owned();
    } else {
        unreachable!()
    }
    assert_ne!(
        semantic_fingerprint(&module).expect("changed demand identity"),
        baseline
    );
    module.machines[0].contract.crash_context[0].maximum_scope = "Activation".to_owned();
    assert_ne!(
        semantic_fingerprint(&module).expect("changed context maximum identity"),
        baseline
    );
}

#[test]
fn v22_crash_decodes_its_single_scope_as_equal_minimum_and_demand() {
    let mut module = fixture();
    module.semantic_version = SemanticVersion::V22;
    module.machines[0].blocks[1].terminator = Terminator::Crash {
        edge: edge_id(2),
        cause: CrashCause::Trap,
        damage_minimum: "Activation".to_owned(),
        containment_demand: "Activation".to_owned(),
        frontier_lower_bound: Vec::new(),
    };
    let bytes = encode_module(&module).expect("v22 crash encodes");
    let decoded = decode_module(&bytes).expect("v22 crash decodes");
    assert_eq!(decoded, module);
    let migrated = migrate_module_to_current(&decoded).expect("v22 crash migrates");
    assert_eq!(migrated.semantic_version, SemanticVersion::CURRENT);
    assert_eq!(
        migrated.machines[0].contract.crash_context,
        vec![psi_terminal::CrashContextMaximum {
            cause: CrashCause::Trap,
            maximum_scope: "ExecutionDomain".to_owned(),
        }]
    );
    assert!(matches!(
        &migrated.machines[0].blocks[1].terminator,
        Terminator::Crash {
            damage_minimum,
            containment_demand,
            ..
        } if damage_minimum == "Activation" && containment_demand == "Activation"
    ));
}

#[test]
fn archived_v16_current_fixture_keeps_its_identity() {
    let mut module = fixture();
    module.semantic_version = SemanticVersion::V16;
    assert_eq!(
        semantic_fingerprint(&module).unwrap().to_string(),
        "40dbb76be0de2b2a25865f90d8fd59a428f90d9031fb008f571362aeb64d45af"
    );
}

#[test]
fn v16_proposition_vocabulary_round_trips_and_enters_identity() {
    let mut module = fixture();
    module.proposition_declarations = vec![PropositionDeclaration {
        id: proposition_id(1),
        name: "converges_together".to_owned(),
        binders: vec![
            PropositionBinderDeclaration {
                name: "Left".to_owned(),
                kind: PropositionBinderKind::Machine,
            },
            PropositionBinderDeclaration {
                name: "Precision".to_owned(),
                kind: PropositionBinderKind::Const {
                    type_identity: "u32".to_owned(),
                },
            },
        ],
        parameter_types: vec!["CauchySeq<Left>".to_owned()],
        evidence: PropositionEvidence::Witness {
            evidence_type: "ConvergenceEvidence<Left>".to_owned(),
        },
    }];
    module.proposition_applications = vec![PropositionApplicationIdentity {
        id: proposition_id(1),
        declaration: proposition_id(1),
        binder_arguments: vec![
            PropositionBinderArgumentIdentity {
                kind: PropositionBinderArgumentKind::Machine,
                identity: "unit_sample".to_owned(),
            },
            PropositionBinderArgumentIdentity {
                kind: PropositionBinderArgumentKind::Const,
                identity: "32u32".to_owned(),
            },
        ],
        arguments: vec!["sequence".to_owned()],
    }];

    let bytes = encode_module(&module).expect("v16 proposition vocabulary should encode");
    assert_eq!(decode_module(&bytes), Ok(module.clone()));

    let original = semantic_fingerprint(&module).expect("vocabulary has identity");
    module.proposition_declarations[0].evidence = PropositionEvidence::Witness {
        evidence_type: "AlternativeEvidence<Left>".to_owned(),
    };
    assert_ne!(
        semantic_fingerprint(&module).expect("changed evidence interface has identity"),
        original
    );
    module.proposition_declarations[0].evidence = PropositionEvidence::FactOnly;
    assert_ne!(
        semantic_fingerprint(&module).expect("changed vocabulary has identity"),
        original
    );
}

#[test]
fn proposition_vocabulary_is_versioned_and_category_checked() {
    let mut module = fixture();
    module.proposition_declarations = vec![PropositionDeclaration {
        id: proposition_id(1),
        name: "related".to_owned(),
        binders: vec![PropositionBinderDeclaration {
            name: "Carrier".to_owned(),
            kind: PropositionBinderKind::Type,
        }],
        parameter_types: vec!["Carrier".to_owned()],
        evidence: PropositionEvidence::FactOnly,
    }];
    module.proposition_applications = vec![PropositionApplicationIdentity {
        id: proposition_id(1),
        declaration: proposition_id(1),
        binder_arguments: vec![PropositionBinderArgumentIdentity {
            kind: PropositionBinderArgumentKind::Machine,
            identity: "Generator".to_owned(),
        }],
        arguments: vec!["value".to_owned()],
    }];
    assert!(matches!(
        encode_module(&module),
        Err(CodecError::InvalidModule(
            psi_terminal_verifier::ModuleError::PropositionApplicationBinderMismatch(_)
        ))
    ));

    module.proposition_applications.clear();
    module.semantic_version = SemanticVersion::V15;
    assert_eq!(
        encode_module(&module),
        Err(CodecError::PropositionVocabularyRequiresV16)
    );
}

#[test]
fn archived_v13_current_fixture_keeps_its_identity() {
    let mut module = fixture();
    module.semantic_version = SemanticVersion::V13;
    assert_eq!(
        semantic_fingerprint(&module).unwrap().to_string(),
        "6cc8107a9a366b943151ecf8c766065d77d4ea1bb9c3b62c5603471e2e25e6e6"
    );
}

#[test]
fn archived_v12_current_fixture_keeps_its_identity() {
    let mut module = fixture();
    module.semantic_version = SemanticVersion::V12;
    assert_eq!(
        semantic_fingerprint(&module).unwrap().to_string(),
        "f5939272cbb8d15cd8201a16ba7eed0b6851398996d416752b415c7aef7aeb4a"
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

    let mut sparse = module;
    sparse.machines[0].content_identity_reshuffles[0].claim = claim_id(9);
    let migrated = migrate_module_to_current(&sparse).expect("v10 reshuffle migrates to current");
    assert_eq!(migrated.semantic_version, SemanticVersion::CURRENT);
    assert_eq!(migrated.machines[0].content_entry_claims.len(), 1);
    assert_eq!(
        migrated.machines[0].content_entry_claims[0].claim,
        claim_id(1)
    );
    assert_eq!(
        migrated.machines[0].content_identity_reshuffles[0].claim,
        claim_id(1)
    );
}

#[test]
fn v15_boolean_not_has_stable_canonical_bytes() {
    let mut module = boolean_fixture(SemanticVersion::V15);
    module.machines[0].parameters = vec![ValueDeclaration {
        id: value_id(12),
        scalar_type: ScalarType::Boolean,
    }];
    module.machines[0].blocks[0].operations[0].kind = OperationKind::BooleanNot {
        operand: value_id(12),
    };
    let bytes = encode_module(&module).expect("v15 Boolean-not module should encode");

    assert_eq!(decode_module(&bytes), Ok(module.clone()));
    assert_eq!(encode_module(&decode_module(&bytes).unwrap()), Ok(bytes));
    assert_eq!(
        semantic_fingerprint(&module).unwrap().to_string(),
        "feff8da3d27a1a60d4c54377641f9da883286964dfd40e1304bd88788b78ce8b"
    );
}

#[test]
fn v17_boolean_equality_has_stable_canonical_bytes() {
    let mut module = boolean_fixture(SemanticVersion::V17);
    module.machines[0].parameters = vec![
        ValueDeclaration {
            id: value_id(12),
            scalar_type: ScalarType::Boolean,
        },
        ValueDeclaration {
            id: value_id(13),
            scalar_type: ScalarType::Boolean,
        },
    ];
    module.machines[0].blocks[0].operations[0].kind = OperationKind::BooleanEqual {
        left: value_id(12),
        right: value_id(13),
    };
    let bytes = encode_module(&module).expect("v17 Boolean-equality module should encode");

    assert_eq!(decode_module(&bytes), Ok(module.clone()));
    assert_eq!(encode_module(&decode_module(&bytes).unwrap()), Ok(bytes));
    assert_eq!(
        semantic_fingerprint(&module).unwrap().to_string(),
        "e46c836ce6d3c86000124c2320b74b9fb26043f200481415a00b289b4067888e"
    );
}

#[test]
fn v18_integer_equality_has_stable_canonical_bytes() {
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let scalar_type = ScalarType::Integer(integer);
    let mut module = boolean_fixture(SemanticVersion::V18);
    module.machines[0].parameters = vec![
        ValueDeclaration {
            id: value_id(12),
            scalar_type,
        },
        ValueDeclaration {
            id: value_id(13),
            scalar_type,
        },
    ];
    module.machines[0].blocks[0].operations[0].kind = OperationKind::IntegerEqual {
        left: value_id(12),
        right: value_id(13),
    };
    let bytes = encode_module(&module).expect("v18 integer-equality module should encode");

    assert_eq!(decode_module(&bytes), Ok(module.clone()));
    assert_eq!(encode_module(&decode_module(&bytes).unwrap()), Ok(bytes));
    assert_eq!(
        semantic_fingerprint(&module).unwrap().to_string(),
        "485cc2e33baf30365538ecc6086e14debdcd9acf860a4f547e48eb5a75c1ea67"
    );
}

#[test]
fn v19_integer_ordering_has_stable_distinct_canonical_bytes() {
    let integer = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let scalar_type = ScalarType::Integer(integer);
    let mut fingerprints = Vec::new();
    for inclusive in [false, true] {
        let mut module = boolean_fixture(SemanticVersion::V19);
        module.machines[0].parameters = vec![
            ValueDeclaration {
                id: value_id(12),
                scalar_type,
            },
            ValueDeclaration {
                id: value_id(13),
                scalar_type,
            },
        ];
        module.machines[0].blocks[0].operations[0].kind = if inclusive {
            OperationKind::IntegerLessOrEqual {
                left: value_id(12),
                right: value_id(13),
            }
        } else {
            OperationKind::IntegerLessThan {
                left: value_id(12),
                right: value_id(13),
            }
        };
        let bytes = encode_module(&module).expect("v19 integer ordering should encode");
        assert_eq!(decode_module(&bytes), Ok(module.clone()));
        assert_eq!(encode_module(&decode_module(&bytes).unwrap()), Ok(bytes));
        fingerprints.push(semantic_fingerprint(&module).unwrap().to_string());
    }
    assert_eq!(
        fingerprints,
        [
            "b3ff100dce2ca7694563837ae403e3f6754be7e1d29a395d086f853ca0832032",
            "d5b9ae3bdd3b3ced4fb6e9d03116f6df7128325c388035be437c4c3fb0d09baa",
        ]
    );
}

#[test]
fn v20_integer_bitwise_has_stable_distinct_canonical_bytes() {
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let scalar_type = ScalarType::Integer(integer);
    let mut fingerprints = Vec::new();
    for kind in 0_u8..3 {
        let module = bitwise_fixture(kind, scalar_type);
        let bytes = encode_module(&module).expect("v20 integer bitwise should encode");
        assert_eq!(decode_module(&bytes), Ok(module.clone()));
        assert_eq!(encode_module(&decode_module(&bytes).unwrap()), Ok(bytes));
        fingerprints.push(semantic_fingerprint(&module).unwrap().to_string());
    }
    assert_eq!(
        fingerprints,
        [
            "03939b9634885e2eb145699a615acc6c20d102661dd41c945739c3a62fafabac",
            "65c640057a6241b4c058a008fc60e52f4610f19493cf93add40bb4a9456a4db7",
            "dd37cd20f4ca3deda40ecd0ce94b410c5983d12207dcf8c9bf6d34161341a15d",
        ]
    );
}

#[test]
fn v21_wrapping_shifts_have_stable_distinct_canonical_bytes() {
    let mut fingerprints = Vec::new();
    for left in [true, false] {
        let module = wrapping_shift_fixture(left);
        let bytes = encode_module(&module).expect("v21 wrapping shift should encode");
        assert_eq!(decode_module(&bytes), Ok(module.clone()));
        assert_eq!(encode_module(&decode_module(&bytes).unwrap()), Ok(bytes));
        fingerprints.push(semantic_fingerprint(&module).unwrap().to_string());
    }
    assert_eq!(
        fingerprints,
        [
            "b87a1e7f569fc72959db367153851c407032a36ec126f9eb324467d4a7b27703",
            "aaa2e02bb9b280e4344900e9a1cc59200a0caef1aca79f30e7420ddf5b784aa2",
        ]
    );
}

#[test]
fn v25_integer_bitwise_not_has_stable_canonical_bytes() {
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let scalar_type = ScalarType::Integer(integer);
    let mut module = bitwise_fixture(0, scalar_type);
    module.semantic_version = SemanticVersion::V25;
    let operand = module.machines[0].parameters[0].id;
    module.machines[0].blocks[0].operations[0].kind = OperationKind::IntegerBitwiseNot { operand };
    let bytes = encode_module(&module).expect("v25 integer bitwise-not should encode");
    assert_eq!(decode_module(&bytes), Ok(module.clone()));
    assert_eq!(encode_module(&decode_module(&bytes).unwrap()), Ok(bytes));
    assert_eq!(
        semantic_fingerprint(&module).unwrap().to_string(),
        "9f6f66f8b172ad4e8f50b7f6ec65715321ebe13e897c00ee081ccd6e0a92d91c"
    );
}

#[test]
fn v26_integer_widen_has_stable_canonical_bytes() {
    let source_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let target_type = IntegerType::new(IntegerSign::Unsigned, 64).expect("u64");
    let operand = value_id(16);
    let computed = value_id(17);
    let result = value_id(18);
    let module = TerminalModule {
        semantic_version: SemanticVersion::V26,
        entry: machine_id(16),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        machines: vec![TerminalMachine {
            id: machine_id(16),
            parameters: vec![ValueDeclaration {
                id: operand,
                scalar_type: ScalarType::Integer(source_type),
            }],
            result: ValueDeclaration {
                id: result,
                scalar_type: ScalarType::Integer(target_type),
            },
            structural_places: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: block_id(16),
            blocks: vec![Block {
                id: block_id(16),
                parameters: Vec::new(),
                operations: vec![Operation {
                    id: operation_id(16),
                    result: ValueDeclaration {
                        id: computed,
                        scalar_type: ScalarType::Integer(target_type),
                    },
                    kind: OperationKind::IntegerWiden { operand },
                }],
                terminator: Terminator::Return {
                    edge: edge_id(16),
                    value: computed,
                },
            }],
            contract: MachineContract {
                id: contract_id(16),
                crash_context: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
            },
        }],
    };
    let bytes = encode_module(&module).expect("v26 integer widen should encode");
    assert_eq!(decode_module(&bytes), Ok(module.clone()));
    assert_eq!(encode_module(&decode_module(&bytes).unwrap()), Ok(bytes));
    assert_eq!(
        semantic_fingerprint(&module).unwrap().to_string(),
        "9ce4f4943b365ce4662fbf72477b81c6f97ab84d6e9d5af97ce0c0f9e346225f"
    );
}

#[test]
fn v27_address_carrier_has_stable_canonical_bytes() {
    let address = IntegerType::address(64).expect("addr");
    let operand = value_id(19);
    let result = value_id(20);
    let module = TerminalModule {
        semantic_version: SemanticVersion::V27,
        entry: machine_id(17),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        machines: vec![TerminalMachine {
            id: machine_id(17),
            parameters: vec![ValueDeclaration {
                id: operand,
                scalar_type: ScalarType::Integer(address),
            }],
            result: ValueDeclaration {
                id: result,
                scalar_type: ScalarType::Integer(address),
            },
            structural_places: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: block_id(17),
            blocks: vec![Block {
                id: block_id(17),
                parameters: Vec::new(),
                operations: Vec::new(),
                terminator: Terminator::Return {
                    edge: edge_id(17),
                    value: operand,
                },
            }],
            contract: MachineContract {
                id: contract_id(17),
                crash_context: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
            },
        }],
    };
    let bytes = encode_module(&module).expect("v27 address module should encode");
    assert_eq!(decode_module(&bytes), Ok(module.clone()));
    assert_eq!(encode_module(&decode_module(&bytes).unwrap()), Ok(bytes));
    assert_eq!(
        semantic_fingerprint(&module).unwrap().to_string(),
        "3f02af6d6911ce66b210e82a6e770236b0b8b3236d9bf15ca319bf7a41b369aa"
    );
}

#[test]
fn v28_exact_integer_cast_has_stable_canonical_bytes() {
    let source_type = IntegerType::new(IntegerSign::Unsigned, 64).expect("u64");
    let target_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let operand = value_id(21);
    let computed = value_id(22);
    let result = value_id(23);
    let module = TerminalModule {
        semantic_version: SemanticVersion::V28,
        entry: machine_id(18),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        machines: vec![TerminalMachine {
            id: machine_id(18),
            parameters: vec![ValueDeclaration {
                id: operand,
                scalar_type: ScalarType::Integer(source_type),
            }],
            result: ValueDeclaration {
                id: result,
                scalar_type: ScalarType::Integer(target_type),
            },
            structural_places: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: block_id(18),
            blocks: vec![Block {
                id: block_id(18),
                parameters: Vec::new(),
                operations: vec![Operation {
                    id: operation_id(18),
                    result: ValueDeclaration {
                        id: computed,
                        scalar_type: ScalarType::Integer(target_type),
                    },
                    kind: OperationKind::IntegerExactCast {
                        operand,
                        obligation: obligation_id(18),
                    },
                }],
                terminator: Terminator::Return {
                    edge: edge_id(18),
                    value: computed,
                },
            }],
            contract: MachineContract {
                id: contract_id(18),
                crash_context: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
            },
        }],
    };
    let bytes = encode_module(&module).expect("v28 exact cast should encode");
    assert_eq!(decode_module(&bytes), Ok(module.clone()));
    assert_eq!(encode_module(&decode_module(&bytes).unwrap()), Ok(bytes));
    assert_eq!(
        semantic_fingerprint(&module).unwrap().to_string(),
        "48edb4e767c87665bb16dee763452bbc0fe164d2d6fcf3cab26d2eb62aa0c5f3"
    );
}

#[test]
fn v29_exact_right_shift_has_stable_canonical_bytes() {
    let value_type = IntegerType::new(IntegerSign::Unsigned, 64).expect("u64");
    let count_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let value = value_id(24);
    let count = value_id(25);
    let computed = value_id(26);
    let result = value_id(27);
    let module = TerminalModule {
        semantic_version: SemanticVersion::V29,
        entry: machine_id(19),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        machines: vec![TerminalMachine {
            id: machine_id(19),
            parameters: vec![
                ValueDeclaration {
                    id: value,
                    scalar_type: ScalarType::Integer(value_type),
                },
                ValueDeclaration {
                    id: count,
                    scalar_type: ScalarType::Integer(count_type),
                },
            ],
            result: ValueDeclaration {
                id: result,
                scalar_type: ScalarType::Integer(value_type),
            },
            structural_places: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: block_id(19),
            blocks: vec![Block {
                id: block_id(19),
                parameters: Vec::new(),
                operations: vec![Operation {
                    id: operation_id(19),
                    result: ValueDeclaration {
                        id: computed,
                        scalar_type: ScalarType::Integer(value_type),
                    },
                    kind: OperationKind::ExactIntegerShiftRight {
                        value,
                        count,
                        obligation: obligation_id(19),
                    },
                }],
                terminator: Terminator::Return {
                    edge: edge_id(19),
                    value: computed,
                },
            }],
            contract: MachineContract {
                id: contract_id(19),
                crash_context: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
            },
        }],
    };
    let bytes = encode_module(&module).expect("v29 exact right shift should encode");
    assert_eq!(decode_module(&bytes), Ok(module.clone()));
    assert_eq!(encode_module(&decode_module(&bytes).unwrap()), Ok(bytes));
    assert_eq!(
        semantic_fingerprint(&module).unwrap().to_string(),
        "8172fa79e963cb3f26c6fced3d89656a1f69faab5d3eaaf0e85a84f4993eace2"
    );
}

#[test]
fn v30_exact_left_shift_has_stable_canonical_bytes() {
    let value_type = IntegerType::new(IntegerSign::Unsigned, 32).expect("u32");
    let count_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let value = value_id(28);
    let count = value_id(29);
    let computed = value_id(30);
    let result = value_id(31);
    let module = TerminalModule {
        semantic_version: SemanticVersion::V30,
        entry: machine_id(20),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        machines: vec![TerminalMachine {
            id: machine_id(20),
            parameters: vec![
                ValueDeclaration {
                    id: value,
                    scalar_type: ScalarType::Integer(value_type),
                },
                ValueDeclaration {
                    id: count,
                    scalar_type: ScalarType::Integer(count_type),
                },
            ],
            result: ValueDeclaration {
                id: result,
                scalar_type: ScalarType::Integer(value_type),
            },
            structural_places: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: block_id(20),
            blocks: vec![Block {
                id: block_id(20),
                parameters: Vec::new(),
                operations: vec![Operation {
                    id: operation_id(20),
                    result: ValueDeclaration {
                        id: computed,
                        scalar_type: ScalarType::Integer(value_type),
                    },
                    kind: OperationKind::ExactIntegerShiftLeft {
                        value,
                        count,
                        obligation: obligation_id(20),
                    },
                }],
                terminator: Terminator::Return {
                    edge: edge_id(20),
                    value: computed,
                },
            }],
            contract: MachineContract {
                id: contract_id(20),
                crash_context: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
            },
        }],
    };
    let bytes = encode_module(&module).expect("v30 exact left shift should encode");
    assert_eq!(decode_module(&bytes), Ok(module.clone()));
    assert_eq!(encode_module(&decode_module(&bytes).unwrap()), Ok(bytes));
    assert_eq!(
        semantic_fingerprint(&module).unwrap().to_string(),
        "661538f8719fb7c200f7a74e4265bda127f290fc0a5556e53c0b26ec92e4b5da"
    );
}

#[test]
fn v31_exact_add_has_stable_canonical_bytes() {
    let scalar_type = IntegerType::new(IntegerSign::Unsigned, 32).expect("u32");
    let left = value_id(32);
    let right = value_id(33);
    let computed = value_id(34);
    let result = value_id(35);
    let declaration = |id| ValueDeclaration {
        id,
        scalar_type: ScalarType::Integer(scalar_type),
    };
    let module = TerminalModule {
        semantic_version: SemanticVersion::V31,
        entry: machine_id(21),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        machines: vec![TerminalMachine {
            id: machine_id(21),
            parameters: vec![declaration(left), declaration(right)],
            result: declaration(result),
            structural_places: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: block_id(21),
            blocks: vec![Block {
                id: block_id(21),
                parameters: Vec::new(),
                operations: vec![Operation {
                    id: operation_id(21),
                    result: declaration(computed),
                    kind: OperationKind::ExactIntegerAdd {
                        left,
                        right,
                        obligation: obligation_id(21),
                    },
                }],
                terminator: Terminator::Return {
                    edge: edge_id(21),
                    value: computed,
                },
            }],
            contract: MachineContract {
                id: contract_id(21),
                crash_context: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
            },
        }],
    };
    let bytes = encode_module(&module).expect("v31 exact add should encode");
    assert_eq!(decode_module(&bytes), Ok(module.clone()));
    assert_eq!(encode_module(&decode_module(&bytes).unwrap()), Ok(bytes));
    assert_eq!(
        semantic_fingerprint(&module).unwrap().to_string(),
        "6dc30e2c8917dc66edec2117f43dc0f31da12031783f7c12b047d1310546f4ba"
    );
}

#[test]
fn v32_exact_subtract_has_stable_canonical_bytes() {
    let scalar_type = IntegerType::new(IntegerSign::Unsigned, 32).expect("u32");
    let left = value_id(36);
    let right = value_id(37);
    let computed = value_id(38);
    let result = value_id(39);
    let declaration = |id| ValueDeclaration {
        id,
        scalar_type: ScalarType::Integer(scalar_type),
    };
    let module = TerminalModule {
        semantic_version: SemanticVersion::V32,
        entry: machine_id(22),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        machines: vec![TerminalMachine {
            id: machine_id(22),
            parameters: vec![declaration(left), declaration(right)],
            result: declaration(result),
            structural_places: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: block_id(22),
            blocks: vec![Block {
                id: block_id(22),
                parameters: Vec::new(),
                operations: vec![Operation {
                    id: operation_id(22),
                    result: declaration(computed),
                    kind: OperationKind::ExactIntegerSubtract {
                        left,
                        right,
                        obligation: obligation_id(22),
                    },
                }],
                terminator: Terminator::Return {
                    edge: edge_id(22),
                    value: computed,
                },
            }],
            contract: MachineContract {
                id: contract_id(22),
                crash_context: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
            },
        }],
    };
    let bytes = encode_module(&module).expect("v32 exact subtract should encode");
    assert_eq!(decode_module(&bytes), Ok(module.clone()));
    assert_eq!(encode_module(&decode_module(&bytes).unwrap()), Ok(bytes));
    assert_eq!(
        semantic_fingerprint(&module).unwrap().to_string(),
        "24381000184ad790e9d684cf487cbfe72777c7cfc9f48410d20315c8118d6fa3"
    );
}

#[test]
fn v33_exact_multiply_has_stable_canonical_bytes() {
    let scalar_type = IntegerType::new(IntegerSign::Unsigned, 32).expect("u32");
    let left = value_id(40);
    let right = value_id(41);
    let computed = value_id(42);
    let result = value_id(43);
    let declaration = |id| ValueDeclaration {
        id,
        scalar_type: ScalarType::Integer(scalar_type),
    };
    let module = TerminalModule {
        semantic_version: SemanticVersion::V33,
        entry: machine_id(23),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        machines: vec![TerminalMachine {
            id: machine_id(23),
            parameters: vec![declaration(left), declaration(right)],
            result: declaration(result),
            structural_places: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: block_id(23),
            blocks: vec![Block {
                id: block_id(23),
                parameters: Vec::new(),
                operations: vec![Operation {
                    id: operation_id(23),
                    result: declaration(computed),
                    kind: OperationKind::ExactIntegerMultiply {
                        left,
                        right,
                        obligation: obligation_id(23),
                    },
                }],
                terminator: Terminator::Return {
                    edge: edge_id(23),
                    value: computed,
                },
            }],
            contract: MachineContract {
                id: contract_id(23),
                crash_context: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
            },
        }],
    };
    let bytes = encode_module(&module).expect("v33 exact multiply should encode");
    assert_eq!(decode_module(&bytes), Ok(module.clone()));
    assert_eq!(encode_module(&decode_module(&bytes).unwrap()), Ok(bytes));
    assert_eq!(
        semantic_fingerprint(&module).unwrap().to_string(),
        "89073e4a28853cd7b262435f90f67088ed3ae5de0789db32fbd75f715aa5173b"
    );
}

#[test]
fn v34_exact_divide_has_stable_canonical_bytes() {
    let scalar_type = IntegerType::new(IntegerSign::Unsigned, 32).expect("u32");
    let left = value_id(50);
    let right = value_id(51);
    let computed = value_id(52);
    let result = value_id(53);
    let declaration = |id| ValueDeclaration {
        id,
        scalar_type: ScalarType::Integer(scalar_type),
    };
    let module = TerminalModule {
        semantic_version: SemanticVersion::V34,
        entry: machine_id(24),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        machines: vec![TerminalMachine {
            id: machine_id(24),
            parameters: vec![declaration(left), declaration(right)],
            result: declaration(result),
            structural_places: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: block_id(24),
            blocks: vec![Block {
                id: block_id(24),
                parameters: Vec::new(),
                operations: vec![Operation {
                    id: operation_id(24),
                    result: declaration(computed),
                    kind: OperationKind::ExactIntegerDivide {
                        left,
                        right,
                        obligation: obligation_id(24),
                    },
                }],
                terminator: Terminator::Return {
                    edge: edge_id(24),
                    value: computed,
                },
            }],
            contract: MachineContract {
                id: contract_id(24),
                crash_context: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
            },
        }],
    };
    let bytes = encode_module(&module).expect("v34 exact divide should encode");
    assert_eq!(decode_module(&bytes), Ok(module.clone()));
    assert_eq!(encode_module(&decode_module(&bytes).unwrap()), Ok(bytes));
    assert_eq!(
        semantic_fingerprint(&module).unwrap().to_string(),
        "c63e87f2702e611915a471d312e796e1189bbb916aeb49c87c70e2da6600e5f4"
    );
}

#[test]
fn v35_exact_remainder_has_stable_canonical_bytes() {
    let scalar_type = IntegerType::new(IntegerSign::Unsigned, 32).expect("u32");
    let left = value_id(60);
    let right = value_id(61);
    let computed = value_id(62);
    let result = value_id(63);
    let declaration = |id| ValueDeclaration {
        id,
        scalar_type: ScalarType::Integer(scalar_type),
    };
    let module = TerminalModule {
        semantic_version: SemanticVersion::V35,
        entry: machine_id(25),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        machines: vec![TerminalMachine {
            id: machine_id(25),
            parameters: vec![declaration(left), declaration(right)],
            result: declaration(result),
            structural_places: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: block_id(25),
            blocks: vec![Block {
                id: block_id(25),
                parameters: Vec::new(),
                operations: vec![Operation {
                    id: operation_id(25),
                    result: declaration(computed),
                    kind: OperationKind::ExactIntegerRemainder {
                        left,
                        right,
                        obligation: obligation_id(25),
                    },
                }],
                terminator: Terminator::Return {
                    edge: edge_id(25),
                    value: computed,
                },
            }],
            contract: MachineContract {
                id: contract_id(25),
                crash_context: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
            },
        }],
    };
    let bytes = encode_module(&module).expect("v35 exact remainder should encode");
    assert_eq!(decode_module(&bytes), Ok(module.clone()));
    assert_eq!(encode_module(&decode_module(&bytes).unwrap()), Ok(bytes));
    assert_eq!(
        semantic_fingerprint(&module).unwrap().to_string(),
        "2f0127e591206e4fb4bb0902aecf7ac2a33b5c2aa0250e0d662083cb07723811"
    );
}

#[test]
fn v36_wrapping_divide_has_stable_canonical_bytes() {
    let scalar_type = IntegerType::new(IntegerSign::Signed, 32).expect("i32");
    let left = value_id(70);
    let right = value_id(71);
    let computed = value_id(72);
    let result = value_id(73);
    let declaration = |id| ValueDeclaration {
        id,
        scalar_type: ScalarType::Integer(scalar_type),
    };
    let module = TerminalModule {
        semantic_version: SemanticVersion::V36,
        entry: machine_id(26),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        machines: vec![TerminalMachine {
            id: machine_id(26),
            parameters: vec![declaration(left), declaration(right)],
            result: declaration(result),
            structural_places: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: block_id(26),
            blocks: vec![Block {
                id: block_id(26),
                parameters: Vec::new(),
                operations: vec![Operation {
                    id: operation_id(26),
                    result: declaration(computed),
                    kind: OperationKind::WrappingIntegerDivide {
                        left,
                        right,
                        obligation: obligation_id(26),
                    },
                }],
                terminator: Terminator::Return {
                    edge: edge_id(26),
                    value: computed,
                },
            }],
            contract: MachineContract {
                id: contract_id(26),
                crash_context: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
            },
        }],
    };
    let bytes = encode_module(&module).expect("v36 wrapping divide should encode");
    assert_eq!(decode_module(&bytes), Ok(module.clone()));
    assert_eq!(encode_module(&decode_module(&bytes).unwrap()), Ok(bytes));
    assert_eq!(
        semantic_fingerprint(&module).unwrap().to_string(),
        "1046d2b16b3a519557f389854002e6cdb164609730f5f0d82f8ee0456c693e7d"
    );
}

#[test]
fn v37_wrapping_remainder_has_stable_canonical_bytes() {
    let scalar_type = IntegerType::new(IntegerSign::Signed, 32).expect("i32");
    let left = value_id(74);
    let right = value_id(75);
    let computed = value_id(76);
    let result = value_id(77);
    let declaration = |id| ValueDeclaration {
        id,
        scalar_type: ScalarType::Integer(scalar_type),
    };
    let module = TerminalModule {
        semantic_version: SemanticVersion::V37,
        entry: machine_id(27),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        machines: vec![TerminalMachine {
            id: machine_id(27),
            parameters: vec![declaration(left), declaration(right)],
            result: declaration(result),
            structural_places: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: block_id(27),
            blocks: vec![Block {
                id: block_id(27),
                parameters: Vec::new(),
                operations: vec![Operation {
                    id: operation_id(27),
                    result: declaration(computed),
                    kind: OperationKind::WrappingIntegerRemainder {
                        left,
                        right,
                        obligation: obligation_id(27),
                    },
                }],
                terminator: Terminator::Return {
                    edge: edge_id(27),
                    value: computed,
                },
            }],
            contract: MachineContract {
                id: contract_id(27),
                crash_context: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
            },
        }],
    };
    let bytes = encode_module(&module).expect("v37 wrapping remainder should encode");
    assert_eq!(decode_module(&bytes), Ok(module.clone()));
    assert_eq!(encode_module(&decode_module(&bytes).unwrap()), Ok(bytes));
    assert_eq!(
        semantic_fingerprint(&module).unwrap().to_string(),
        "9bbd789ea30c652e9933aef0cda8fadc5fc8a2538ac17643e7e4c700a76dd25d"
    );
}

#[test]
fn v38_saturating_divide_has_stable_canonical_bytes() {
    let scalar_type = IntegerType::new(IntegerSign::Signed, 32).expect("i32");
    let left = value_id(78);
    let right = value_id(79);
    let computed = value_id(80);
    let result = value_id(81);
    let declaration = |id| ValueDeclaration {
        id,
        scalar_type: ScalarType::Integer(scalar_type),
    };
    let module = TerminalModule {
        semantic_version: SemanticVersion::V38,
        entry: machine_id(28),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        machines: vec![TerminalMachine {
            id: machine_id(28),
            parameters: vec![declaration(left), declaration(right)],
            result: declaration(result),
            structural_places: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: block_id(28),
            blocks: vec![Block {
                id: block_id(28),
                parameters: Vec::new(),
                operations: vec![Operation {
                    id: operation_id(28),
                    result: declaration(computed),
                    kind: OperationKind::SaturatingIntegerDivide {
                        left,
                        right,
                        obligation: obligation_id(28),
                    },
                }],
                terminator: Terminator::Return {
                    edge: edge_id(28),
                    value: computed,
                },
            }],
            contract: MachineContract {
                id: contract_id(28),
                crash_context: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
            },
        }],
    };
    let bytes = encode_module(&module).expect("v38 saturating divide should encode");
    assert_eq!(decode_module(&bytes), Ok(module.clone()));
    assert_eq!(encode_module(&decode_module(&bytes).unwrap()), Ok(bytes));
    assert_eq!(
        semantic_fingerprint(&module).unwrap().to_string(),
        "0fc3a60d668188922a6aa88f78fc2520b61fcf33d838e3b9a37c330c496479fd"
    );
}

#[test]
fn v39_saturating_remainder_has_stable_canonical_bytes() {
    let scalar_type = IntegerType::new(IntegerSign::Signed, 32).expect("i32");
    let left = value_id(82);
    let right = value_id(83);
    let computed = value_id(84);
    let result = value_id(85);
    let declaration = |id| ValueDeclaration {
        id,
        scalar_type: ScalarType::Integer(scalar_type),
    };
    let module = TerminalModule {
        semantic_version: SemanticVersion::V39,
        entry: machine_id(29),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        machines: vec![TerminalMachine {
            id: machine_id(29),
            parameters: vec![declaration(left), declaration(right)],
            result: declaration(result),
            structural_places: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: block_id(29),
            blocks: vec![Block {
                id: block_id(29),
                parameters: Vec::new(),
                operations: vec![Operation {
                    id: operation_id(29),
                    result: declaration(computed),
                    kind: OperationKind::SaturatingIntegerRemainder {
                        left,
                        right,
                        obligation: obligation_id(29),
                    },
                }],
                terminator: Terminator::Return {
                    edge: edge_id(29),
                    value: computed,
                },
            }],
            contract: MachineContract {
                id: contract_id(29),
                crash_context: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
            },
        }],
    };
    let bytes = encode_module(&module).expect("v39 saturating remainder should encode");
    assert_eq!(decode_module(&bytes), Ok(module.clone()));
    assert_eq!(encode_module(&decode_module(&bytes).unwrap()), Ok(bytes));
    assert_eq!(
        semantic_fingerprint(&module).unwrap().to_string(),
        "7421e10cecaacc39378286d8b49462be60cd64df8dcb1c2f76239b9661c46ff1"
    );
}

#[test]
fn archived_v14_current_fixture_keeps_its_identity() {
    let mut module = fixture();
    module.semantic_version = SemanticVersion::V14;
    assert_eq!(
        semantic_fingerprint(&module).unwrap().to_string(),
        "b11e8ba98262fcdabafac23a4941aa11c18b1293e53758957d839345c01c4fcc"
    );
}

#[test]
fn v11_sum_case_identity_reshuffle_has_stable_canonical_bytes() {
    let mut module = identity_reshuffle_fixture(SemanticVersion::V11);
    let segments = vec![
        ContentPlaceSegment::Case("Present".to_owned()),
        ContentPlaceSegment::Field("payload".to_owned()),
    ];
    module.machines[0].content_identity_reshuffles[0]
        .input
        .segments = segments.clone();
    module.machines[0].content_identity_reshuffles[0]
        .output
        .segments = segments;
    let bytes = encode_module(&module).expect("v11 sum-case reshuffle should encode");

    assert_eq!(decode_module(&bytes), Ok(module.clone()));
    assert_eq!(encode_module(&decode_module(&bytes).unwrap()), Ok(bytes));
    assert_eq!(
        semantic_fingerprint(&module).unwrap().to_string(),
        "9416872934ea26fdf70a5cac07e881fae236cf157cb70f4b90dbb85f2c1102da"
    );
}

#[test]
fn v12_partition_composition_has_stable_canonical_bytes() {
    let module = partition_composition_fixture();
    let bytes = encode_module(&module).expect("v12 partition composition should encode");

    assert_eq!(decode_module(&bytes), Ok(module.clone()));
    assert_eq!(encode_module(&decode_module(&bytes).unwrap()), Ok(bytes));
    assert_eq!(
        semantic_fingerprint(&module).unwrap().to_string(),
        "f05e8d1d90585767fdb80846f8022699286fa85f1eccc8fc2680d5d1d58792ec"
    );

    let migrated = migrate_module_to_current(&module).expect("v12 partition migrates to v14");
    assert_eq!(migrated.machines[0].content_entry_claims.len(), 1);
    assert_eq!(
        migrated.machines[0].content_partition_compositions[0].input_claims,
        vec![claim_id(1)]
    );
}

#[test]
fn v14_entry_claim_has_stable_canonical_bytes() {
    let module = entry_claim_fixture();
    let bytes = encode_module(&module).expect("v14 entry claim should encode");

    assert_eq!(decode_module(&bytes), Ok(module.clone()));
    assert_eq!(encode_module(&decode_module(&bytes).unwrap()), Ok(bytes));
    assert_eq!(
        semantic_fingerprint(&module).unwrap().to_string(),
        "2bc16f09bad47a02b07f524ece5f7880886ab0fd0374e4c30b0ba7612251f67e"
    );
}

#[test]
fn entry_claim_encoding_is_v14_and_canonically_ordered() {
    let mut old = entry_claim_fixture();
    old.semantic_version = SemanticVersion::V13;
    assert!(matches!(
        encode_module(&old),
        Err(CodecError::InvalidModule(
            psi_terminal_verifier::ModuleError::ContentEntryClaimsRequireSemanticVersion {
                required: SemanticVersion::V14,
                actual: SemanticVersion::V13,
                ..
            }
        ))
    ));

    let mut projections = entry_claim_fixture();
    projections.machines[0].content_entry_claims[0]
        .projections
        .swap(0, 1);
    assert_eq!(
        encode_module(&projections),
        Err(CodecError::NonCanonicalOrder(
            "entry-claim content projections by identity and algebra"
        ))
    );
}

#[test]
fn partition_composition_encoding_is_v12_and_canonically_ordered() {
    let mut old = partition_composition_fixture();
    old.semantic_version = SemanticVersion::V11;
    assert!(matches!(
        encode_module(&old),
        Err(CodecError::InvalidModule(
            psi_terminal_verifier::ModuleError::ContentPartitionCompositionsRequireSemanticVersion {
                required: SemanticVersion::V12,
                actual: SemanticVersion::V11,
                ..
            }
        ))
    ));

    let mut substitutions = partition_composition_fixture();
    substitutions.machines[0].content_partition_compositions[0]
        .substitutions
        .swap(0, 1);
    assert_eq!(
        encode_module(&substitutions),
        Err(CodecError::NonCanonicalOrder(
            "partition place substitutions"
        ))
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
fn v10_cannot_encode_sum_case_content_paths() {
    let mut module = identity_reshuffle_fixture(SemanticVersion::V10);
    module.machines[0].content_identity_reshuffles[0]
        .input
        .segments = vec![ContentPlaceSegment::Case("Present".to_owned())];

    assert!(matches!(
        encode_module(&module),
        Err(CodecError::InvalidModule(
            psi_terminal_verifier::ModuleError::ContentIdentityCasePathRequiresSemanticVersion {
                required: SemanticVersion::V11,
                actual: SemanticVersion::V10,
                ..
            }
        ))
    ));
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
        0, 0, 0, 0, // zero crash context maxima
        8, 0, 0, 0, // eight requirements
        1, 2, 3, // Truth, Falsehood, Atom
    ];
    let contract_offset = reordered_requirements
        .windows(contract_prefix.len())
        .position(|window| window == contract_prefix)
        .expect("fixture contract prefix should be unique");
    reordered_requirements.swap(contract_offset + 16, contract_offset + 17);
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
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
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
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
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
                crash_context: Vec::new(),
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

fn bitwise_fixture(kind: u8, scalar_type: ScalarType) -> TerminalModule {
    let left = value_id(12);
    let right = value_id(13);
    let computed = value_id(14);
    let result = value_id(15);
    TerminalModule {
        semantic_version: SemanticVersion::V20,
        entry: machine_id(12),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        machines: vec![TerminalMachine {
            id: machine_id(12),
            parameters: vec![
                ValueDeclaration {
                    id: left,
                    scalar_type,
                },
                ValueDeclaration {
                    id: right,
                    scalar_type,
                },
            ],
            result: ValueDeclaration {
                id: result,
                scalar_type,
            },
            structural_places: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: block_id(12),
            blocks: vec![Block {
                id: block_id(12),
                parameters: Vec::new(),
                operations: vec![Operation {
                    id: operation_id(12),
                    result: ValueDeclaration {
                        id: computed,
                        scalar_type,
                    },
                    kind: match kind {
                        0 => OperationKind::IntegerBitwiseAnd { left, right },
                        1 => OperationKind::IntegerBitwiseOr { left, right },
                        2 => OperationKind::IntegerBitwiseXor { left, right },
                        _ => unreachable!(),
                    },
                }],
                terminator: Terminator::Return {
                    edge: edge_id(12),
                    value: computed,
                },
            }],
            contract: MachineContract {
                id: contract_id(12),
                crash_context: Vec::new(),
                requires: vec![Proposition::Truth],
                ensures: vec![ContractClause {
                    obligation: obligation_id(12),
                    proposition: Proposition::Truth,
                }],
            },
        }],
    }
}

fn wrapping_shift_fixture(left_shift: bool) -> TerminalModule {
    let value_type =
        ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).expect("u8 value type"));
    let count_type =
        ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 16).expect("i16 count type"));
    let value = value_id(12);
    let count = value_id(13);
    let computed = value_id(14);
    let result = value_id(15);
    TerminalModule {
        semantic_version: SemanticVersion::V21,
        entry: machine_id(12),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        machines: vec![TerminalMachine {
            id: machine_id(12),
            parameters: vec![
                ValueDeclaration {
                    id: value,
                    scalar_type: value_type,
                },
                ValueDeclaration {
                    id: count,
                    scalar_type: count_type,
                },
            ],
            result: ValueDeclaration {
                id: result,
                scalar_type: value_type,
            },
            structural_places: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: block_id(12),
            blocks: vec![Block {
                id: block_id(12),
                parameters: Vec::new(),
                operations: vec![Operation {
                    id: operation_id(12),
                    result: ValueDeclaration {
                        id: computed,
                        scalar_type: value_type,
                    },
                    kind: if left_shift {
                        OperationKind::WrappingIntegerShiftLeft { value, count }
                    } else {
                        OperationKind::WrappingIntegerShiftRight { value, count }
                    },
                }],
                terminator: Terminator::Return {
                    edge: edge_id(12),
                    value: computed,
                },
            }],
            contract: MachineContract {
                id: contract_id(12),
                crash_context: Vec::new(),
                requires: vec![Proposition::Truth],
                ensures: vec![ContractClause {
                    obligation: obligation_id(12),
                    proposition: Proposition::Truth,
                }],
            },
        }],
    }
}

fn boolean_fixture(semantic_version: SemanticVersion) -> TerminalModule {
    TerminalModule {
        semantic_version,
        entry: machine_id(10),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        machines: vec![TerminalMachine {
            id: machine_id(10),
            parameters: Vec::new(),
            result: ValueDeclaration {
                id: value_id(11),
                scalar_type: ScalarType::Boolean,
            },
            structural_places: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
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
                crash_context: Vec::new(),
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
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
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
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
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
                crash_context: Vec::new(),
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

fn entry_claim_fixture() -> TerminalModule {
    let mut module = identity_reshuffle_fixture(SemanticVersion::V14);
    let reshuffle = module.machines[0].content_identity_reshuffles[0].clone();
    module.machines[0].content_entry_claims = vec![ContentEntryClaim {
        claim: reshuffle.claim,
        input: reshuffle.input,
        projections: reshuffle.projections,
    }];
    module
}

fn partition_composition_fixture() -> TerminalModule {
    let mut module = identity_reshuffle_fixture(SemanticVersion::V12);
    let machine = &mut module.machines[0];
    machine.content_identity_reshuffles[0]
        .projections
        .truncate(1);
    let projection = machine.content_identity_reshuffles[0].projections[0].projection;
    let algebra = machine.content_identity_reshuffles[0].projections[0]
        .algebra
        .clone();
    let result_root = machine.content_identity_reshuffles[0].output.root;
    let source_input_root = place_id(90);
    let source_result_root = place_id(91);
    let place = |version, root, field: Option<&str>| ContentStructuralPlace {
        version,
        root,
        segments: field
            .into_iter()
            .map(|field| ContentPlaceSegment::Field(field.to_owned()))
            .collect(),
    };
    let term = |subject| ContentTerm::Projection {
        projection,
        subject,
    };
    let source_input = place(ContentPlaceVersion::Entry, source_input_root, None);
    let source_left = place(
        ContentPlaceVersion::Current,
        source_result_root,
        Some("left"),
    );
    let source_right = place(
        ContentPlaceVersion::Current,
        source_result_root,
        Some("right"),
    );
    let target_input = machine.content_identity_reshuffles[0].input.clone();
    let target_left = place(ContentPlaceVersion::Current, result_root, Some("left"));
    let target_right = place(ContentPlaceVersion::Current, result_root, Some("right"));
    machine.content_identity_reshuffles[0].input = target_input.clone();
    machine.content_identity_reshuffles[0].output = target_left.clone();
    let source = ContentConservation::new(
        algebra.clone(),
        term(source_input.clone()),
        ContentTerm::separate([term(source_left.clone()), term(source_right.clone())])
            .expect("source partition"),
    );
    let derived = ContentConservation::new(
        algebra,
        term(target_input.clone()),
        ContentTerm::separate([term(target_left.clone()), term(target_right.clone())])
            .expect("derived partition"),
    );
    let mut substitutions = vec![
        ContentPlaceSubstitution {
            source: source_input,
            target: target_input,
        },
        ContentPlaceSubstitution {
            source: source_left,
            target: target_left,
        },
        ContentPlaceSubstitution {
            source: source_right,
            target: target_right,
        },
    ];
    substitutions.sort();
    machine.content_partition_compositions = vec![ContentPartitionComposition {
        source_fingerprint: 0xfeed_face_dead_beef,
        source_structural_places: vec![
            StructuralPlaceDeclaration {
                id: source_input_root,
                kind: StructuralPlaceKind::Parameter {
                    position: 0,
                    is_self: false,
                },
            },
            StructuralPlaceDeclaration {
                id: source_result_root,
                kind: StructuralPlaceKind::Result,
            },
        ],
        source,
        input_claims: vec![claim_id(1)],
        substitutions,
        derived,
    }];
    module
}

fn wrapping_add_fixture(semantic_version: SemanticVersion) -> TerminalModule {
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let scalar_type = ScalarType::Integer(integer);
    TerminalModule {
        semantic_version,
        entry: machine_id(20),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        machines: vec![TerminalMachine {
            id: machine_id(20),
            parameters: Vec::new(),
            result: ValueDeclaration {
                id: value_id(23),
                scalar_type,
            },
            structural_places: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
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
                crash_context: Vec::new(),
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
