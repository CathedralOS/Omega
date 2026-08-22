use psi_core::{
    BlockId, BoundaryMachineId, ClaimId, ContractId, EdgeId, IntegerSign, IntegerType,
    IntegerValue, MachineId, ObligationId, OperationId, PlaceId, Proposition, ScalarTerm,
    ScalarType, ServiceId, StructuralDomainId, StructuralFieldId, StructuralPlaceKind,
    StructuralTypeId, ValueId,
};
use psi_terminal::{
    BindingRelevance, Block, BoundaryMachineDeclaration, ClaimTransfer, CompletionReceipt,
    EntryClaim, MachineContract, NominalAffineCleanup, Operation, OperationKind, OperationResult,
    ServiceDeclaration, StructuralArgument, StructuralDomainDeclaration,
    StructuralDomainRequirement, StructuralFieldDeclaration, StructuralFieldType,
    StructuralMultiplicity, StructuralParameterDeclaration, StructuralPlaceDeclaration,
    StructuralTypeDeclaration, StructuralTypeShape, SuccessorEdge, TerminalAffineCleanupAction,
    TerminalMachine, TerminalMachineResult, TerminalModule, Terminator, ValueDeclaration,
    VocabularyMarker,
};
use psi_terminal_codec::{
    canonical_proposition_order_key, decode_module, encode_module, semantic_fingerprint,
};
use psi_terminal_verifier::validate_module;

const MATCHING_BYTES: &str = include_str!("fixtures/terminal_ledger_spike.hex");
const ASYMMETRIC_BYTES: &str = include_str!("fixtures/terminal_ledger_spike_asymmetric.hex");
const STRUCTURAL_EFFECT_BYTES: &str =
    include_str!("fixtures/terminal_ledger_structural_effect.hex");
const CALL_COMPOSITION_BYTES: &str = include_str!("fixtures/terminal_ledger_call_composition.hex");

#[test]
fn gamma_ledger_spike_fixtures_are_exact_current_terminal_bytes() {
    for (name, asymmetric, expected) in [
        ("matching", false, MATCHING_BYTES),
        ("asymmetric", true, ASYMMETRIC_BYTES),
    ] {
        let module = ledger_spike_fixture(asymmetric);
        let bytes = encode_module(&module).expect("ledger spike fixture must encode");
        assert_eq!(decode_module(&bytes), Ok(module.clone()));
        assert_eq!(
            encode_module(&decode_module(&bytes).unwrap()),
            Ok(bytes.clone())
        );
        assert_eq!(
            hex(&bytes),
            expected.split_whitespace().collect::<String>(),
            "{name} fixture bytes drifted; reviewed replacement:\n{}",
            wrapped_hex(&bytes)
        );

        let identity = semantic_fingerprint(&module).expect("spike fixture identity");
        assert_ne!(identity.as_bytes(), &[0; 32]);
    }
}

#[test]
fn gamma_structural_effect_ledger_fixture_is_exact_current_terminal_bytes() {
    let module = structural_effect_ledger_fixture();
    validate_module(&module).expect("structural/effect ledger spike must validate");
    let bytes = encode_module(&module).expect("structural/effect ledger spike must encode");
    assert_eq!(decode_module(&bytes), Ok(module.clone()));
    assert_eq!(
        encode_module(&decode_module(&bytes).unwrap()),
        Ok(bytes.clone())
    );
    assert_eq!(
        hex(&bytes),
        STRUCTURAL_EFFECT_BYTES
            .split_whitespace()
            .collect::<String>(),
        "structural/effect fixture bytes drifted; reviewed replacement:\n{}",
        wrapped_hex(&bytes)
    );

    let identity = semantic_fingerprint(&module).expect("structural/effect fixture identity");
    assert_ne!(identity.as_bytes(), &[0; 32]);
}

#[test]
fn gamma_call_composition_ledger_fixture_is_exact_current_terminal_bytes() {
    let module = call_composition_ledger_fixture();
    validate_module(&module).expect("call-composition ledger spike must validate");
    let bytes = encode_module(&module).expect("call-composition ledger spike must encode");
    assert_eq!(decode_module(&bytes), Ok(module.clone()));
    assert_eq!(
        encode_module(&decode_module(&bytes).unwrap()),
        Ok(bytes.clone())
    );
    assert_eq!(
        hex(&bytes),
        CALL_COMPOSITION_BYTES
            .split_whitespace()
            .collect::<String>(),
        "call-composition fixture bytes drifted; reviewed replacement:\n{}",
        wrapped_hex(&bytes)
    );

    let identity = semantic_fingerprint(&module).expect("call-composition fixture identity");
    assert_ne!(identity.as_bytes(), &[0; 32]);
}

fn call_composition_ledger_fixture() -> TerminalModule {
    let resource = structural_type_id(10);
    let pending = structural_domain_id(10);
    let caller_place = place_id(10);
    let callee_place = place_id(20);
    let parameter = |place, position, is_self| StructuralParameterDeclaration {
        place,
        position,
        is_self,
        structural_type: resource,
        multiplicity: StructuralMultiplicity::Linear,
        qualifications: vec![pending],
    };

    let caller = TerminalMachine {
        id: machine_id(10),
        attachment: None,
        parameters: Vec::new(),
        structural_parameters: vec![parameter(caller_place, 0, false)],
        result: TerminalMachineResult::Unit,
        structural_places: vec![StructuralPlaceDeclaration {
            id: caller_place,
            kind: StructuralPlaceKind::Parameter {
                position: 0,
                is_self: false,
            },
        }],
        entry_claims: vec![EntryClaim {
            claim: claim_id(1),
            input: caller_place,
            path: Vec::new(),
        }],
        published_service_ceiling: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: block_id(10),
        blocks: vec![Block {
            id: block_id(10),
            parameters: Vec::new(),
            operations: vec![Operation {
                id: operation_id(10),
                result: OperationResult::Unit,
                kind: OperationKind::CallUnit {
                    callee: machine_id(20),
                    structural_arguments: vec![StructuralArgument {
                        place: caller_place,
                        path: Vec::new(),
                    }],
                    claim_transfers: vec![ClaimTransfer {
                        claim: claim_id(1),
                        argument_index: 0,
                    }],
                    requirement_obligations: Vec::new(),
                    crash_continuations: Vec::new(),
                },
            }],
            terminator: Terminator::ReturnUnit {
                edge: edge_id(10),
                trivial_affine_discards: Vec::new(),
            },
        }],
        contract: MachineContract {
            id: contract_id(10),
            crash_routes: Vec::new(),
            requires: Vec::new(),
            ensures: Vec::new(),
        },
    };

    let callee = TerminalMachine {
        id: machine_id(20),
        attachment: None,
        parameters: Vec::new(),
        structural_parameters: vec![parameter(callee_place, 0, false)],
        result: TerminalMachineResult::Unit,
        structural_places: vec![StructuralPlaceDeclaration {
            id: callee_place,
            kind: StructuralPlaceKind::Parameter {
                position: 0,
                is_self: false,
            },
        }],
        entry_claims: vec![EntryClaim {
            claim: claim_id(1),
            input: callee_place,
            path: Vec::new(),
        }],
        published_service_ceiling: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: block_id(20),
        blocks: vec![Block {
            id: block_id(20),
            parameters: Vec::new(),
            operations: vec![Operation {
                id: operation_id(20),
                result: OperationResult::Unit,
                kind: OperationKind::BoundaryCall {
                    boundary: boundary_machine_id(10),
                    structural_arguments: vec![StructuralArgument {
                        place: callee_place,
                        path: Vec::new(),
                    }],
                    completion_receipts: vec![CompletionReceipt {
                        claim: claim_id(1),
                        argument_index: 0,
                    }],
                    requirement_obligations: Vec::new(),
                },
            }],
            terminator: Terminator::ReturnUnit {
                edge: edge_id(20),
                trivial_affine_discards: Vec::new(),
            },
        }],
        contract: MachineContract {
            id: contract_id(20),
            crash_routes: Vec::new(),
            requires: Vec::new(),
            ensures: Vec::new(),
        },
    };

    TerminalModule {
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: caller.id,
        structural_types: vec![StructuralTypeDeclaration {
            id: resource,
            identity: "Spike::Resource".into(),
            shape: StructuralTypeShape::Record { fields: Vec::new() },
        }],
        structural_domains: vec![StructuralDomainDeclaration {
            id: pending,
            identity: "Spike::Resource::Pending".into(),
            carrier: resource,
        }],
        services: Vec::new(),
        root_service_reach: Default::default(),
        boundary_machines: vec![BoundaryMachineDeclaration {
            id: boundary_machine_id(10),
            identity: "Spike::Resource::settle".into(),
            attachment: Some(resource),
            structural_parameters: vec![parameter(place_id(30), 0, true)],
            result: None,
            requires: vec![StructuralDomainRequirement {
                argument_index: 0,
                domain: pending,
            }],
            published_service_ceiling: Vec::new(),
        }],
        provider_candidates: Vec::new(),
        float_meaning_projections: Vec::new(),
        float_meaning_equalities: Vec::new(),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        evidence_terms: Vec::new(),
        evidence_contract_lanes: Vec::new(),
        evidence_package_invocations: Vec::new(),
        closed_conformance_applications: Vec::new(),
        machines: vec![caller, callee],
    }
}

fn structural_effect_ledger_fixture() -> TerminalModule {
    let boolean_box = StructuralTypeId::new(1).expect("Boolean-box type");
    let empty_record = StructuralTypeId::new(2).expect("empty-record type");
    let input = place_id(1);
    let local = place_id(2);
    let field = structural_field_id(1);
    let service = service_id(1);

    let entry = TerminalMachine {
        id: machine_id(1),
        attachment: None,
        parameters: vec![ValueDeclaration {
            id: value_id(10),
            scalar_type: ScalarType::Boolean,
        }],
        structural_parameters: vec![StructuralParameterDeclaration {
            place: input,
            position: 0,
            is_self: false,
            structural_type: boolean_box,
            multiplicity: StructuralMultiplicity::Affine,
            qualifications: Vec::new(),
        }],
        result: TerminalMachineResult::Scalar(ValueDeclaration {
            id: value_id(11),
            scalar_type: ScalarType::Boolean,
        }),
        structural_places: vec![StructuralPlaceDeclaration {
            id: input,
            kind: StructuralPlaceKind::Parameter {
                position: 0,
                is_self: false,
            },
        }],
        entry_claims: Vec::new(),
        published_service_ceiling: vec![service],
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: block_id(10),
        blocks: vec![Block {
            id: block_id(10),
            parameters: Vec::new(),
            operations: vec![
                scalar_operation(
                    10,
                    ValueDeclaration {
                        id: value_id(12),
                        scalar_type: ScalarType::Boolean,
                    },
                    OperationKind::BooleanStructuralField {
                        source: input,
                        field,
                    },
                ),
                Operation {
                    id: operation_id(11),
                    result: OperationResult::Unit,
                    kind: OperationKind::PortWrite {
                        service,
                        port: 0x3f8,
                        value: 0x4b,
                    },
                },
            ],
            terminator: Terminator::Return {
                edge: edge_id(10),
                value: value_id(12),
                cleanup_actions: vec![TerminalAffineCleanupAction::InvokeNominal(
                    NominalAffineCleanup {
                        place: input,
                        structural_type: boolean_box,
                        cleanup_machine: machine_id(3),
                        cleanup_receiver: None,
                        requirement_obligations: Vec::new(),
                    },
                )],
            },
        }],
        contract: MachineContract {
            id: contract_id(1),
            crash_routes: Vec::new(),
            requires: Vec::new(),
            ensures: Vec::new(),
        },
    };

    let establish = TerminalMachine {
        id: machine_id(2),
        attachment: None,
        parameters: Vec::new(),
        structural_parameters: Vec::new(),
        result: TerminalMachineResult::Unit,
        structural_places: vec![StructuralPlaceDeclaration {
            id: local,
            kind: StructuralPlaceKind::TrivialAffineLocal {
                declaration_ordinal: 0,
                structural_type: empty_record,
            },
        }],
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: block_id(20),
        blocks: vec![Block {
            id: block_id(20),
            parameters: Vec::new(),
            operations: vec![Operation {
                id: operation_id(20),
                result: OperationResult::Unit,
                kind: OperationKind::EstablishTrivialAffineLocal { destination: local },
            }],
            terminator: Terminator::ReturnUnit {
                edge: edge_id(20),
                trivial_affine_discards: vec![local],
            },
        }],
        contract: MachineContract {
            id: contract_id(2),
            crash_routes: Vec::new(),
            requires: Vec::new(),
            ensures: Vec::new(),
        },
    };

    let cleanup = TerminalMachine {
        id: machine_id(3),
        attachment: Some(boolean_box),
        parameters: Vec::new(),
        structural_parameters: Vec::new(),
        result: TerminalMachineResult::Unit,
        structural_places: Vec::new(),
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: block_id(30),
        blocks: vec![Block {
            id: block_id(30),
            parameters: Vec::new(),
            operations: Vec::new(),
            terminator: Terminator::ReturnUnit {
                edge: edge_id(30),
                trivial_affine_discards: Vec::new(),
            },
        }],
        contract: MachineContract {
            id: contract_id(3),
            crash_routes: Vec::new(),
            requires: Vec::new(),
            ensures: Vec::new(),
        },
    };

    TerminalModule {
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: machine_id(1),
        structural_types: vec![
            StructuralTypeDeclaration {
                id: boolean_box,
                identity: "Spike::BooleanBox".into(),
                shape: StructuralTypeShape::Record {
                    fields: vec![StructuralFieldDeclaration {
                        id: field,
                        identity: "flag".into(),
                        relevance: BindingRelevance::Relevant,
                        field_type: StructuralFieldType::Scalar(ScalarType::Boolean),
                    }],
                },
            },
            StructuralTypeDeclaration {
                id: empty_record,
                identity: "Spike::Empty".into(),
                shape: StructuralTypeShape::Record { fields: Vec::new() },
            },
        ],
        structural_domains: Vec::new(),
        services: vec![ServiceDeclaration {
            id: service,
            identity: "PortSpace".into(),
            parents: Vec::new(),
        }],
        root_service_reach: Default::default(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        float_meaning_projections: Vec::new(),
        float_meaning_equalities: Vec::new(),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        evidence_terms: Vec::new(),
        evidence_contract_lanes: Vec::new(),
        evidence_package_invocations: Vec::new(),
        closed_conformance_applications: Vec::new(),
        machines: vec![entry, establish, cleanup],
    }
}

fn ledger_spike_fixture(asymmetric: bool) -> TerminalModule {
    let i8_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let i16_type = IntegerType::new(IntegerSign::Signed, 16).expect("i16");
    let integer = ScalarType::Integer(i8_type);
    let value = |id| ValueDeclaration {
        id: value_id(id),
        scalar_type: integer,
    };
    let boolean = |id| ValueDeclaration {
        id: value_id(id),
        scalar_type: ScalarType::Boolean,
    };
    let i16_value = |id| ValueDeclaration {
        id: value_id(id),
        scalar_type: ScalarType::Integer(i16_type),
    };
    let term = |id| ScalarTerm::value(value_id(id), integer);
    let literal =
        |value| ScalarTerm::integer(i8_type, IntegerValue::Signed(value)).expect("i8 literal");

    let mut callee_requires = vec![
        Proposition::LessOrEqual(literal(-128), term(200)),
        Proposition::LessOrEqual(term(201), literal(127)),
    ];
    callee_requires.sort_by_key(|proposition| {
        canonical_proposition_order_key(proposition).expect("canonical requirement")
    });

    let caller = TerminalMachine {
        id: machine_id(1),
        attachment: None,
        parameters: vec![value(10), value(11), i16_value(12)],
        structural_parameters: Vec::new(),
        result: TerminalMachineResult::Scalar(value(13)),
        structural_places: Vec::new(),
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: block_id(10),
        blocks: vec![
            Block {
                id: block_id(10),
                parameters: Vec::new(),
                operations: vec![
                    scalar_operation(
                        10,
                        value(20),
                        OperationKind::ExactIntegerAdd {
                            left: value_id(10),
                            right: value_id(11),
                            obligation: obligation_id(100),
                        },
                    ),
                    scalar_operation(
                        11,
                        value(21),
                        OperationKind::WrappingIntegerAdd {
                            left: value_id(10),
                            right: value_id(11),
                        },
                    ),
                    scalar_operation(
                        12,
                        value(22),
                        OperationKind::ExactIntegerDivide {
                            left: value_id(10),
                            right: value_id(11),
                            obligation: obligation_id(101),
                        },
                    ),
                    scalar_operation(
                        13,
                        value(23),
                        OperationKind::ExactIntegerRemainder {
                            left: value_id(10),
                            right: value_id(11),
                            obligation: obligation_id(102),
                        },
                    ),
                    scalar_operation(
                        14,
                        value(24),
                        OperationKind::WrappingIntegerDivide {
                            left: value_id(10),
                            right: value_id(11),
                            obligation: obligation_id(103),
                        },
                    ),
                    scalar_operation(
                        15,
                        value(25),
                        OperationKind::WrappingIntegerRemainder {
                            left: value_id(10),
                            right: value_id(11),
                            obligation: obligation_id(104),
                        },
                    ),
                    scalar_operation(
                        16,
                        boolean(26),
                        OperationKind::IntegerLessThan {
                            left: value_id(10),
                            right: value_id(11),
                        },
                    ),
                    scalar_operation(
                        17,
                        value(27),
                        OperationKind::Call {
                            callee: machine_id(2),
                            arguments: vec![value_id(10), value_id(11)],
                            requirement_obligations: vec![obligation_id(105), obligation_id(106)],
                            crash_continuations: Vec::new(),
                        },
                    ),
                ],
                terminator: Terminator::Conditional {
                    condition: value_id(26),
                    when_true: successor(100, 11),
                    when_false: successor(101, 12),
                },
            },
            Block {
                id: block_id(11),
                parameters: Vec::new(),
                operations: Vec::new(),
                terminator: Terminator::Jump {
                    edge: edge_id(102),
                    target: block_id(13),
                    arguments: vec![value_id(20)],
                    trivial_affine_discards: Vec::new(),
                },
            },
            Block {
                id: block_id(12),
                parameters: Vec::new(),
                operations: Vec::new(),
                terminator: Terminator::Jump {
                    edge: edge_id(103),
                    target: block_id(13),
                    arguments: vec![value_id(if asymmetric { 21 } else { 20 })],
                    trivial_affine_discards: Vec::new(),
                },
            },
            Block {
                id: block_id(13),
                parameters: vec![value(30)],
                operations: vec![
                    scalar_operation(
                        18,
                        value(32),
                        OperationKind::ExactIntegerSubtract {
                            left: value_id(10),
                            right: value_id(11),
                            obligation: obligation_id(107),
                        },
                    ),
                    scalar_operation(
                        19,
                        value(33),
                        OperationKind::WrappingIntegerSubtract {
                            left: value_id(10),
                            right: value_id(11),
                        },
                    ),
                    scalar_operation(
                        20,
                        value(34),
                        OperationKind::SaturatingIntegerSubtract {
                            left: value_id(10),
                            right: value_id(11),
                        },
                    ),
                    scalar_operation(
                        21,
                        value(35),
                        OperationKind::ExactIntegerMultiply {
                            left: value_id(10),
                            right: value_id(11),
                            obligation: obligation_id(108),
                        },
                    ),
                    scalar_operation(
                        22,
                        value(36),
                        OperationKind::WrappingIntegerMultiply {
                            left: value_id(10),
                            right: value_id(11),
                        },
                    ),
                    scalar_operation(
                        23,
                        value(37),
                        OperationKind::SaturatingIntegerMultiply {
                            left: value_id(10),
                            right: value_id(11),
                        },
                    ),
                    scalar_operation(
                        24,
                        value(38),
                        OperationKind::SaturatingIntegerAdd {
                            left: value_id(10),
                            right: value_id(11),
                        },
                    ),
                    scalar_operation(
                        25,
                        value(31),
                        OperationKind::WrappingIntegerAdd {
                            left: value_id(30),
                            right: value_id(11),
                        },
                    ),
                    scalar_operation(
                        26,
                        value(39),
                        OperationKind::SaturatingIntegerDivide {
                            left: value_id(10),
                            right: value_id(11),
                            obligation: obligation_id(109),
                        },
                    ),
                    scalar_operation(
                        27,
                        value(40),
                        OperationKind::SaturatingIntegerRemainder {
                            left: value_id(10),
                            right: value_id(11),
                            obligation: obligation_id(110),
                        },
                    ),
                    scalar_operation(
                        28,
                        value(41),
                        OperationKind::IntegerConstant {
                            value: IntegerValue::Signed(5),
                        },
                    ),
                    scalar_operation(
                        29,
                        boolean(42),
                        OperationKind::BooleanConstant { value: true },
                    ),
                    scalar_operation(
                        30,
                        boolean(43),
                        OperationKind::BooleanNot {
                            operand: value_id(42),
                        },
                    ),
                    scalar_operation(
                        31,
                        boolean(44),
                        OperationKind::BooleanEqual {
                            left: value_id(42),
                            right: value_id(43),
                        },
                    ),
                    scalar_operation(
                        32,
                        boolean(45),
                        OperationKind::IntegerEqual {
                            left: value_id(10),
                            right: value_id(41),
                        },
                    ),
                    scalar_operation(
                        33,
                        boolean(46),
                        OperationKind::IntegerLessOrEqual {
                            left: value_id(10),
                            right: value_id(41),
                        },
                    ),
                    scalar_operation(
                        34,
                        i16_value(47),
                        OperationKind::IntegerWiden {
                            operand: value_id(10),
                        },
                    ),
                    scalar_operation(
                        35,
                        value(48),
                        OperationKind::IntegerExactCast {
                            operand: value_id(12),
                            obligation: obligation_id(111),
                        },
                    ),
                    scalar_operation(
                        36,
                        value(49),
                        OperationKind::IntegerBitwiseNot {
                            operand: value_id(10),
                        },
                    ),
                    scalar_operation(
                        37,
                        value(50),
                        OperationKind::IntegerBitwiseAnd {
                            left: value_id(10),
                            right: value_id(11),
                        },
                    ),
                    scalar_operation(
                        38,
                        value(51),
                        OperationKind::IntegerBitwiseOr {
                            left: value_id(10),
                            right: value_id(11),
                        },
                    ),
                    scalar_operation(
                        39,
                        value(52),
                        OperationKind::IntegerBitwiseXor {
                            left: value_id(10),
                            right: value_id(11),
                        },
                    ),
                    scalar_operation(
                        40,
                        value(53),
                        OperationKind::WrappingIntegerShiftLeft {
                            value: value_id(10),
                            count: value_id(12),
                        },
                    ),
                    scalar_operation(
                        41,
                        value(54),
                        OperationKind::WrappingIntegerShiftRight {
                            value: value_id(10),
                            count: value_id(12),
                        },
                    ),
                    scalar_operation(
                        42,
                        value(55),
                        OperationKind::ExactIntegerShiftLeft {
                            value: value_id(10),
                            count: value_id(12),
                            obligation: obligation_id(112),
                        },
                    ),
                    scalar_operation(
                        43,
                        value(56),
                        OperationKind::ExactIntegerShiftRight {
                            value: value_id(10),
                            count: value_id(12),
                            obligation: obligation_id(113),
                        },
                    ),
                ],
                terminator: Terminator::Return {
                    edge: edge_id(104),
                    value: value_id(31),
                    cleanup_actions: Vec::new(),
                },
            },
        ],
        contract: MachineContract {
            id: contract_id(1),
            crash_routes: Vec::new(),
            requires: Vec::new(),
            ensures: Vec::new(),
        },
    };

    let callee = TerminalMachine {
        id: machine_id(2),
        attachment: None,
        parameters: vec![value(200), value(201)],
        structural_parameters: Vec::new(),
        result: TerminalMachineResult::Scalar(value(202)),
        structural_places: Vec::new(),
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: block_id(20),
        blocks: vec![Block {
            id: block_id(20),
            parameters: Vec::new(),
            operations: Vec::new(),
            terminator: Terminator::Return {
                edge: edge_id(200),
                value: value_id(200),
                cleanup_actions: Vec::new(),
            },
        }],
        contract: MachineContract {
            id: contract_id(2),
            crash_routes: Vec::new(),
            requires: callee_requires,
            ensures: Vec::new(),
        },
    };

    TerminalModule {
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: machine_id(1),
        structural_types: Vec::new(),
        structural_domains: Vec::new(),
        services: Vec::new(),
        root_service_reach: Default::default(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        float_meaning_projections: Vec::new(),
        float_meaning_equalities: Vec::new(),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        evidence_terms: Vec::new(),
        evidence_contract_lanes: Vec::new(),
        evidence_package_invocations: Vec::new(),
        closed_conformance_applications: Vec::new(),
        machines: vec![caller, callee],
    }
}

fn scalar_operation(id: u64, result: ValueDeclaration, kind: OperationKind) -> Operation {
    Operation {
        id: operation_id(id),
        result: OperationResult::Scalar(result),
        kind,
    }
}

fn successor(edge: u64, target: u64) -> SuccessorEdge {
    SuccessorEdge {
        edge: edge_id(edge),
        target: block_id(target),
        arguments: Vec::new(),
        trivial_affine_discards: Vec::new(),
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn wrapped_hex(bytes: &[u8]) -> String {
    let hex = hex(bytes);
    hex.as_bytes()
        .chunks(96)
        .map(|chunk| std::str::from_utf8(chunk).expect("hex is UTF-8"))
        .collect::<Vec<_>>()
        .join("\n")
}

macro_rules! id_constructor {
    ($function:ident, $type:ty) => {
        fn $function(raw: u64) -> $type {
            <$type>::new(raw).expect("spike identities are nonzero")
        }
    };
}

id_constructor!(value_id, ValueId);
id_constructor!(machine_id, MachineId);
id_constructor!(block_id, BlockId);
id_constructor!(operation_id, OperationId);
id_constructor!(edge_id, EdgeId);
id_constructor!(contract_id, ContractId);
id_constructor!(obligation_id, ObligationId);
id_constructor!(place_id, PlaceId);
id_constructor!(service_id, ServiceId);
id_constructor!(structural_type_id, StructuralTypeId);
id_constructor!(structural_field_id, StructuralFieldId);
id_constructor!(structural_domain_id, StructuralDomainId);
id_constructor!(boundary_machine_id, BoundaryMachineId);
id_constructor!(claim_id, ClaimId);
