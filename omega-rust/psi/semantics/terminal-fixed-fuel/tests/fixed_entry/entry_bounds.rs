use super::{
    executable_nominal_affine_fixture, five_ordered_shared_executable_nominal_affine_fixture,
    fixture, ordered_empty_nominal_affine_fixture, ordered_one_executable_nominal_affine_fixture,
    ordered_shared_executable_nominal_affine_fixture,
    ordered_two_distinct_executable_nominal_affine_fixture, three_helper_nominal_affine_fixture,
    three_ordered_shared_executable_nominal_affine_fixture, two_helper_nominal_affine_fixture,
    unit_fixture,
};
use crate::{
    block_id, claim_id, contract_id, edge_id, machine_id, obligation_id, operation_id, place_id,
    structural_case_id, structural_type_id, value_id,
};
use proof_admission::{
    AdmissionProfile, CertificateEnvelope, EvidenceRoute, ProofNode, ProofRule, ProofSystemMarker,
};
use semantic_vocabulary::{
    EvidenceIdentity, IntegerSign, IntegerType, Proposition, ScalarTerm, ScalarType,
};
use terminal_codec::{decode_module, encode_module, terminal_psi_identity};
use terminal_fixed_fuel::{
    derive_fixed_entry_fuel, derive_fixed_safe_point_segments, derive_fixed_segment_fuel,
    validate_fixed_entry_fuel, validate_fixed_segment_fuel,
};
use terminal_psi::{
    Block, EntryClaim, MachineContract, NominalAffineCleanup, Operation, OperationKind,
    OperationResult, StructuralAccess, StructuralFieldDeclaration, StructuralFieldType,
    StructuralMultiplicity, StructuralOperationResult, StructuralParameterDeclaration,
    StructuralPlaceDeclaration, StructuralResultDeclaration, StructuralTypeDeclaration,
    StructuralTypeShape, TerminalAffineCleanupAction, TerminalMachine, TerminalMachineResult,
    Terminator, ValueDeclaration,
};
use terminal_verifier::{
    ObligationEvidence, ProofBundle, reconstruct_operation_obligations, verify_module,
};

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
            cases: vec![terminal_psi::StructuralCaseDeclaration {
                id: result_case,
                identity: "Success".into(),
                fields: Vec::new(),
            }],
        },
    }];
    let machine = &mut module.machines[0];
    machine.result = TerminalMachineResult::Structural(StructuralResultDeclaration {
        reference_sources: Vec::new(),
        place: result_place,
        structural_type,
        multiplicity: StructuralMultiplicity::Unrestricted,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    });
    machine.structural_places = vec![
        StructuralPlaceDeclaration {
            id: operation_place,
            kind: semantic_vocabulary::StructuralPlaceKind::OperationResult {
                producer: operation_id(910),
                structural_type,
            },
        },
        StructuralPlaceDeclaration {
            id: result_place,
            kind: semantic_vocabulary::StructuralPlaceKind::Result,
        },
    ];
    machine.blocks[0].operations = vec![Operation {
        static_reach_binding: None,
        id: operation_id(910),
        result: OperationResult::Structural(StructuralOperationResult {
            place: operation_place,
            structural_type,
            multiplicity: StructuralMultiplicity::Unrestricted,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            claims: Vec::new(),
        }),
        kind: OperationKind::EstablishScalarCase {
            result_case,
            fields: Vec::new(),
        },
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
            closed_reach_application: None,
            declared_service_reach: Vec::new(),
            id: machine_id(900),
            attachment: None,
            structural_parameters: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            parameters: Vec::new(),
            ranked_scc: None,
            result: TerminalMachineResult::Structural(StructuralResultDeclaration {
                reference_sources: Vec::new(),
                place: caller_result_place,
                structural_type,
                multiplicity: StructuralMultiplicity::Unrestricted,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
            }),
            structural_places: vec![
                StructuralPlaceDeclaration {
                    id: caller_operation_place,
                    kind: semantic_vocabulary::StructuralPlaceKind::OperationResult {
                        producer: caller_operation,
                        structural_type,
                    },
                },
                StructuralPlaceDeclaration {
                    id: caller_result_place,
                    kind: semantic_vocabulary::StructuralPlaceKind::Result,
                },
            ],
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: block_id(900),
            blocks: vec![Block {
                erased_scalar_formals: Vec::new(),
                structural_parameters: Vec::new(),
                id: block_id(900),
                parameters: Vec::new(),
                operations: vec![Operation {
                    static_reach_binding: None,
                    id: caller_operation,
                    result: OperationResult::Structural(StructuralOperationResult {
                        place: caller_operation_place,
                        structural_type,
                        multiplicity: StructuralMultiplicity::Unrestricted,
                        qualifications: Vec::new(),
                        projected_qualifications: Vec::new(),
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
                erased_scalar_formals: Vec::new(),
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
                    id: semantic_vocabulary::StructuralFieldId::new(index).unwrap(),
                    field_type: StructuralFieldType::Scalar(ScalarType::Integer(
                        IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                    )),
                    relevance: terminal_psi::BindingRelevance::Relevant,
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
        projected_qualifications: Vec::new(),
    }];
    caller.structural_places = vec![StructuralPlaceDeclaration {
        id: source,
        kind: semantic_vocabulary::StructuralPlaceKind::Parameter {
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
        closed_reach_application: None,
        declared_service_reach: Vec::new(),
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
            erased_scalar_formals: Vec::new(),
            structural_parameters: Vec::new(),
            id: block_id(901),
            parameters: Vec::new(),
            operations: Vec::new(),
            terminator: Terminator::ReturnUnit {
                edge: edge_id(901),
                trivial_affine_discards: Vec::new(),
            },
        }],
        contract: MachineContract {
            erased_scalar_formals: Vec::new(),
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
        qualifications: Default::default(),
        id: value_id(900),
        scalar_type: ScalarType::Boolean,
    }];
    caller.result = TerminalMachineResult::Scalar(ValueDeclaration {
        qualifications: Default::default(),
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

/// Crossing a nominal-affine return edge suspends into each cleanup machine
/// in order; that suspended work is metered before control leaves the machine,
/// so the segment ending at the edge must bound it exactly like the entry
/// bound does.
#[test]
fn nominal_affine_cleanups_compose_into_the_return_edge_segment() {
    let module = ordered_empty_nominal_affine_fixture(true);
    let verified = verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("same-target nominal cleanup module verifies");
    assert_eq!(
        derive_fixed_entry_fuel(&verified, machine_id(900))
            .expect("the entry bound counts both cleanup invocations")
            .ceiling_units(),
        3
    );

    let segment =
        derive_fixed_segment_fuel(&verified, machine_id(900), block_id(900), edge_id(900))
            .expect("the cleanup-bearing return edge bounds its segment");
    assert_eq!(
        segment.ceiling_units(),
        3,
        "the segment charges the return edge plus both cleanup machine executions"
    );
    validate_fixed_segment_fuel(&verified, &segment).unwrap();

    let catalog = derive_fixed_safe_point_segments(&verified, machine_id(900))
        .expect("the safe-point catalog covers the return edge");
    assert_eq!(catalog.len(), 1);
    assert_eq!(catalog[0].ceiling_units(), 3);
}

#[test]
fn executable_nominal_cleanup_works_inside_the_return_edge_segment() {
    let module = ordered_one_executable_nominal_affine_fixture();
    let verified = verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("one executable ordered cleanup module verifies");
    assert_eq!(
        derive_fixed_entry_fuel(&verified, machine_id(900))
            .expect("one executable ordered cleanup has an exact fixed bound")
            .ceiling_units(),
        5
    );

    let segment =
        derive_fixed_segment_fuel(&verified, machine_id(900), block_id(900), edge_id(900))
            .expect("the cleanup-bearing return edge bounds its segment");
    assert_eq!(
        segment.ceiling_units(),
        5,
        "root edge plus the executable cleanup's call, helper edge, and drop edge, plus the empty drop"
    );
    validate_fixed_segment_fuel(&verified, &segment).unwrap();
}

/// The same composition holds through `Return`'s ordered cleanup actions:
/// `InvokeNominal` entries suspend into ordinary machines whose bounds join
/// the segment ceiling in execution order.
#[test]
fn scalar_return_segment_composes_each_cleanup_action() {
    let mut module = ordered_empty_nominal_affine_fixture(true);
    let caller = &mut module.machines[0];
    caller.parameters = vec![ValueDeclaration {
        qualifications: Default::default(),
        id: value_id(900),
        scalar_type: ScalarType::Boolean,
    }];
    caller.result = TerminalMachineResult::Scalar(ValueDeclaration {
        qualifications: Default::default(),
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

    let segment =
        derive_fixed_segment_fuel(&verified, machine_id(900), block_id(900), edge_id(900))
            .expect("the scalar return edge bounds its segment");
    assert_eq!(
        segment.ceiling_units(),
        3,
        "the scalar return edge plus both ordered cleanup invocations"
    );
    validate_fixed_segment_fuel(&verified, &segment).unwrap();
}

#[test]
fn contextual_scalar_cleanup_proof_metadata_adds_zero_fixed_fuel() {
    let mut module = ordered_empty_nominal_affine_fixture(true);
    let first = semantic_vocabulary::StructuralFieldId::new(1).expect("first field");
    let second = semantic_vocabulary::StructuralFieldId::new(2).expect("second field");
    module.structural_types[0].shape = StructuralTypeShape::Record {
        fields: [first, second]
            .into_iter()
            .map(|id| StructuralFieldDeclaration {
                id,
                identity: format!("flag_{}", id.get()),
                relevance: terminal_psi::BindingRelevance::Relevant,
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
        qualifications: Default::default(),
        id: value_id(910),
        scalar_type: ScalarType::Boolean,
    }];
    caller.result = TerminalMachineResult::Scalar(ValueDeclaration {
        qualifications: Default::default(),
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

    // A cleanup requirement obligation is discharged against the return edge's
    // live observation set, not the caller's permanent assumption list: the
    // verifier treats owned-field entry requirements as validity-scoped
    // observations (terminal-verifier README, "Owned/mutable-field entry
    // requirements enter the validity-scoped observation set"), so the caller
    // premise for an owned root is absent from the `Assumption` roster and is
    // cited as the `SemanticAxiom` that the reconstructed cleanup question
    // still observes at the edge. The goals below name each obligation's
    // receiver-substituted proposition explicitly so the certificates prove the
    // exact reconstructed question rather than whatever the verifier emits.
    let goals = [
        (obligation_id(3), place_id(901), first),
        (obligation_id(4), place_id(901), second),
        (obligation_id(1), place_id(900), first),
        (obligation_id(2), place_id(900), second),
    ];
    let reconstructed =
        reconstruct_operation_obligations(&module).expect("contextual scalar cleanup reconstructs");
    assert_eq!(reconstructed.len(), goals.len());
    let mut evidence = goals
        .into_iter()
        .enumerate()
        .map(|(index, (obligation, root, field))| {
            let conclusion = Proposition::Equal(
                ScalarTerm::boolean(true),
                ScalarTerm::boolean_field(root, field),
            );
            let site = reconstructed
                .iter()
                .find(|site| site.obligation.id == obligation)
                .expect("cleanup obligation is reconstructed");
            assert_eq!(site.obligation.proposition, conclusion);
            ObligationEvidence {
                obligation,
                route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                    identity: EvidenceIdentity::new(index as u64 + 1).expect("certificate"),
                    proof_system_marker: ProofSystemMarker::CURRENT,
                    proof: ProofNode {
                        rule: ProofRule::SemanticAxiom {
                            index: site
                                .semantic_axioms
                                .iter()
                                .position(|axiom| axiom == &conclusion)
                                .expect("cleanup goal is a live owned-field observation"),
                        },
                        conclusion,
                    },
                }),
            }
        })
        .collect::<Vec<_>>();
    evidence.sort_by_key(|evidence| evidence.obligation);
    let proof = ProofBundle {
        recursive_components: Vec::new(),
        control_cycles: Vec::new(),
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
        qualifications: Default::default(),
        id: value_id(910),
        scalar_type: ScalarType::Boolean,
    }];
    caller.result = TerminalMachineResult::Scalar(ValueDeclaration {
        qualifications: Default::default(),
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
            kind: semantic_vocabulary::StructuralPlaceKind::TrivialAffineLocal {
                declaration_ordinal: 0,
                structural_type: local_type,
                construction: None,
            },
        },
        StructuralPlaceDeclaration {
            id: second,
            kind: semantic_vocabulary::StructuralPlaceKind::TrivialAffineLocal {
                declaration_ordinal: 1,
                structural_type: local_type,
                construction: None,
            },
        },
    ];
    machine.blocks[0].operations = vec![
        Operation {
            static_reach_binding: None,
            id: operation_id(900),
            result: OperationResult::Unit,
            kind: OperationKind::EstablishTrivialAffineLocal { destination: first },
        },
        Operation {
            static_reach_binding: None,
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
            projected_qualifications: Vec::new(),
        },
        StructuralParameterDeclaration {
            access: StructuralAccess::Owned,
            place: first_affine,
            position: 1,
            is_self: false,
            structural_type,
            multiplicity: StructuralMultiplicity::Affine,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        },
        StructuralParameterDeclaration {
            access: StructuralAccess::Owned,
            place: second_affine,
            position: 2,
            is_self: false,
            structural_type,
            multiplicity: StructuralMultiplicity::Affine,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        },
    ];
    machine.result = TerminalMachineResult::Structural(StructuralResultDeclaration {
        reference_sources: Vec::new(),
        place: result_place,
        structural_type,
        multiplicity: StructuralMultiplicity::Linear,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    });
    machine.structural_places = vec![
        StructuralPlaceDeclaration {
            id: source,
            kind: semantic_vocabulary::StructuralPlaceKind::Parameter {
                position: 0,
                is_self: false,
            },
        },
        StructuralPlaceDeclaration {
            id: result_place,
            kind: semantic_vocabulary::StructuralPlaceKind::Result,
        },
        StructuralPlaceDeclaration {
            id: first_affine,
            kind: semantic_vocabulary::StructuralPlaceKind::Parameter {
                position: 1,
                is_self: false,
            },
        },
        StructuralPlaceDeclaration {
            id: second_affine,
            kind: semantic_vocabulary::StructuralPlaceKind::Parameter {
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
        projected_qualifications: Vec::new(),
    }];
    machine.result = TerminalMachineResult::Structural(StructuralResultDeclaration {
        reference_sources: Vec::new(),
        place: result_place,
        structural_type,
        multiplicity: StructuralMultiplicity::Linear,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    });
    machine.structural_places = vec![
        StructuralPlaceDeclaration {
            id: source,
            kind: semantic_vocabulary::StructuralPlaceKind::Parameter {
                position: 0,
                is_self: false,
            },
        },
        StructuralPlaceDeclaration {
            id: result_place,
            kind: semantic_vocabulary::StructuralPlaceKind::Result,
        },
        StructuralPlaceDeclaration {
            id: local,
            kind: semantic_vocabulary::StructuralPlaceKind::TrivialAffineLocal {
                declaration_ordinal: 0,
                structural_type: local_type,
                construction: None,
            },
        },
        StructuralPlaceDeclaration {
            id: second_local,
            kind: semantic_vocabulary::StructuralPlaceKind::TrivialAffineLocal {
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
            static_reach_binding: None,
            id: operation_id(900),
            result: OperationResult::Unit,
            kind: OperationKind::EstablishTrivialAffineLocal { destination: local },
        },
        Operation {
            static_reach_binding: None,
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
