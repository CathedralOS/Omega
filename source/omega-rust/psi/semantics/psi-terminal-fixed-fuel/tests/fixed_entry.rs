use psi_core::{
    BlockId, BoundaryMachineId, ClaimId, ContractId, EdgeId, EvidenceIdentity, IntegerSign,
    IntegerType, IntegerValue, MachineId, ObligationId, OperationId, PlaceId, Proposition,
    ScalarTerm, ScalarType, ServiceId, StructuralCaseId, StructuralTypeId, ValueId,
};
use psi_proof_admission::{
    AdmissionProfile, CertificateEnvelope, EvidenceRoute, PrimitiveJudgment, ProofNode, ProofRule,
    ProofSystemMarker,
};
use psi_terminal::{
    Block, BoundaryMachineDeclaration, ClaimTransfer, CompletionReceipt, ContractClause,
    CrashCause, CrashRouteBucket, CrashRouteGuard, EntryClaim, MachineContract,
    NominalAffineCleanup, Operation, OperationKind, OperationResult, ServiceDeclaration,
    StructuralAccess, StructuralArgument, StructuralFieldDeclaration, StructuralFieldType,
    StructuralMultiplicity, StructuralOperationResult, StructuralParameterDeclaration,
    StructuralPathSegment, StructuralPlaceDeclaration, StructuralResultDeclaration,
    StructuralTypeDeclaration, StructuralTypeShape, SuccessorEdge, TerminalAffineCleanupAction,
    TerminalMachine, TerminalMachineResult, TerminalModule, Terminator, ValueDeclaration,
    VocabularyMarker,
};
use psi_terminal_codec::{CodecError, decode_module, encode_module, terminal_psi_identity};
use psi_terminal_fixed_fuel::{
    FixedFuelError, derive_fixed_entry_fuel, derive_fixed_safe_point_segments,
    derive_fixed_segment_fuel, derive_validated_fixed_safe_point_segments,
    retain_validated_fixed_safe_point_segments, validate_fixed_entry_fuel,
    validate_fixed_safe_point_segments, validate_fixed_segment_fuel,
    validate_retained_fixed_safe_point_segments,
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
fn payloadless_case_operation_adds_one_fixed_fuel_unit() {
    let structural_type = structural_type_id(910);
    let result_case = structural_case_id(910);
    let operation_place = place_id(910);
    let result_place = place_id(911);
    let mut module = unit_fixture();
    module.structural_types = vec![StructuralTypeDeclaration {
        id: structural_type,
        identity: "test::Outcome".into(),
        shape: StructuralTypeShape::Sum {
            cases: vec![psi_terminal::StructuralCaseDeclaration {
                id: result_case,
                identity: "Success".into(),
                fields: Vec::new(),
            }],
        },
    }];
    let machine = &mut module.machines[0];
    machine.result = TerminalMachineResult::Structural(StructuralResultDeclaration {
        place: result_place,
        structural_type,
        multiplicity: StructuralMultiplicity::Unrestricted,
        qualifications: Vec::new(),
    });
    machine.structural_places = vec![
        StructuralPlaceDeclaration {
            id: operation_place,
            kind: psi_core::StructuralPlaceKind::OperationResult {
                producer: operation_id(910),
                structural_type,
            },
        },
        StructuralPlaceDeclaration {
            id: result_place,
            kind: psi_core::StructuralPlaceKind::Result,
        },
    ];
    machine.blocks[0].operations = vec![Operation {
        id: operation_id(910),
        result: OperationResult::Structural(StructuralOperationResult {
            place: operation_place,
            structural_type,
            multiplicity: StructuralMultiplicity::Unrestricted,
            qualifications: Vec::new(),
            claims: Vec::new(),
        }),
        kind: OperationKind::EstablishPayloadlessCase { result_case },
    }];
    machine.blocks[0].terminator = Terminator::ReturnStructural {
        edge: edge_id(910),
        source: operation_place,
        returned_claims: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };

    let verified = verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("payloadless case module verifies");
    let certificate = derive_fixed_entry_fuel(&verified, machine_id(900))
        .expect("payloadless case has an exact fixed bound");

    assert_eq!(certificate.ceiling_units(), 2);
    validate_fixed_entry_fuel(&verified, &certificate).unwrap();
    drop(verified);

    let mut callee = module.machines.remove(0);
    callee.id = machine_id(901);
    callee.entry = block_id(901);
    callee.blocks[0].id = block_id(901);
    callee.contract.id = contract_id(901);
    let caller_operation = operation_id(911);
    let caller_operation_place = place_id(912);
    let caller_result_place = place_id(913);
    module.machines = vec![
        TerminalMachine {
            id: machine_id(900),
            attachment: None,
            structural_parameters: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            parameters: Vec::new(),
            ranked_scc: None,
            result: TerminalMachineResult::Structural(StructuralResultDeclaration {
                place: caller_result_place,
                structural_type,
                multiplicity: StructuralMultiplicity::Unrestricted,
                qualifications: Vec::new(),
            }),
            structural_places: vec![
                StructuralPlaceDeclaration {
                    id: caller_operation_place,
                    kind: psi_core::StructuralPlaceKind::OperationResult {
                        producer: caller_operation,
                        structural_type,
                    },
                },
                StructuralPlaceDeclaration {
                    id: caller_result_place,
                    kind: psi_core::StructuralPlaceKind::Result,
                },
            ],
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: block_id(900),
            blocks: vec![Block {
                id: block_id(900),
                parameters: Vec::new(),
                operations: vec![Operation {
                    id: caller_operation,
                    result: OperationResult::Structural(StructuralOperationResult {
                        place: caller_operation_place,
                        structural_type,
                        multiplicity: StructuralMultiplicity::Unrestricted,
                        qualifications: Vec::new(),
                        claims: Vec::new(),
                    }),
                    kind: OperationKind::CallStructural {
                        callee: machine_id(901),
                        structural_arguments: Vec::new(),
                        claim_transfers: Vec::new(),
                        returned_claim_transfers: Vec::new(),
                        requirement_obligations: Vec::new(),
                        crash_continuations: Vec::new(),
                        selected_evidence: Vec::new(),
                    },
                }],
                terminator: Terminator::ReturnStructural {
                    edge: edge_id(911),
                    source: caller_operation_place,
                    returned_claims: Vec::new(),
                    trivial_affine_discards: Vec::new(),
                },
            }],
            contract: MachineContract {
                id: contract_id(900),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        },
        callee,
    ];
    let verified = verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("payloadless structural caller verifies");
    let certificate = derive_fixed_entry_fuel(&verified, machine_id(900))
        .expect("payloadless structural call has an exact fixed bound");
    assert_eq!(certificate.ceiling_units(), 4);
    validate_fixed_entry_fuel(&verified, &certificate).unwrap();
}

#[test]
fn nominal_affine_cleanup_composes_the_cleanup_machine_bound() {
    let structural_type = structural_type_id(900);
    let source = place_id(900);
    let cleanup_machine = machine_id(901);
    let mut module = unit_fixture();
    module.structural_types = vec![StructuralTypeDeclaration {
        id: structural_type,
        identity: "test::Token".into(),
        shape: StructuralTypeShape::Record {
            fields: (900..905)
                .map(|index| StructuralFieldDeclaration {
                    identity: format!("payload_{index}"),
                    id: psi_core::StructuralFieldId::new(index).unwrap(),
                    field_type: StructuralFieldType::Scalar(ScalarType::Integer(
                        IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                    )),
                    relevance: psi_terminal::BindingRelevance::Relevant,
                })
                .collect(),
        },
    }];
    let caller = &mut module.machines[0];
    caller.structural_parameters = vec![StructuralParameterDeclaration {
        access: StructuralAccess::Owned,
        place: source,
        position: 0,
        is_self: false,
        structural_type,
        multiplicity: StructuralMultiplicity::Affine,
        qualifications: Vec::new(),
    }];
    caller.structural_places = vec![StructuralPlaceDeclaration {
        id: source,
        kind: psi_core::StructuralPlaceKind::Parameter {
            position: 0,
            is_self: false,
        },
    }];
    caller.blocks[0].terminator = Terminator::ReturnUnitNominalAffine {
        edge: edge_id(900),
        cleanups: vec![NominalAffineCleanup {
            place: source,
            structural_type,
            cleanup_machine,
            cleanup_receiver: None,
            requirement_obligations: Vec::new(),
        }],
    };
    module.machines.push(TerminalMachine {
        id: cleanup_machine,
        attachment: Some(structural_type),
        parameters: Vec::new(),
        structural_parameters: Vec::new(),
        ranked_scc: None,
        result: TerminalMachineResult::Unit,
        structural_places: Vec::new(),
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: block_id(901),
        blocks: vec![Block {
            id: block_id(901),
            parameters: Vec::new(),
            operations: Vec::new(),
            terminator: Terminator::ReturnUnit {
                edge: edge_id(901),
                trivial_affine_discards: Vec::new(),
            },
        }],
        contract: MachineContract {
            id: contract_id(901),
            crash_routes: Vec::new(),
            requires: Vec::new(),
            ensures: Vec::new(),
            outcome_specific_ensures: Vec::new(),
        },
    });

    let verified = verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("nominal cleanup module verifies");
    assert_eq!(
        derive_fixed_entry_fuel(&verified, machine_id(900))
            .expect("nominal cleanup has an exact fixed bound")
            .ceiling_units(),
        2,
        "the caller edge and cleanup-machine edge are both charged"
    );
}

#[test]
fn ordered_empty_nominal_affine_cleanups_have_exact_three_unit_bound() {
    let module = ordered_empty_nominal_affine_fixture(false);
    let verified = verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("ordered nominal cleanup module verifies");
    let certificate = derive_fixed_entry_fuel(&verified, machine_id(900))
        .expect("ordered nominal cleanups have an exact fixed bound");

    assert_eq!(
        certificate.ceiling_units(),
        3,
        "one root edge plus two cleanup-machine return edges"
    );
    validate_fixed_entry_fuel(&verified, &certificate).unwrap();
}

#[test]
fn ordered_nominal_affine_cleanups_count_the_same_target_twice() {
    let module = ordered_empty_nominal_affine_fixture(true);
    let verified = verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("same-target nominal cleanup module verifies");
    let certificate = derive_fixed_entry_fuel(&verified, machine_id(900))
        .expect("same-target nominal cleanups have an exact fixed bound");

    assert_eq!(
        certificate.ceiling_units(),
        3,
        "memoization must not collapse two invocations of one cleanup machine"
    );
    validate_fixed_entry_fuel(&verified, &certificate).unwrap();
}

#[test]
fn ordered_nominal_affine_cleanups_include_one_executable_body_in_the_exact_bound() {
    let module = ordered_one_executable_nominal_affine_fixture();
    let verified = verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("one executable ordered cleanup module verifies");
    let certificate = derive_fixed_entry_fuel(&verified, machine_id(900))
        .expect("one executable ordered cleanup has an exact fixed bound");

    assert_eq!(
        certificate.ceiling_units(),
        5,
        "root edge + helper call/edge + executable drop edge + empty drop edge"
    );
    validate_fixed_entry_fuel(&verified, &certificate).unwrap();
}

#[test]
fn ordered_nominal_affine_cleanups_include_two_distinct_executable_bodies() {
    let module = ordered_two_distinct_executable_nominal_affine_fixture();
    let verified = verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("two distinct executable cleanup bodies verify");
    let certificate = derive_fixed_entry_fuel(&verified, machine_id(900))
        .expect("two executable cleanup bodies have an exact fixed bound");
    assert_eq!(certificate.ceiling_units(), 7);
    validate_fixed_entry_fuel(&verified, &certificate).unwrap();
}

#[test]
fn ordered_nominal_affine_cleanups_count_shared_executable_target_and_helper_twice() {
    let module = ordered_shared_executable_nominal_affine_fixture();
    let verified = verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("shared executable cleanup target verifies");
    let certificate = derive_fixed_entry_fuel(&verified, machine_id(900))
        .expect("shared executable cleanup has an exact repeated bound");
    assert_eq!(
        certificate.ceiling_units(),
        7,
        "root plus the shared call/helper/drop path invoked twice"
    );
    validate_fixed_entry_fuel(&verified, &certificate).unwrap();
}

#[test]
fn three_nominal_affine_cleanups_count_shared_executable_target_and_helper_three_times() {
    let module = three_ordered_shared_executable_nominal_affine_fixture();
    let verified = verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("three shared executable cleanup actions verify");
    let certificate = derive_fixed_entry_fuel(&verified, machine_id(900))
        .expect("three shared executable cleanups have an exact repeated bound");
    assert_eq!(
        certificate.ceiling_units(),
        10,
        "root plus the shared call/helper/drop path invoked three times"
    );
    validate_fixed_entry_fuel(&verified, &certificate).unwrap();
}

#[test]
fn finite_nominal_cleanup_list_counts_every_shared_executable_invocation() {
    let module = five_ordered_shared_executable_nominal_affine_fixture();
    let verified = verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("five shared executable cleanup actions verify");
    let certificate = derive_fixed_entry_fuel(&verified, machine_id(900))
        .expect("five shared executable cleanups have an exact repeated bound");
    assert_eq!(certificate.ceiling_units(), 16);
    validate_fixed_entry_fuel(&verified, &certificate).unwrap();
}

#[test]
fn executable_nominal_affine_cleanup_has_exact_four_unit_bound() {
    let module = executable_nominal_affine_fixture();
    let verified = verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("executable nominal cleanup module verifies");
    let certificate = derive_fixed_entry_fuel(&verified, machine_id(900))
        .expect("executable nominal cleanup has an exact fixed bound");

    assert_eq!(
        certificate.ceiling_units(),
        4,
        "root edge + drop call + helper edge + drop edge"
    );
    validate_fixed_entry_fuel(&verified, &certificate).unwrap();
}

#[test]
fn scalar_return_composes_every_nominal_cleanup_bound() {
    let mut module = ordered_empty_nominal_affine_fixture(true);
    let caller = &mut module.machines[0];
    caller.parameters = vec![ValueDeclaration {
        id: value_id(900),
        scalar_type: ScalarType::Boolean,
    }];
    caller.result = TerminalMachineResult::Scalar(ValueDeclaration {
        id: value_id(901),
        scalar_type: ScalarType::Boolean,
    });
    let Terminator::ReturnUnitNominalAffine { edge, cleanups } = std::mem::replace(
        &mut caller.blocks[0].terminator,
        Terminator::ReturnUnit {
            edge: edge_id(999),
            trivial_affine_discards: Vec::new(),
        },
    ) else {
        unreachable!()
    };
    caller.blocks[0].terminator = Terminator::Return {
        edge,
        value: value_id(900),
        cleanup_actions: cleanups
            .into_iter()
            .map(TerminalAffineCleanupAction::InvokeNominal)
            .collect(),
    };
    let verified = verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("scalar return cleanup module verifies");
    let certificate = derive_fixed_entry_fuel(&verified, machine_id(900)).unwrap();
    assert_eq!(certificate.ceiling_units(), 3);
    validate_fixed_entry_fuel(&verified, &certificate).unwrap();
}

#[test]
fn contextual_scalar_cleanup_proof_metadata_adds_zero_fixed_fuel() {
    let mut module = ordered_empty_nominal_affine_fixture(true);
    let first = psi_core::StructuralFieldId::new(1).expect("first field");
    let second = psi_core::StructuralFieldId::new(2).expect("second field");
    module.structural_types[0].shape = StructuralTypeShape::Record {
        fields: [first, second]
            .into_iter()
            .map(|id| StructuralFieldDeclaration {
                id,
                identity: format!("flag_{}", id.get()),
                relevance: psi_terminal::BindingRelevance::Relevant,
                field_type: StructuralFieldType::Scalar(ScalarType::Boolean),
            })
            .collect(),
    };
    let receiver = place_id(999);
    module.machines[1].contract.requires = [first, second]
        .into_iter()
        .map(|field| {
            Proposition::Equal(
                ScalarTerm::boolean(true),
                ScalarTerm::boolean_field(receiver, field),
            )
        })
        .collect();
    let caller = &mut module.machines[0];
    caller.parameters = vec![ValueDeclaration {
        id: value_id(910),
        scalar_type: ScalarType::Boolean,
    }];
    caller.result = TerminalMachineResult::Scalar(ValueDeclaration {
        id: value_id(911),
        scalar_type: ScalarType::Boolean,
    });
    caller.contract.requires = [place_id(900), place_id(901)]
        .into_iter()
        .flat_map(|root| {
            [first, second].map(move |field| {
                Proposition::Equal(
                    ScalarTerm::boolean(true),
                    ScalarTerm::boolean_field(root, field),
                )
            })
        })
        .collect();
    caller.contract.requires.sort();
    let Terminator::ReturnUnitNominalAffine { edge, mut cleanups } = std::mem::replace(
        &mut caller.blocks[0].terminator,
        Terminator::ReturnUnit {
            edge: edge_id(999),
            trivial_affine_discards: Vec::new(),
        },
    ) else {
        unreachable!()
    };
    cleanups[0].cleanup_receiver = Some(receiver);
    cleanups[0].requirement_obligations = vec![obligation_id(3), obligation_id(4)];
    cleanups[1].cleanup_receiver = Some(receiver);
    cleanups[1].requirement_obligations = vec![obligation_id(1), obligation_id(2)];
    caller.blocks[0].terminator = Terminator::Return {
        edge,
        value: value_id(910),
        cleanup_actions: cleanups
            .into_iter()
            .map(TerminalAffineCleanupAction::InvokeNominal)
            .collect(),
    };

    let goals = [
        (obligation_id(3), place_id(901), first),
        (obligation_id(4), place_id(901), second),
        (obligation_id(1), place_id(900), first),
        (obligation_id(2), place_id(900), second),
    ];
    let mut evidence = goals
        .into_iter()
        .enumerate()
        .map(|(index, (obligation, root, field))| {
            let conclusion = Proposition::Equal(
                ScalarTerm::boolean(true),
                ScalarTerm::boolean_field(root, field),
            );
            ObligationEvidence {
                obligation,
                route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                    identity: EvidenceIdentity::new(index as u64 + 1).expect("certificate"),
                    proof_system_marker: ProofSystemMarker::CURRENT,
                    proof: ProofNode {
                        rule: ProofRule::Assumption {
                            index: module.machines[0]
                                .contract
                                .requires
                                .iter()
                                .position(|requirement| requirement == &conclusion)
                                .expect("cleanup goal is a caller premise"),
                        },
                        conclusion,
                    },
                }),
            }
        })
        .collect::<Vec<_>>();
    evidence.sort_by_key(|evidence| evidence.obligation);
    let proof = ProofBundle {
        evidence_producers: Vec::new(),
        evidence,
    };
    let verified = verify_module(&module, &proof, &AdmissionProfile::default())
        .expect("contextual scalar cleanup verifies");
    let contextual = derive_fixed_entry_fuel(&verified, machine_id(900))
        .expect("contextual scalar cleanup has an exact fixed bound");
    assert_eq!(contextual.ceiling_units(), 3);
    validate_fixed_entry_fuel(&verified, &contextual).unwrap();

    drop(verified);
    let mut proof_free = module;
    proof_free.machines[0].contract.requires.clear();
    proof_free.machines[1].contract.requires.clear();
    let Terminator::Return {
        cleanup_actions, ..
    } = &mut proof_free.machines[0].blocks[0].terminator
    else {
        unreachable!()
    };
    for action in cleanup_actions {
        let TerminalAffineCleanupAction::InvokeNominal(cleanup) = action else {
            unreachable!()
        };
        cleanup.cleanup_receiver = None;
        cleanup.requirement_obligations.clear();
    }
    let proof_free_verified = verify_module(
        &proof_free,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("proof-free scalar cleanup baseline verifies");
    let proof_free_certificate = derive_fixed_entry_fuel(&proof_free_verified, machine_id(900))
        .expect("proof-free scalar cleanup has an exact fixed bound");
    assert_eq!(
        contextual.ceiling_units(),
        proof_free_certificate.ceiling_units(),
        "proof receivers, obligations, and evidence are non-executable metadata"
    );
}

#[test]
fn mixed_scalar_return_counts_nominal_work_but_not_root_discards() {
    let mut module = three_ordered_shared_executable_nominal_affine_fixture();
    let caller = &mut module.machines[0];
    caller.parameters = vec![ValueDeclaration {
        id: value_id(910),
        scalar_type: ScalarType::Boolean,
    }];
    caller.result = TerminalMachineResult::Scalar(ValueDeclaration {
        id: value_id(911),
        scalar_type: ScalarType::Boolean,
    });
    let Terminator::ReturnUnitNominalAffine { edge, cleanups } = std::mem::replace(
        &mut caller.blocks[0].terminator,
        Terminator::ReturnUnit {
            edge: edge_id(999),
            trivial_affine_discards: Vec::new(),
        },
    ) else {
        unreachable!()
    };
    assert_eq!(cleanups.len(), 3);
    caller.blocks[0].terminator = Terminator::Return {
        edge,
        value: value_id(910),
        cleanup_actions: vec![
            TerminalAffineCleanupAction::InvokeNominal(cleanups[0].clone()),
            TerminalAffineCleanupAction::DiscardRoot(cleanups[1].place),
            TerminalAffineCleanupAction::InvokeNominal(cleanups[2].clone()),
        ],
    };

    let verified = verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("mixed scalar cleanup module verifies");
    let certificate = derive_fixed_entry_fuel(&verified, machine_id(900))
        .expect("mixed scalar cleanup has an exact fixed bound");
    assert_eq!(
        certificate.ceiling_units(),
        7,
        "one scalar-return edge plus two three-unit nominal paths; the interleaved discard is no-code"
    );
    validate_fixed_entry_fuel(&verified, &certificate).unwrap();
}

#[test]
fn two_helper_nominal_affine_cleanup_has_exact_six_unit_bound() {
    let module = two_helper_nominal_affine_fixture();
    let verified = verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("two-helper nominal cleanup module verifies");
    let certificate = derive_fixed_entry_fuel(&verified, machine_id(900))
        .expect("two-helper nominal cleanup has an exact fixed bound");

    assert_eq!(
        certificate.ceiling_units(),
        6,
        "root edge + first call/helper edge + second call/helper edge + drop edge"
    );
    validate_fixed_entry_fuel(&verified, &certificate).unwrap();
}

#[test]
fn three_helper_nominal_affine_cleanup_has_exact_eight_unit_bound() {
    let module = three_helper_nominal_affine_fixture();
    let verified = verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("three-helper nominal cleanup module verifies");
    let certificate = derive_fixed_entry_fuel(&verified, machine_id(900))
        .expect("three-helper nominal cleanup has an exact fixed bound");

    assert_eq!(
        certificate.ceiling_units(),
        8,
        "root edge + three call/helper edges + drop edge"
    );
    validate_fixed_entry_fuel(&verified, &certificate).unwrap();
}

#[test]
fn unit_affine_local_establishments_are_in_the_fixed_entry_bound() {
    let local_type = structural_type_id(900);
    let first = place_id(900);
    let second = place_id(901);
    let mut module = unit_fixture();
    module.structural_types = vec![StructuralTypeDeclaration {
        id: local_type,
        identity: "test::EmptyScratch".into(),
        shape: StructuralTypeShape::Record { fields: Vec::new() },
    }];
    let machine = &mut module.machines[0];
    machine.structural_places = vec![
        StructuralPlaceDeclaration {
            id: first,
            kind: psi_core::StructuralPlaceKind::TrivialAffineLocal {
                declaration_ordinal: 0,
                structural_type: local_type,
                construction: None,
            },
        },
        StructuralPlaceDeclaration {
            id: second,
            kind: psi_core::StructuralPlaceKind::TrivialAffineLocal {
                declaration_ordinal: 1,
                structural_type: local_type,
                construction: None,
            },
        },
    ];
    machine.blocks[0].operations = vec![
        Operation {
            id: operation_id(900),
            result: OperationResult::Unit,
            kind: OperationKind::EstablishTrivialAffineLocal { destination: first },
        },
        Operation {
            id: operation_id(901),
            result: OperationResult::Unit,
            kind: OperationKind::EstablishTrivialAffineLocal {
                destination: second,
            },
        },
    ];
    machine.blocks[0].terminator = Terminator::ReturnUnit {
        edge: edge_id(900),
        trivial_affine_discards: vec![second, first],
    };
    let verified = verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("Unit local cleanup verifies");
    let certificate = derive_fixed_entry_fuel(&verified, machine_id(900)).unwrap();
    assert_eq!(certificate.ceiling_units(), 3);
    validate_fixed_entry_fuel(&verified, &certificate).unwrap();
}

#[test]
fn structural_return_is_one_normal_edge_unit() {
    let structural_type = structural_type_id(900);
    let source = place_id(900);
    let result_place = place_id(901);
    let first_affine = place_id(902);
    let second_affine = place_id(903);
    let claim = claim_id(1);
    let mut module = unit_fixture();
    module.structural_types = vec![StructuralTypeDeclaration {
        id: structural_type,
        identity: "test::Resource".into(),
        shape: StructuralTypeShape::Record { fields: Vec::new() },
    }];
    let machine = &mut module.machines[0];
    machine.structural_parameters = vec![
        StructuralParameterDeclaration {
            access: StructuralAccess::Owned,
            place: source,
            position: 0,
            is_self: false,
            structural_type,
            multiplicity: StructuralMultiplicity::Linear,
            qualifications: Vec::new(),
        },
        StructuralParameterDeclaration {
            access: StructuralAccess::Owned,
            place: first_affine,
            position: 1,
            is_self: false,
            structural_type,
            multiplicity: StructuralMultiplicity::Affine,
            qualifications: Vec::new(),
        },
        StructuralParameterDeclaration {
            access: StructuralAccess::Owned,
            place: second_affine,
            position: 2,
            is_self: false,
            structural_type,
            multiplicity: StructuralMultiplicity::Affine,
            qualifications: Vec::new(),
        },
    ];
    machine.result = TerminalMachineResult::Structural(StructuralResultDeclaration {
        place: result_place,
        structural_type,
        multiplicity: StructuralMultiplicity::Linear,
        qualifications: Vec::new(),
    });
    machine.structural_places = vec![
        StructuralPlaceDeclaration {
            id: source,
            kind: psi_core::StructuralPlaceKind::Parameter {
                position: 0,
                is_self: false,
            },
        },
        StructuralPlaceDeclaration {
            id: result_place,
            kind: psi_core::StructuralPlaceKind::Result,
        },
        StructuralPlaceDeclaration {
            id: first_affine,
            kind: psi_core::StructuralPlaceKind::Parameter {
                position: 1,
                is_self: false,
            },
        },
        StructuralPlaceDeclaration {
            id: second_affine,
            kind: psi_core::StructuralPlaceKind::Parameter {
                position: 2,
                is_self: false,
            },
        },
    ];
    machine.entry_claims = vec![EntryClaim {
        claim,
        input: source,
        path: Vec::new(),
    }];
    machine.blocks[0].terminator = Terminator::ReturnStructural {
        edge: edge_id(900),
        source,
        returned_claims: vec![claim],
        trivial_affine_discards: vec![second_affine, first_affine],
    };
    let verified = verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("structural return verifies");
    let certificate = derive_fixed_entry_fuel(&verified, machine_id(900)).unwrap();
    assert_eq!(certificate.ceiling_units(), 1);
    validate_fixed_entry_fuel(&verified, &certificate).unwrap();
}

#[test]
fn each_trivial_affine_local_establishment_adds_one_fixed_fuel_unit() {
    let structural_type = structural_type_id(900);
    let local_type = structural_type_id(901);
    let source = place_id(900);
    let result_place = place_id(901);
    let local = place_id(902);
    let second_local = place_id(903);
    let claim = claim_id(1);
    let mut module = unit_fixture();
    module.structural_types = vec![
        StructuralTypeDeclaration {
            id: structural_type,
            identity: "test::Resource".into(),
            shape: StructuralTypeShape::Record { fields: Vec::new() },
        },
        StructuralTypeDeclaration {
            id: local_type,
            identity: "test::EmptyScratch".into(),
            shape: StructuralTypeShape::Record { fields: Vec::new() },
        },
    ];
    let machine = &mut module.machines[0];
    machine.structural_parameters = vec![StructuralParameterDeclaration {
        access: StructuralAccess::Owned,
        place: source,
        position: 0,
        is_self: false,
        structural_type,
        multiplicity: StructuralMultiplicity::Linear,
        qualifications: Vec::new(),
    }];
    machine.result = TerminalMachineResult::Structural(StructuralResultDeclaration {
        place: result_place,
        structural_type,
        multiplicity: StructuralMultiplicity::Linear,
        qualifications: Vec::new(),
    });
    machine.structural_places = vec![
        StructuralPlaceDeclaration {
            id: source,
            kind: psi_core::StructuralPlaceKind::Parameter {
                position: 0,
                is_self: false,
            },
        },
        StructuralPlaceDeclaration {
            id: result_place,
            kind: psi_core::StructuralPlaceKind::Result,
        },
        StructuralPlaceDeclaration {
            id: local,
            kind: psi_core::StructuralPlaceKind::TrivialAffineLocal {
                declaration_ordinal: 0,
                structural_type: local_type,
                construction: None,
            },
        },
        StructuralPlaceDeclaration {
            id: second_local,
            kind: psi_core::StructuralPlaceKind::TrivialAffineLocal {
                declaration_ordinal: 1,
                structural_type: local_type,
                construction: None,
            },
        },
    ];
    machine.entry_claims = vec![EntryClaim {
        claim,
        input: source,
        path: Vec::new(),
    }];
    machine.blocks[0].operations = vec![
        Operation {
            id: operation_id(900),
            result: OperationResult::Unit,
            kind: OperationKind::EstablishTrivialAffineLocal { destination: local },
        },
        Operation {
            id: operation_id(901),
            result: OperationResult::Unit,
            kind: OperationKind::EstablishTrivialAffineLocal {
                destination: second_local,
            },
        },
    ];
    machine.blocks[0].terminator = Terminator::ReturnStructural {
        edge: edge_id(900),
        source,
        returned_claims: vec![claim],
        trivial_affine_discards: vec![second_local, local],
    };
    let verified = verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("structural return with trivial local verifies");
    let certificate = derive_fixed_entry_fuel(&verified, machine_id(900)).unwrap();
    assert_eq!(certificate.ceiling_units(), 3);
    validate_fixed_entry_fuel(&verified, &certificate).unwrap();
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
fn write_only_primitive_store_has_exact_local_and_transitive_call_bounds() {
    let module = write_only_primitive_store_fixture();
    let verified = verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("write-only primitive-store module verifies");

    let callee = derive_fixed_entry_fuel(&verified, machine_id(701))
        .expect("constant, store, and return have a fixed bound");
    assert_eq!(callee.ceiling_units(), 3);
    validate_fixed_entry_fuel(&verified, &callee).unwrap();

    let caller = derive_fixed_entry_fuel(&verified, machine_id(700))
        .expect("the Unit call composes the store callee bound");
    assert_eq!(caller.ceiling_units(), 5);
    validate_fixed_entry_fuel(&verified, &caller).unwrap();
    let segments = derive_fixed_safe_point_segments(&verified, machine_id(700)).unwrap();
    assert_eq!(segments.len(), 1);
    assert_eq!(segments[0].ceiling_units(), 5);
}

#[test]
fn projected_unit_calls_compose_each_callee_bound_in_call_order() {
    let mut module = unit_effect_fixture();
    module.root_service_reach = Default::default();
    let element = structural_type_id(950);
    let array = structural_type_id(951);
    module.structural_types = vec![
        StructuralTypeDeclaration {
            id: element,
            identity: "test::Receipt".into(),
            shape: StructuralTypeShape::Record { fields: Vec::new() },
        },
        StructuralTypeDeclaration {
            id: array,
            identity: "[test::Receipt;2]".into(),
            shape: StructuralTypeShape::FixedArray { element, length: 2 },
        },
    ];
    module.boundary_machines[0].structural_parameters = vec![StructuralParameterDeclaration {
        access: StructuralAccess::Owned,
        place: place_id(952),
        position: 0,
        is_self: false,
        structural_type: element,
        multiplicity: StructuralMultiplicity::Linear,
        qualifications: Vec::new(),
    }];

    let caller = &mut module.machines[0];
    caller.structural_parameters = vec![StructuralParameterDeclaration {
        access: StructuralAccess::Owned,
        place: place_id(950),
        position: 0,
        is_self: false,
        structural_type: array,
        multiplicity: StructuralMultiplicity::Linear,
        qualifications: Vec::new(),
    }];
    caller.structural_places = vec![StructuralPlaceDeclaration {
        id: place_id(950),
        kind: psi_core::StructuralPlaceKind::Parameter {
            position: 0,
            is_self: false,
        },
    }];
    caller.entry_claims = vec![
        EntryClaim {
            claim: claim_id(1),
            input: place_id(950),
            path: vec![StructuralPathSegment::FixedIndex(0)],
        },
        EntryClaim {
            claim: claim_id(2),
            input: place_id(950),
            path: vec![StructuralPathSegment::FixedIndex(1)],
        },
    ];
    caller.blocks[0].operations = (0..2)
        .map(|index| Operation {
            id: operation_id(950 + index),
            result: OperationResult::Unit,
            kind: OperationKind::CallUnit {
                callee: machine_id(701),
                structural_arguments: vec![StructuralArgument {
                    access: StructuralAccess::Owned,
                    place: place_id(950),
                    path: vec![StructuralPathSegment::FixedIndex(index)],
                }],
                claim_transfers: vec![ClaimTransfer {
                    claim: claim_id(1 + index),
                    argument_index: 0,
                }],
                requirement_obligations: Vec::new(),
                crash_continuations: Vec::new(),
            },
        })
        .collect();

    let callee = &mut module.machines[1];
    callee.structural_parameters = vec![StructuralParameterDeclaration {
        access: StructuralAccess::Owned,
        place: place_id(951),
        position: 0,
        is_self: false,
        structural_type: element,
        multiplicity: StructuralMultiplicity::Linear,
        qualifications: Vec::new(),
    }];
    callee.structural_places = vec![StructuralPlaceDeclaration {
        id: place_id(951),
        kind: psi_core::StructuralPlaceKind::Parameter {
            position: 0,
            is_self: false,
        },
    }];
    callee.entry_claims = vec![EntryClaim {
        claim: claim_id(1),
        input: place_id(951),
        path: Vec::new(),
    }];
    let OperationKind::BoundaryCall {
        structural_arguments,
        completion_receipts,
        ..
    } = &mut callee.blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    *structural_arguments = vec![StructuralArgument {
        access: StructuralAccess::Owned,
        place: place_id(951),
        path: Vec::new(),
    }];
    *completion_receipts = vec![CompletionReceipt {
        claim: claim_id(1),
        argument_index: 0,
    }];

    let verified = verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("projected sibling calls verify");
    let certificate = derive_fixed_entry_fuel(&verified, machine_id(700)).unwrap();
    assert_eq!(certificate.ceiling_units(), 7);
    validate_fixed_entry_fuel(&verified, &certificate).unwrap();
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
                    trivial_affine_discards: Vec::new(),
                },
                when_false: SuccessorEdge {
                    edge: edge_id(3),
                    target: block_id(4),
                    arguments: Vec::new(),
                    trivial_affine_discards: Vec::new(),
                },
            },
        },
        Block {
            id: block_id(3),
            parameters: Vec::new(),
            operations: Vec::new(),
            terminator: Terminator::Return {
                cleanup_actions: Vec::new(),
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

#[test]
fn retained_safe_point_catalog_is_complete_ordered_and_semantically_exact() {
    let (module, proof) = fixture();
    let verified = verify_module(&module, &proof, &AdmissionProfile::default()).unwrap();
    let segments = derive_fixed_safe_point_segments(&verified, machine_id(1)).unwrap();
    let retained =
        retain_validated_fixed_safe_point_segments(&verified, machine_id(1), segments.clone())
            .expect("the complete canonical partition is retainable");

    assert_eq!(
        retained.terminal_psi(),
        terminal_psi_identity(&module).unwrap()
    );
    assert_eq!(retained.schedule().marker(), 1);
    assert_eq!(retained.machine(), machine_id(1));
    assert_eq!(retained.certificates(), segments);
    validate_retained_fixed_safe_point_segments(&verified, &retained)
        .expect("the retained partition independently replays");

    assert_eq!(
        retain_validated_fixed_safe_point_segments(
            &verified,
            machine_id(1),
            segments[..1].to_vec(),
        ),
        Err(FixedFuelError::CertificateMismatch),
        "an omitted reachable segment cannot be sealed"
    );
    let mut reordered = segments.clone();
    reordered.reverse();
    assert_eq!(
        retain_validated_fixed_safe_point_segments(&verified, machine_id(1), reordered),
        Err(FixedFuelError::CertificateMismatch),
        "a reordered partition cannot be sealed"
    );
    let mut duplicated = segments.clone();
    duplicated.push(segments[0].clone());
    assert_eq!(
        retain_validated_fixed_safe_point_segments(&verified, machine_id(1), duplicated),
        Err(FixedFuelError::CertificateMismatch),
        "a duplicated segment cannot be sealed"
    );

    let (mut drifted_module, drifted_proof) = fixture();
    let OperationKind::IntegerConstant { value } =
        &mut drifted_module.machines[0].blocks[0].operations[0].kind
    else {
        panic!("fixture begins with an integer constant")
    };
    *value = IntegerValue::Signed(8);
    let drifted_verified = verify_module(
        &drifted_module,
        &drifted_proof,
        &AdmissionProfile::default(),
    )
    .expect("the semantically drifted module remains structurally valid");
    assert_eq!(
        validate_retained_fixed_safe_point_segments(&drifted_verified, &retained),
        Err(FixedFuelError::CertificateMismatch),
        "a different terminal semantic identity cannot replay the catalog"
    );

    let derived = derive_validated_fixed_safe_point_segments(&verified, machine_id(1))
        .expect("direct catalog derivation succeeds");
    assert_eq!(derived.certificates(), segments);
}

fn unit_fixture() -> TerminalModule {
    TerminalModule {
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: machine_id(900),
        structural_types: Vec::new(),
        structural_domains: Vec::new(),
        services: Vec::new(),
        root_service_reach: Default::default(),
        placed_view_inputs: Vec::new(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        float_meaning_projections: Vec::new(),
        float_meaning_equalities: Vec::new(),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        evidence_terms: Vec::new(),
        evidence_contract_lanes: Vec::new(),
        proof_output_calls: Vec::new(),
        closed_conformance_applications: Vec::new(),
        quotient_correspondences: Vec::new(),
        machines: vec![TerminalMachine {
            id: machine_id(900),
            attachment: None,
            structural_parameters: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            parameters: Vec::new(),
            ranked_scc: None,
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
                outcome_specific_ensures: Vec::new(),
            },
        }],
    }
}

fn ordered_empty_nominal_affine_fixture(same_target: bool) -> TerminalModule {
    let first_type = structural_type_id(900);
    let second_type = if same_target {
        first_type
    } else {
        structural_type_id(901)
    };
    let first_place = place_id(900);
    let second_place = place_id(901);
    let first_cleanup = machine_id(901);
    let second_cleanup = if same_target {
        first_cleanup
    } else {
        machine_id(902)
    };
    let primitive_record = |id, identity: &str, field| StructuralTypeDeclaration {
        id,
        identity: identity.into(),
        shape: StructuralTypeShape::Record {
            fields: vec![StructuralFieldDeclaration {
                identity: "payload".into(),
                id: psi_core::StructuralFieldId::new(field).unwrap(),
                field_type: StructuralFieldType::Scalar(ScalarType::Integer(
                    IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                )),
                relevance: psi_terminal::BindingRelevance::Relevant,
            }],
        },
    };
    let cleanup_machine = |id, attachment, block, edge, contract| TerminalMachine {
        id,
        attachment: Some(attachment),
        parameters: Vec::new(),
        structural_parameters: Vec::new(),
        ranked_scc: None,
        result: TerminalMachineResult::Unit,
        structural_places: Vec::new(),
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: block,
        blocks: vec![Block {
            id: block,
            parameters: Vec::new(),
            operations: Vec::new(),
            terminator: Terminator::ReturnUnit {
                edge,
                trivial_affine_discards: Vec::new(),
            },
        }],
        contract: MachineContract {
            id: contract,
            crash_routes: Vec::new(),
            requires: Vec::new(),
            ensures: Vec::new(),
            outcome_specific_ensures: Vec::new(),
        },
    };

    let mut module = unit_fixture();
    module.structural_types = vec![primitive_record(first_type, "test::First", 900)];
    if !same_target {
        module
            .structural_types
            .push(primitive_record(second_type, "test::Second", 901));
    }
    let caller = &mut module.machines[0];
    caller.structural_parameters = vec![
        StructuralParameterDeclaration {
            access: StructuralAccess::Owned,
            place: first_place,
            position: 0,
            is_self: false,
            structural_type: first_type,
            multiplicity: StructuralMultiplicity::Affine,
            qualifications: Vec::new(),
        },
        StructuralParameterDeclaration {
            access: StructuralAccess::Owned,
            place: second_place,
            position: 1,
            is_self: false,
            structural_type: second_type,
            multiplicity: StructuralMultiplicity::Affine,
            qualifications: Vec::new(),
        },
    ];
    caller.structural_places = vec![
        StructuralPlaceDeclaration {
            id: first_place,
            kind: psi_core::StructuralPlaceKind::Parameter {
                position: 0,
                is_self: false,
            },
        },
        StructuralPlaceDeclaration {
            id: second_place,
            kind: psi_core::StructuralPlaceKind::Parameter {
                position: 1,
                is_self: false,
            },
        },
    ];
    caller.blocks[0].terminator = Terminator::ReturnUnitNominalAffine {
        edge: edge_id(900),
        cleanups: vec![
            NominalAffineCleanup {
                place: second_place,
                structural_type: second_type,
                cleanup_machine: second_cleanup,
                cleanup_receiver: None,
                requirement_obligations: Vec::new(),
            },
            NominalAffineCleanup {
                place: first_place,
                structural_type: first_type,
                cleanup_machine: first_cleanup,
                cleanup_receiver: None,
                requirement_obligations: Vec::new(),
            },
        ],
    };
    module.machines.push(cleanup_machine(
        first_cleanup,
        first_type,
        block_id(901),
        edge_id(901),
        contract_id(901),
    ));
    if !same_target {
        module.machines.push(cleanup_machine(
            second_cleanup,
            second_type,
            block_id(902),
            edge_id(902),
            contract_id(902),
        ));
    }
    module
}

fn ordered_one_executable_nominal_affine_fixture() -> TerminalModule {
    let mut module = ordered_empty_nominal_affine_fixture(false);
    let helper_type = structural_type_id(902);
    module.structural_types.push(StructuralTypeDeclaration {
        id: helper_type,
        identity: "test::Helper".into(),
        shape: StructuralTypeShape::Record { fields: Vec::new() },
    });
    let mut helper = module.machines[1].clone();
    helper.id = machine_id(903);
    helper.attachment = Some(helper_type);
    helper.entry = block_id(903);
    helper.blocks[0].id = block_id(903);
    helper.blocks[0].terminator = Terminator::ReturnUnit {
        edge: edge_id(903),
        trivial_affine_discards: Vec::new(),
    };
    helper.contract.id = contract_id(903);
    module.machines[2].blocks[0].operations.push(Operation {
        id: operation_id(903),
        result: OperationResult::Unit,
        kind: OperationKind::CallUnit {
            callee: helper.id,
            structural_arguments: Vec::new(),
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
    });
    module.machines.push(helper);
    module
}

fn ordered_two_distinct_executable_nominal_affine_fixture() -> TerminalModule {
    let mut module = ordered_one_executable_nominal_affine_fixture();
    let helper_type = structural_type_id(903);
    module.structural_types.push(StructuralTypeDeclaration {
        id: helper_type,
        identity: "test::SecondHelper".into(),
        shape: StructuralTypeShape::Record { fields: Vec::new() },
    });
    let mut helper = module.machines[3].clone();
    helper.id = machine_id(904);
    helper.attachment = Some(helper_type);
    helper.entry = block_id(904);
    helper.blocks[0].id = block_id(904);
    helper.blocks[0].terminator = Terminator::ReturnUnit {
        edge: edge_id(904),
        trivial_affine_discards: Vec::new(),
    };
    helper.contract.id = contract_id(904);
    module.machines[1].blocks[0].operations.push(Operation {
        id: operation_id(904),
        result: OperationResult::Unit,
        kind: OperationKind::CallUnit {
            callee: helper.id,
            structural_arguments: Vec::new(),
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
    });
    module.machines.push(helper);
    module
}

fn ordered_shared_executable_nominal_affine_fixture() -> TerminalModule {
    let mut module = ordered_empty_nominal_affine_fixture(true);
    let helper_type = structural_type_id(901);
    module.structural_types.push(StructuralTypeDeclaration {
        id: helper_type,
        identity: "test::SharedHelper".into(),
        shape: StructuralTypeShape::Record { fields: Vec::new() },
    });
    let mut helper = module.machines[1].clone();
    helper.id = machine_id(902);
    helper.attachment = Some(helper_type);
    helper.entry = block_id(902);
    helper.blocks[0].id = block_id(902);
    helper.blocks[0].terminator = Terminator::ReturnUnit {
        edge: edge_id(902),
        trivial_affine_discards: Vec::new(),
    };
    helper.contract.id = contract_id(902);
    module.machines[1].blocks[0].operations.push(Operation {
        id: operation_id(902),
        result: OperationResult::Unit,
        kind: OperationKind::CallUnit {
            callee: helper.id,
            structural_arguments: Vec::new(),
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
    });
    module.machines.push(helper);
    module
}

fn three_ordered_shared_executable_nominal_affine_fixture() -> TerminalModule {
    let mut module = ordered_shared_executable_nominal_affine_fixture();
    let caller = &mut module.machines[0];
    caller
        .structural_parameters
        .push(StructuralParameterDeclaration {
            access: StructuralAccess::Owned,
            place: place_id(902),
            position: 2,
            is_self: false,
            structural_type: structural_type_id(900),
            multiplicity: StructuralMultiplicity::Affine,
            qualifications: Vec::new(),
        });
    caller.structural_places.push(StructuralPlaceDeclaration {
        id: place_id(902),
        kind: psi_core::StructuralPlaceKind::Parameter {
            position: 2,
            is_self: false,
        },
    });
    let Terminator::ReturnUnitNominalAffine { cleanups, .. } = &mut caller.blocks[0].terminator
    else {
        unreachable!("ordered fixture retains nominal cleanup")
    };
    cleanups.insert(
        0,
        NominalAffineCleanup {
            place: place_id(902),
            structural_type: structural_type_id(900),
            cleanup_machine: machine_id(901),
            cleanup_receiver: None,
            requirement_obligations: Vec::new(),
        },
    );
    module
}

fn five_ordered_shared_executable_nominal_affine_fixture() -> TerminalModule {
    let mut module = three_ordered_shared_executable_nominal_affine_fixture();
    let caller = &mut module.machines[0];
    for position in 3_u32..5 {
        let place = place_id(u64::from(position) + 900);
        caller
            .structural_parameters
            .push(StructuralParameterDeclaration {
                access: StructuralAccess::Owned,
                place,
                position,
                is_self: false,
                structural_type: structural_type_id(900),
                multiplicity: StructuralMultiplicity::Affine,
                qualifications: Vec::new(),
            });
        caller.structural_places.push(StructuralPlaceDeclaration {
            id: place,
            kind: psi_core::StructuralPlaceKind::Parameter {
                position,
                is_self: false,
            },
        });
        let Terminator::ReturnUnitNominalAffine { cleanups, .. } = &mut caller.blocks[0].terminator
        else {
            unreachable!()
        };
        cleanups.insert(
            0,
            NominalAffineCleanup {
                place,
                structural_type: structural_type_id(900),
                cleanup_machine: machine_id(901),
                cleanup_receiver: None,
                requirement_obligations: Vec::new(),
            },
        );
    }
    module
}

fn executable_nominal_affine_fixture() -> TerminalModule {
    let empty_contract = |raw| MachineContract {
        id: contract_id(raw),
        crash_routes: Vec::new(),
        requires: Vec::new(),
        ensures: Vec::new(),
        outcome_specific_ensures: Vec::new(),
    };
    let token_type = structural_type_id(900);
    let helper_type = structural_type_id(901);
    let source = place_id(900);
    let mut module = unit_fixture();
    module.structural_types = vec![
        StructuralTypeDeclaration {
            id: token_type,
            identity: "test::Token".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![StructuralFieldDeclaration {
                    identity: "payload".into(),
                    id: psi_core::StructuralFieldId::new(900).unwrap(),
                    field_type: StructuralFieldType::Scalar(ScalarType::Integer(
                        IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                    )),
                    relevance: psi_terminal::BindingRelevance::Relevant,
                }],
            },
        },
        StructuralTypeDeclaration {
            id: helper_type,
            identity: "test::Helper".into(),
            shape: StructuralTypeShape::Record { fields: Vec::new() },
        },
    ];
    let caller = &mut module.machines[0];
    caller.structural_parameters = vec![StructuralParameterDeclaration {
        access: StructuralAccess::Owned,
        place: source,
        position: 0,
        is_self: false,
        structural_type: token_type,
        multiplicity: StructuralMultiplicity::Affine,
        qualifications: Vec::new(),
    }];
    caller.structural_places = vec![StructuralPlaceDeclaration {
        id: source,
        kind: psi_core::StructuralPlaceKind::Parameter {
            position: 0,
            is_self: false,
        },
    }];
    caller.blocks[0].terminator = Terminator::ReturnUnitNominalAffine {
        edge: edge_id(900),
        cleanups: vec![NominalAffineCleanup {
            place: source,
            structural_type: token_type,
            cleanup_machine: machine_id(901),
            cleanup_receiver: None,
            requirement_obligations: Vec::new(),
        }],
    };
    module.machines.push(TerminalMachine {
        id: machine_id(901),
        attachment: Some(token_type),
        parameters: Vec::new(),
        structural_parameters: Vec::new(),
        ranked_scc: None,
        result: TerminalMachineResult::Unit,
        structural_places: Vec::new(),
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: block_id(901),
        blocks: vec![Block {
            id: block_id(901),
            parameters: Vec::new(),
            operations: vec![Operation {
                id: operation_id(901),
                result: OperationResult::Unit,
                kind: OperationKind::CallUnit {
                    callee: machine_id(902),
                    structural_arguments: Vec::new(),
                    claim_transfers: Vec::new(),
                    requirement_obligations: Vec::new(),
                    crash_continuations: Vec::new(),
                },
            }],
            terminator: Terminator::ReturnUnit {
                edge: edge_id(901),
                trivial_affine_discards: Vec::new(),
            },
        }],
        contract: empty_contract(901),
    });
    module.machines.push(TerminalMachine {
        id: machine_id(902),
        attachment: Some(helper_type),
        parameters: Vec::new(),
        structural_parameters: Vec::new(),
        ranked_scc: None,
        result: TerminalMachineResult::Unit,
        structural_places: Vec::new(),
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: block_id(902),
        blocks: vec![Block {
            id: block_id(902),
            parameters: Vec::new(),
            operations: Vec::new(),
            terminator: Terminator::ReturnUnit {
                edge: edge_id(902),
                trivial_affine_discards: Vec::new(),
            },
        }],
        contract: empty_contract(902),
    });
    module
}

fn two_helper_nominal_affine_fixture() -> TerminalModule {
    let mut module = executable_nominal_affine_fixture();
    let second_helper_type = StructuralTypeDeclaration {
        id: structural_type_id(902),
        identity: "test::SecondHelper".into(),
        shape: StructuralTypeShape::Record { fields: Vec::new() },
    };
    module.structural_types.push(second_helper_type.clone());
    module.machines[1].blocks[0].operations.push(Operation {
        id: operation_id(902),
        result: OperationResult::Unit,
        kind: OperationKind::CallUnit {
            callee: machine_id(903),
            structural_arguments: Vec::new(),
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
    });
    module.machines.push(TerminalMachine {
        id: machine_id(903),
        attachment: Some(second_helper_type.id),
        parameters: Vec::new(),
        structural_parameters: Vec::new(),
        ranked_scc: None,
        result: TerminalMachineResult::Unit,
        structural_places: Vec::new(),
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: block_id(903),
        blocks: vec![Block {
            id: block_id(903),
            parameters: Vec::new(),
            operations: Vec::new(),
            terminator: Terminator::ReturnUnit {
                edge: edge_id(903),
                trivial_affine_discards: Vec::new(),
            },
        }],
        contract: MachineContract {
            id: contract_id(903),
            crash_routes: Vec::new(),
            requires: Vec::new(),
            ensures: Vec::new(),
            outcome_specific_ensures: Vec::new(),
        },
    });
    module
}

fn three_helper_nominal_affine_fixture() -> TerminalModule {
    let mut module = two_helper_nominal_affine_fixture();
    let third_helper_type = StructuralTypeDeclaration {
        id: structural_type_id(903),
        identity: "test::ThirdHelper".into(),
        shape: StructuralTypeShape::Record { fields: Vec::new() },
    };
    module.structural_types.push(third_helper_type.clone());
    module.machines[1].blocks[0].operations.push(Operation {
        id: operation_id(903),
        result: OperationResult::Unit,
        kind: OperationKind::CallUnit {
            callee: machine_id(904),
            structural_arguments: Vec::new(),
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
    });
    let mut third_helper = module.machines[2].clone();
    third_helper.id = machine_id(904);
    third_helper.attachment = Some(third_helper_type.id);
    third_helper.entry = block_id(904);
    third_helper.blocks[0].id = block_id(904);
    third_helper.blocks[0].terminator = Terminator::ReturnUnit {
        edge: edge_id(904),
        trivial_affine_discards: Vec::new(),
    };
    third_helper.contract.id = contract_id(904);
    module.machines.push(third_helper);
    module
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
        root_service_reach: psi_terminal::TerminalRootServiceReach {
            concrete: vec![service],
            installation_dependencies: Vec::new(),
        },
        placed_view_inputs: Vec::new(),
        boundary_machines: vec![BoundaryMachineDeclaration {
            id: boundary_id(1),
            identity: "test::boundary".into(),
            attachment: None,
            scalar_parameters: Vec::new(),
            structural_parameters: Vec::new(),
            result: None,
            requires: Vec::new(),
            published_service_ceiling: Vec::new(),
            program_local_root_introductions: Vec::new(),
            content_guarantees: Vec::new(),
        }],
        provider_candidates: Vec::new(),
        float_meaning_projections: Vec::new(),
        float_meaning_equalities: Vec::new(),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        evidence_terms: Vec::new(),
        evidence_contract_lanes: Vec::new(),
        proof_output_calls: Vec::new(),
        closed_conformance_applications: Vec::new(),
        quotient_correspondences: Vec::new(),
        machines: vec![
            TerminalMachine {
                id: machine_id(700),
                attachment: None,
                parameters: Vec::new(),
                structural_parameters: Vec::new(),
                ranked_scc: None,
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
                    outcome_specific_ensures: Vec::new(),
                },
            },
            TerminalMachine {
                id: machine_id(701),
                attachment: None,
                parameters: Vec::new(),
                structural_parameters: Vec::new(),
                ranked_scc: None,
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
                        kind: OperationKind::BoundaryCall {
                            boundary: boundary_id(1),
                            arguments: Vec::new(),
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
                    outcome_specific_ensures: Vec::new(),
                },
            },
        ],
    }
}

fn write_only_primitive_store_fixture() -> TerminalModule {
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let scalar_type = ScalarType::Integer(integer);
    let structural_type = structural_type_id(960);
    let caller_place = place_id(960);
    let callee_place = place_id(961);
    let parameter = |place| StructuralParameterDeclaration {
        place,
        position: 0,
        is_self: false,
        structural_type,
        multiplicity: StructuralMultiplicity::Unrestricted,
        access: StructuralAccess::WriteOnlyBorrow,
        qualifications: Vec::new(),
    };
    let structural_place = |id| StructuralPlaceDeclaration {
        id,
        kind: psi_core::StructuralPlaceKind::Parameter {
            position: 0,
            is_self: false,
        },
    };
    let mut module = unit_effect_fixture();
    module.structural_types = vec![StructuralTypeDeclaration {
        id: structural_type,
        identity: "test::WriteOnlyU8".into(),
        shape: StructuralTypeShape::PrimitiveScalar(scalar_type),
    }];
    module.services.clear();
    module.root_service_reach = Default::default();
    module.boundary_machines.clear();

    let caller = &mut module.machines[0];
    caller.structural_parameters = vec![parameter(caller_place)];
    caller.structural_places = vec![structural_place(caller_place)];
    caller.published_service_ceiling.clear();
    caller.blocks[0].operations = vec![Operation {
        id: operation_id(700),
        result: OperationResult::Unit,
        kind: OperationKind::CallUnit {
            callee: machine_id(701),
            structural_arguments: vec![StructuralArgument {
                place: caller_place,
                path: Vec::new(),
                access: StructuralAccess::WriteOnlyBorrow,
            }],
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
    }];

    let callee = &mut module.machines[1];
    callee.structural_parameters = vec![parameter(callee_place)];
    callee.structural_places = vec![structural_place(callee_place)];
    callee.blocks[0].operations = vec![
        Operation {
            id: operation_id(702),
            result: OperationResult::Scalar(ValueDeclaration {
                id: value_id(960),
                scalar_type,
            }),
            kind: OperationKind::IntegerConstant {
                value: IntegerValue::Unsigned(7),
            },
        },
        Operation {
            id: operation_id(703),
            result: OperationResult::Unit,
            kind: OperationKind::WriteOnlyPrimitiveStore {
                destination: callee_place,
                value: value_id(960),
            },
        },
    ];
    module
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
        root_service_reach: Default::default(),
        placed_view_inputs: Vec::new(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        float_meaning_projections: Vec::new(),
        float_meaning_equalities: Vec::new(),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        evidence_terms: Vec::new(),
        evidence_contract_lanes: Vec::new(),
        proof_output_calls: Vec::new(),
        closed_conformance_applications: Vec::new(),
        quotient_correspondences: Vec::new(),
        machines: vec![TerminalMachine {
            id: machine_id(1),
            attachment: None,
            structural_parameters: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            parameters: Vec::new(),
            ranked_scc: None,
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
                        trivial_affine_discards: Vec::new(),
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
                        cleanup_actions: Vec::new(),
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
                outcome_specific_ensures: Vec::new(),
            },
        }],
    };
    let proof = ProofBundle {
        evidence_producers: Vec::new(),
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
        outcome_specific_ensures: Vec::new(),
    };
    TerminalModule {
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: machine_id(1),
        structural_types: Vec::new(),
        structural_domains: Vec::new(),
        services: Vec::new(),
        root_service_reach: Default::default(),
        placed_view_inputs: Vec::new(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        float_meaning_projections: Vec::new(),
        float_meaning_equalities: Vec::new(),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        evidence_terms: Vec::new(),
        evidence_contract_lanes: Vec::new(),
        proof_output_calls: Vec::new(),
        closed_conformance_applications: Vec::new(),
        quotient_correspondences: Vec::new(),
        machines: vec![
            TerminalMachine {
                id: machine_id(1),
                attachment: None,
                structural_parameters: Vec::new(),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                parameters: Vec::new(),
                ranked_scc: None,
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
                        cleanup_actions: Vec::new(),
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
                ranked_scc: None,
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
                        cleanup_actions: Vec::new(),
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
id_constructor!(place_id, PlaceId);
id_constructor!(claim_id, ClaimId);
id_constructor!(structural_type_id, StructuralTypeId);
id_constructor!(structural_case_id, StructuralCaseId);
