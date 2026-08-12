use psi_core::{
    BlockId, BoundaryMachineId, ContractId, EdgeId, IntegerSign, IntegerType, IntegerValue,
    MachineId, ObligationId, OperationId, Proposition, ScalarTerm, ScalarType, ServiceId, ValueId,
};
use psi_proof_kernel::{AdmissionProfile, EvidenceRoute, PrimitiveJudgment};
use psi_terminal::{
    Block, BoundaryMachineDeclaration, ContractClause, CrashCause, CrashRouteBucket,
    CrashRouteGuard, MachineContract, Operation, OperationKind, OperationResult,
    ServiceDeclaration, SuccessorEdge, TerminalMachine, TerminalMachineResult, TerminalModule,
    Terminator, ValueDeclaration, VocabularyMarker,
};
use psi_terminal_codec::{CodecError, decode_module, encode_module, terminal_psi_identity};
use psi_terminal_fixed_fuel::{
    FixedFuelError, derive_fixed_entry_fuel, derive_fixed_safe_point_segments,
    derive_fixed_segment_fuel, validate_fixed_entry_fuel, validate_fixed_safe_point_segments,
    validate_fixed_segment_fuel,
};
use psi_terminal_verifier::{ObligationEvidence, ProofBundle, verify_module};

#[test]
fn straight_line_entry_has_an_exact_recomputable_bound() {
    let (module, proof) = fixture();
    let verified = verify_module(&module, &proof, &AdmissionProfile::default()).unwrap();
    let certificate = derive_fixed_entry_fuel(&verified, machine_id(1)).unwrap();

    assert_eq!(
        certificate.terminal_psi(),
        terminal_psi_identity(&module).unwrap()
    );
    assert_eq!(certificate.schedule().marker(), 1);
    assert_eq!(certificate.entry(), machine_id(1));
    assert!(certificate.relevant_preconditions().is_empty());
    assert_eq!(certificate.ceiling_units(), 3);
    validate_fixed_entry_fuel(&verified, &certificate).unwrap();

    let bytes = encode_module(&module).unwrap();
    drop(verified);
    drop(module);
    let decoded = decode_module(&bytes).unwrap();
    let independently_verified =
        verify_module(&decoded, &proof, &AdmissionProfile::default()).unwrap();
    validate_fixed_entry_fuel(&independently_verified, &certificate).unwrap();
}

#[test]
fn unit_return_is_one_normal_edge_unit() {
    let module = unit_fixture();
    let verified = verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("unit module verifies");
    let certificate = derive_fixed_entry_fuel(&verified, machine_id(900))
        .expect("unit return has an exact fixed bound");

    assert_eq!(certificate.ceiling_units(), 1);
    validate_fixed_entry_fuel(&verified, &certificate).unwrap();
    let segments = derive_fixed_safe_point_segments(&verified, machine_id(900)).unwrap();
    assert_eq!(segments.len(), 1);
    assert_eq!(segments[0].end_edge(), edge_id(900));
    assert_eq!(segments[0].ceiling_units(), 1);
}

#[test]
fn semantic_mutation_invalidates_the_old_certificate_without_changing_cost() {
    let (module, proof) = fixture();
    let verified = verify_module(&module, &proof, &AdmissionProfile::default()).unwrap();
    let certificate = derive_fixed_entry_fuel(&verified, machine_id(1)).unwrap();

    let mut changed = module.clone();
    changed.machines[0].blocks[0].operations[0].kind = OperationKind::IntegerConstant {
        value: IntegerValue::Signed(8),
    };
    let changed_verified = verify_module(&changed, &proof, &AdmissionProfile::default()).unwrap();
    let changed_certificate = derive_fixed_entry_fuel(&changed_verified, machine_id(1)).unwrap();
    assert_ne!(
        changed_certificate.terminal_psi(),
        certificate.terminal_psi()
    );
    assert_eq!(
        changed_certificate.ceiling_units(),
        certificate.ceiling_units()
    );
    assert_eq!(
        validate_fixed_entry_fuel(&changed_verified, &certificate),
        Err(FixedFuelError::CertificateMismatch)
    );
}

#[test]
fn certificate_derivation_requires_canonical_semantic_identity() {
    let (mut module, proof) = fixture();
    module.machines[0].blocks.swap(0, 1);
    let verified = verify_module(&module, &proof, &AdmissionProfile::default()).unwrap();
    assert_eq!(
        derive_fixed_entry_fuel(&verified, machine_id(1)),
        Err(FixedFuelError::SemanticIdentity(
            CodecError::NonCanonicalOrder("blocks by BlockId")
        ))
    );
}

#[test]
fn selected_segments_include_their_exact_terminal_edge() {
    let (module, proof) = fixture();
    let verified = verify_module(&module, &proof, &AdmissionProfile::default()).unwrap();

    let entry_to_jump =
        derive_fixed_segment_fuel(&verified, machine_id(1), block_id(1), edge_id(1)).unwrap();
    assert_eq!(
        entry_to_jump.terminal_psi(),
        terminal_psi_identity(&module).unwrap()
    );
    assert_eq!(entry_to_jump.schedule().marker(), 1);
    assert_eq!(entry_to_jump.machine(), machine_id(1));
    assert_eq!(entry_to_jump.start_block(), block_id(1));
    assert_eq!(entry_to_jump.end_edge(), edge_id(1));
    assert!(entry_to_jump.relevant_preconditions().is_empty());
    assert_eq!(entry_to_jump.ceiling_units(), 2);
    validate_fixed_segment_fuel(&verified, &entry_to_jump).unwrap();

    let jump_target_to_return =
        derive_fixed_segment_fuel(&verified, machine_id(1), block_id(2), edge_id(2)).unwrap();
    assert_eq!(jump_target_to_return.ceiling_units(), 1);
    validate_fixed_segment_fuel(&verified, &jump_target_to_return).unwrap();
}

#[test]
fn a_segment_cannot_cross_the_reached_return_to_find_an_unrelated_edge() {
    let (module, proof) = fixture();
    let verified = verify_module(&module, &proof, &AdmissionProfile::default()).unwrap();

    assert_eq!(
        derive_fixed_segment_fuel(&verified, machine_id(1), block_id(2), edge_id(1)),
        Err(FixedFuelError::SegmentEndNotReached {
            requested: edge_id(1),
            reached_terminal: edge_id(2),
        })
    );
}

#[test]
fn crash_is_an_explicit_fixed_fuel_terminal_edge() {
    let (mut module, _) = fixture();
    module.machines[0].contract.crash_routes = vec![psi_terminal::CrashRouteBucket {
        cause: CrashCause::Abort,
        alternatives: vec![psi_terminal::CrashRouteGuard::Truth],
    }];
    module.machines[0].contract.ensures.clear();
    module.machines[0].blocks[1].terminator = Terminator::Crash {
        edge: edge_id(2),
        cause: CrashCause::Abort,
        site_guard: Vec::new(),
        frontier_lower_bound: Vec::new(),
    };
    let verified = verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("crash-ending module verifies");

    let certificate = derive_fixed_entry_fuel(&verified, machine_id(1))
        .expect("crash is a terminal path for fixed fuel");
    assert_eq!(certificate.ceiling_units(), 3);
    validate_fixed_entry_fuel(&verified, &certificate).expect("crash certificate recomputes");

    let segments = derive_fixed_safe_point_segments(&verified, machine_id(1))
        .expect("crash edge closes the final segment");
    assert_eq!(segments.len(), 2);
    assert_eq!(segments[1].end_edge(), edge_id(2));
    assert_eq!(segments[1].ceiling_units(), 1);
}

#[test]
fn calls_include_the_complete_callee_bound() {
    let module = call_fixture();
    let verified = verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("call module verifies");
    let certificate = derive_fixed_entry_fuel(&verified, machine_id(1))
        .expect("acyclic call graph has a fixed bound");
    assert_eq!(certificate.ceiling_units(), 4);
    validate_fixed_entry_fuel(&verified, &certificate).expect("call bound recomputes");
}

#[test]
fn unit_calls_and_effect_operations_use_the_same_transitive_schedule() {
    let module = unit_effect_fixture();
    let verified = verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("Unit/effect module verifies");
    let certificate = derive_fixed_entry_fuel(&verified, machine_id(700))
        .expect("acyclic Unit call has an exact fixed bound");

    assert_eq!(certificate.ceiling_units(), 5);
    validate_fixed_entry_fuel(&verified, &certificate).unwrap();
    let segments = derive_fixed_safe_point_segments(&verified, machine_id(700)).unwrap();
    assert_eq!(segments.len(), 1);
    assert_eq!(segments[0].ceiling_units(), 5);
}

#[test]
fn an_all_crash_callee_excludes_the_unreachable_caller_tail() {
    let mut module = call_fixture();
    let route = CrashRouteBucket {
        cause: CrashCause::Trap,
        alternatives: vec![CrashRouteGuard::Truth],
    };
    module.machines[0].contract.crash_routes = vec![route.clone()];
    let OperationKind::Call {
        crash_continuations,
        ..
    } = &mut module.machines[0].blocks[0].operations[1].kind
    else {
        unreachable!()
    };
    *crash_continuations = vec![route.clone()];
    module.machines[1].contract.crash_routes = vec![route];
    module.machines[1].blocks[0].terminator = Terminator::Crash {
        edge: edge_id(2),
        cause: CrashCause::Trap,
        site_guard: Vec::new(),
        frontier_lower_bound: Vec::new(),
    };
    let verified = verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("all-crash call module verifies");

    assert_eq!(
        derive_fixed_entry_fuel(&verified, machine_id(1))
            .expect("all-crash call has an exact bound")
            .ceiling_units(),
        3,
        "the caller return edge is unreachable after the callee crash"
    );
    assert_eq!(
        derive_fixed_segment_fuel(&verified, machine_id(1), block_id(1), edge_id(1)),
        Err(FixedFuelError::SegmentEndUnreachableAfterCall {
            block: block_id(1),
            callee: machine_id(2),
        })
    );
    assert!(
        derive_fixed_safe_point_segments(&verified, machine_id(1))
            .expect("the caller has no reachable machine-local edge")
            .is_empty()
    );
}

#[test]
fn mixed_call_outcomes_do_not_cross_product_crash_and_caller_return_costs() {
    let mut module = call_fixture();
    let route = CrashRouteBucket {
        cause: CrashCause::Abort,
        alternatives: vec![CrashRouteGuard::Truth],
    };
    module.machines[0].contract.crash_routes = vec![route.clone()];
    let OperationKind::Call {
        crash_continuations,
        ..
    } = &mut module.machines[0].blocks[0].operations[1].kind
    else {
        unreachable!()
    };
    *crash_continuations = vec![route.clone()];

    let callee = &mut module.machines[1];
    callee.contract.crash_routes = vec![route];
    callee.blocks = vec![
        Block {
            id: block_id(2),
            parameters: Vec::new(),
            operations: Vec::new(),
            terminator: Terminator::Conditional {
                condition: value_id(4),
                when_true: SuccessorEdge {
                    edge: edge_id(2),
                    target: block_id(3),
                    arguments: Vec::new(),
                },
                when_false: SuccessorEdge {
                    edge: edge_id(3),
                    target: block_id(4),
                    arguments: Vec::new(),
                },
            },
        },
        Block {
            id: block_id(3),
            parameters: Vec::new(),
            operations: Vec::new(),
            terminator: Terminator::Return {
                trivial_affine_discards: Vec::new(),
                edge: edge_id(4),
                value: value_id(4),
            },
        },
        Block {
            id: block_id(4),
            parameters: Vec::new(),
            operations: vec![
                Operation {
                    id: operation_id(3),
                    result: psi_terminal::OperationResult::Scalar(ValueDeclaration {
                        id: value_id(6),
                        scalar_type: ScalarType::Boolean,
                    }),
                    kind: OperationKind::BooleanConstant { value: false },
                },
                Operation {
                    id: operation_id(4),
                    result: psi_terminal::OperationResult::Scalar(ValueDeclaration {
                        id: value_id(7),
                        scalar_type: ScalarType::Boolean,
                    }),
                    kind: OperationKind::BooleanConstant { value: false },
                },
            ],
            terminator: Terminator::Crash {
                edge: edge_id(5),
                cause: CrashCause::Abort,
                site_guard: Vec::new(),
                frontier_lower_bound: Vec::new(),
            },
        },
    ];
    let verified = verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("mixed call-outcome module verifies");

    assert_eq!(
        derive_fixed_entry_fuel(&verified, machine_id(1))
            .expect("mixed call outcomes have an exact bound")
            .ceiling_units(),
        6,
        "the four-unit crash path must not be followed by the caller return"
    );
    assert_eq!(
        derive_fixed_segment_fuel(&verified, machine_id(1), block_id(1), edge_id(1))
            .expect("the caller return edge remains reachable")
            .ceiling_units(),
        5,
        "the caller segment composes with the callee's two-unit return path"
    );
}

#[test]
fn safe_point_selection_covers_the_complete_ordered_path() {
    let (module, proof) = fixture();
    let verified = verify_module(&module, &proof, &AdmissionProfile::default()).unwrap();

    let segments = derive_fixed_safe_point_segments(&verified, machine_id(1)).unwrap();
    assert_eq!(segments.len(), 2);
    assert_eq!(segments[0].start_block(), block_id(1));
    assert_eq!(segments[0].end_edge(), edge_id(1));
    assert_eq!(segments[0].ceiling_units(), 2);
    assert_eq!(segments[1].start_block(), block_id(2));
    assert_eq!(segments[1].end_edge(), edge_id(2));
    assert_eq!(segments[1].ceiling_units(), 1);
    validate_fixed_safe_point_segments(&verified, machine_id(1), &segments).unwrap();

    assert_eq!(
        validate_fixed_safe_point_segments(&verified, machine_id(1), &segments[..1]),
        Err(FixedFuelError::CertificateMismatch),
        "a producer cannot omit the final terminal segment"
    );
    let mut reordered = segments;
    reordered.reverse();
    assert_eq!(
        validate_fixed_safe_point_segments(&verified, machine_id(1), &reordered),
        Err(FixedFuelError::CertificateMismatch),
        "a producer cannot reorder semantic safe-point segments"
    );
}

fn unit_fixture() -> TerminalModule {
    TerminalModule {
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: machine_id(900),
        structural_types: Vec::new(),
        structural_domains: Vec::new(),
        services: Vec::new(),
        boundary_machines: Vec::new(),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        machines: vec![TerminalMachine {
            id: machine_id(900),
            attachment: None,
            structural_parameters: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            parameters: Vec::new(),
            result: TerminalMachineResult::Unit,
            structural_places: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: block_id(900),
            blocks: vec![Block {
                id: block_id(900),
                parameters: Vec::new(),
                operations: Vec::new(),
                terminator: Terminator::ReturnUnit {
                    edge: edge_id(900),
                    trivial_affine_discards: Vec::new(),
                },
            }],
            contract: MachineContract {
                id: contract_id(900),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
            },
        }],
    }
}

fn unit_effect_fixture() -> TerminalModule {
    let service = service_id(1);
    TerminalModule {
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: machine_id(700),
        structural_types: Vec::new(),
        structural_domains: Vec::new(),
        services: vec![ServiceDeclaration {
            id: service,
            identity: "test::PortIo".into(),
            parents: Vec::new(),
        }],
        boundary_machines: vec![BoundaryMachineDeclaration {
            id: boundary_id(1),
            identity: "test::boundary".into(),
            attachment: None,
            structural_parameters: Vec::new(),
            requires: Vec::new(),
            published_service_ceiling: Vec::new(),
        }],
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        machines: vec![
            TerminalMachine {
                id: machine_id(700),
                attachment: None,
                parameters: Vec::new(),
                structural_parameters: Vec::new(),
                result: TerminalMachineResult::Unit,
                structural_places: Vec::new(),
                entry_claims: Vec::new(),
                published_service_ceiling: vec![service],
                content_entry_claims: Vec::new(),
                content_identity_reshuffles: Vec::new(),
                content_partition_compositions: Vec::new(),
                entry: block_id(700),
                blocks: vec![Block {
                    id: block_id(700),
                    parameters: Vec::new(),
                    operations: vec![
                        Operation {
                            id: operation_id(700),
                            result: OperationResult::Unit,
                            kind: OperationKind::CallUnit {
                                callee: machine_id(701),
                                structural_arguments: Vec::new(),
                                claim_transfers: Vec::new(),
                                requirement_obligations: Vec::new(),
                                crash_continuations: Vec::new(),
                            },
                        },
                        Operation {
                            id: operation_id(701),
                            result: OperationResult::Unit,
                            kind: OperationKind::PortWrite {
                                service,
                                port: 0x20,
                                value: 0x20,
                            },
                        },
                    ],
                    terminator: Terminator::ReturnUnit {
                        edge: edge_id(700),
                        trivial_affine_discards: Vec::new(),
                    },
                }],
                contract: MachineContract {
                    id: contract_id(700),
                    crash_routes: Vec::new(),
                    requires: Vec::new(),
                    ensures: Vec::new(),
                },
            },
            TerminalMachine {
                id: machine_id(701),
                attachment: None,
                parameters: Vec::new(),
                structural_parameters: Vec::new(),
                result: TerminalMachineResult::Unit,
                structural_places: Vec::new(),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                content_entry_claims: Vec::new(),
                content_identity_reshuffles: Vec::new(),
                content_partition_compositions: Vec::new(),
                entry: block_id(701),
                blocks: vec![Block {
                    id: block_id(701),
                    parameters: Vec::new(),
                    operations: vec![Operation {
                        id: operation_id(702),
                        result: OperationResult::Unit,
                        kind: OperationKind::BoundaryCallUnit {
                            boundary: boundary_id(1),
                            structural_arguments: Vec::new(),
                            completion_receipts: Vec::new(),
                            requirement_obligations: Vec::new(),
                        },
                    }],
                    terminator: Terminator::ReturnUnit {
                        edge: edge_id(701),
                        trivial_affine_discards: Vec::new(),
                    },
                }],
                contract: MachineContract {
                    id: contract_id(701),
                    crash_routes: Vec::new(),
                    requires: Vec::new(),
                    ensures: Vec::new(),
                },
            },
        ],
    }
}

fn fixture() -> (TerminalModule, ProofBundle) {
    let integer = IntegerType::new(IntegerSign::Signed, 32).unwrap();
    let scalar_type = ScalarType::Integer(integer);
    let literal = ScalarTerm::integer(integer, IntegerValue::Signed(7)).unwrap();
    let goal = Proposition::Equal(literal.clone(), literal);
    let obligation = obligation_id(1);
    let module = TerminalModule {
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: machine_id(1),
        structural_types: Vec::new(),
        structural_domains: Vec::new(),
        services: Vec::new(),
        boundary_machines: Vec::new(),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        machines: vec![TerminalMachine {
            id: machine_id(1),
            attachment: None,
            structural_parameters: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            parameters: Vec::new(),
            result: TerminalMachineResult::Scalar(ValueDeclaration {
                id: value_id(3),
                scalar_type,
            }),
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
                        result: psi_terminal::OperationResult::Scalar(ValueDeclaration {
                            id: value_id(1),
                            scalar_type,
                        }),
                        kind: OperationKind::IntegerConstant {
                            value: IntegerValue::Signed(7),
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
                    operations: Vec::new(),
                    terminator: Terminator::Return {
                        trivial_affine_discards: Vec::new(),
                        edge: edge_id(2),
                        value: value_id(2),
                    },
                },
            ],
            contract: MachineContract {
                id: contract_id(1),
                crash_routes: Vec::new(),
                requires: vec![goal.clone()],
                ensures: vec![ContractClause {
                    obligation,
                    proposition: goal,
                }],
            },
        }],
    };
    let proof = ProofBundle {
        evidence: vec![ObligationEvidence {
            obligation,
            route: EvidenceRoute::KernelDerived(PrimitiveJudgment::ClosedIntegerRelation),
        }],
    };
    (module, proof)
}

fn call_fixture() -> TerminalModule {
    let boolean = ScalarType::Boolean;
    let declaration = |raw| ValueDeclaration {
        id: value_id(raw),
        scalar_type: boolean,
    };
    let empty_contract = |raw| MachineContract {
        id: contract_id(raw),
        crash_routes: Vec::new(),
        requires: Vec::new(),
        ensures: Vec::new(),
    };
    TerminalModule {
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: machine_id(1),
        structural_types: Vec::new(),
        structural_domains: Vec::new(),
        services: Vec::new(),
        boundary_machines: Vec::new(),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        machines: vec![
            TerminalMachine {
                id: machine_id(1),
                attachment: None,
                structural_parameters: Vec::new(),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                parameters: Vec::new(),
                result: TerminalMachineResult::Scalar(declaration(3)),
                structural_places: Vec::new(),
                content_entry_claims: Vec::new(),
                content_identity_reshuffles: Vec::new(),
                content_partition_compositions: Vec::new(),
                entry: block_id(1),
                blocks: vec![Block {
                    id: block_id(1),
                    parameters: Vec::new(),
                    operations: vec![
                        Operation {
                            id: operation_id(1),
                            result: psi_terminal::OperationResult::Scalar(declaration(1)),
                            kind: OperationKind::BooleanConstant { value: true },
                        },
                        Operation {
                            id: operation_id(2),
                            result: psi_terminal::OperationResult::Scalar(declaration(2)),
                            kind: OperationKind::Call {
                                callee: machine_id(2),
                                arguments: vec![value_id(1)],
                                requirement_obligations: Vec::new(),
                                crash_continuations: Vec::new(),
                            },
                        },
                    ],
                    terminator: Terminator::Return {
                        trivial_affine_discards: Vec::new(),
                        edge: edge_id(1),
                        value: value_id(2),
                    },
                }],
                contract: empty_contract(1),
            },
            TerminalMachine {
                id: machine_id(2),
                attachment: None,
                structural_parameters: Vec::new(),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                parameters: vec![declaration(4)],
                result: TerminalMachineResult::Scalar(declaration(5)),
                structural_places: Vec::new(),
                content_entry_claims: Vec::new(),
                content_identity_reshuffles: Vec::new(),
                content_partition_compositions: Vec::new(),
                entry: block_id(2),
                blocks: vec![Block {
                    id: block_id(2),
                    parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: Terminator::Return {
                        trivial_affine_discards: Vec::new(),
                        edge: edge_id(2),
                        value: value_id(4),
                    },
                }],
                contract: empty_contract(2),
            },
        ],
    }
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
id_constructor!(operation_id, OperationId);
id_constructor!(edge_id, EdgeId);
id_constructor!(contract_id, ContractId);
id_constructor!(obligation_id, ObligationId);
id_constructor!(boundary_id, BoundaryMachineId);
id_constructor!(service_id, ServiceId);
