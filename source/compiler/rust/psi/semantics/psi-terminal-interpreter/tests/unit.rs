use psi_core::{
    BlockId, BoundaryMachineId, ClaimId, ContractId, EdgeId, EvidenceIdentity, MachineId,
    ObligationId, OperationId, PlaceId, Proposition, ScalarTerm, ScalarType, ServiceId,
    StructuralCaseId, StructuralDomainId, StructuralTypeId, ValueId,
};
use psi_proof_admission::{
    AdmissionProfile, CertificateEnvelope, EvidenceRoute, ProofNode, ProofRule, ProofSystemMarker,
};
use psi_terminal::{
    BindingRelevance, Block, BoundaryMachineDeclaration, ByteSequenceCarrier, ClaimTransfer,
    CompletionReceipt, CrashCause, CrashRouteBucket, CrashRouteGuard, EntryClaim, MachineContract,
    NominalAffineCleanup, Operation, OperationKind, OperationResult, ServiceDeclaration,
    StructuralAccess, StructuralAffineDiscard, StructuralArgument, StructuralDomainDeclaration,
    StructuralDomainRequirement, StructuralFieldDeclaration, StructuralFieldType,
    StructuralMultiplicity, StructuralOperationResult, StructuralParameterDeclaration,
    StructuralPathSegment, StructuralPlaceDeclaration, StructuralResultClaimBinding,
    StructuralResultClaimTransfer, StructuralResultDeclaration, StructuralTypeDeclaration,
    StructuralTypeShape, SuccessorEdge, TerminalAffineCleanupAction, TerminalMachine,
    TerminalMachineResult, TerminalModule, Terminator, ValueDeclaration, VocabularyMarker,
};
use psi_terminal_codec::{decode_module, encode_module, encode_proof_bundle};
use psi_terminal_fuel::{FuelChargeSite, FuelExhaustion, TerminalFuelMeter, TerminalFuelSchedule};
use psi_terminal_interpreter::{
    TerminalEffect, TerminalEffectHandler, TerminalEffectRejection, TerminalExecution,
    TerminalExecutionResult, TerminalExecutionStatus, TerminalInterpretError,
    TerminalPayloadlessCaseResult, TerminalPayloadlessCaseValue, TerminalScalarValue,
    TerminalStructuralValue, interpret_terminal_artifact_measured,
    interpret_terminal_artifact_with_effect_handler_measured,
};
use psi_terminal_verifier::{ObligationEvidence, ProofBundle};

#[test]
fn unit_artifact_interprets_as_a_value_less_normal_result() {
    let (semantic, proof) = artifact_sections();
    let measured =
        interpret_terminal_artifact_measured(&semantic, &proof, &AdmissionProfile::default(), &[])
            .expect("unit artifact should interpret");

    assert_eq!(measured.value(), TerminalExecutionResult::Unit);
    assert_eq!(measured.usage().total_units(), 1);
    assert_eq!(
        measured
            .usage()
            .at(FuelChargeSite::Edge(edge_id(1)))
            .unwrap()
            .units(),
        1
    );
}

#[test]
fn payloadless_case_construction_returns_exact_case_and_costs_one_operation() {
    let structural_type = structural_type_id(1);
    let result_case = structural_case_id(1);
    let operation_place = place_id(1);
    let result_place = place_id(2);
    let mut module = unit_module();
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
                producer: operation_id(1),
                structural_type,
            },
        },
        StructuralPlaceDeclaration {
            id: result_place,
            kind: psi_core::StructuralPlaceKind::Result,
        },
    ];
    machine.blocks[0].operations = vec![Operation {
        id: operation_id(1),
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
        edge: edge_id(1),
        source: operation_place,
        returned_claims: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };

    let semantic = encode_module(&module).expect("payloadless case semantics encode");
    let proof = encode_proof_bundle(&ProofBundle::default()).expect("empty proof encodes");
    let measured =
        interpret_terminal_artifact_measured(&semantic, &proof, &AdmissionProfile::default(), &[])
            .expect("verified payloadless case executes");

    assert_eq!(
        measured.value(),
        TerminalExecutionResult::PayloadlessCase(TerminalPayloadlessCaseResult {
            value: TerminalPayloadlessCaseValue {
                structural_type,
                result_case,
            },
        })
    );
    assert_eq!(measured.usage().total_units(), 2);
    assert_eq!(
        measured
            .usage()
            .at(FuelChargeSite::Operation(operation_id(1)))
            .unwrap()
            .units(),
        1
    );
}

#[test]
fn unit_return_fuel_exhaustion_resumes_without_advancing_or_double_charging() {
    let (semantic, proof) = artifact_sections();
    let mut execution =
        TerminalExecution::start_artifact(&semantic, &proof, &AdmissionProfile::default(), &[])
            .expect("unit artifact should start");
    let mut meter = TerminalFuelMeter::with_allowance(0);

    assert_eq!(
        execution.resume(&mut meter).unwrap(),
        TerminalExecutionStatus::SponsorExhausted(FuelExhaustion {
            schedule: TerminalFuelSchedule::CURRENT.identity(),
            site: FuelChargeSite::Edge(edge_id(1)),
            required_units: 1,
            remaining_units: 0,
        })
    );
    assert_eq!(meter.usage().total_units(), 0);

    meter.replenish(1).unwrap();
    assert_eq!(
        execution.resume(&mut meter).unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
    assert_eq!(meter.usage().total_units(), 1);
    assert_eq!(
        execution.resume(&mut meter).unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
    assert_eq!(meter.usage().total_units(), 1);
}

#[test]
fn structural_return_transfers_value_and_claim_atomically_after_edge_charge() {
    let structural_type = structural_type_id(1);
    let domain = structural_domain_id(1);
    let source = place_id(1);
    let result_place = place_id(2);
    let claim = claim_id(1);
    let edge = edge_id(1);
    let module = TerminalModule {
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: machine_id(1),
        structural_types: vec![StructuralTypeDeclaration {
            id: structural_type,
            identity: "test::Resource".into(),
            shape: StructuralTypeShape::Record { fields: Vec::new() },
        }],
        structural_domains: vec![StructuralDomainDeclaration {
            id: domain,
            semantic_domain: psi_core::DomainSemanticId::new(1).expect("semantic domain identity"),
            identity: "test::Owned".into(),
            carrier: structural_type,
            content_projection: None,
        }],
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
        proof_output_calls: Vec::new(),
        closed_conformance_applications: Vec::new(),
        machines: vec![TerminalMachine {
            id: machine_id(1),
            attachment: None,
            parameters: Vec::new(),
            structural_parameters: vec![StructuralParameterDeclaration {
                place: source,
                position: 0,
                is_self: false,
                structural_type,
                multiplicity: StructuralMultiplicity::Linear,
                access: StructuralAccess::Owned,
                qualifications: vec![domain],
            }],
            result: TerminalMachineResult::Structural(StructuralResultDeclaration {
                place: result_place,
                structural_type,
                multiplicity: StructuralMultiplicity::Linear,
                qualifications: vec![domain],
            }),
            structural_places: vec![
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
            ],
            entry_claims: vec![EntryClaim {
                claim,
                input: source,
                path: Vec::new(),
            }],
            published_service_ceiling: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: block_id(1),
            blocks: vec![Block {
                id: block_id(1),
                parameters: Vec::new(),
                operations: Vec::new(),
                terminator: Terminator::ReturnStructural {
                    edge,
                    source,
                    returned_claims: vec![claim],
                    trivial_affine_discards: Vec::new(),
                },
            }],
            contract: empty_contract(contract_id(1)),
        }],
    };
    let semantic = encode_module(&module).expect("structural return encodes");
    let proof = encode_proof_bundle(&ProofBundle::default()).expect("empty proof encodes");
    let argument = TerminalStructuralValue {
        opaque_identity: 0x5eed,
        structural_type,
        qualifications: vec![domain],
        path: Vec::new(),
    };
    let mut execution = TerminalExecution::start_artifact_with_structural_arguments(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[],
        std::slice::from_ref(&argument),
    )
    .expect("verified structural return starts");
    let mut meter = TerminalFuelMeter::with_allowance(0);

    assert_eq!(
        execution.resume(&mut meter).unwrap(),
        TerminalExecutionStatus::SponsorExhausted(FuelExhaustion {
            schedule: TerminalFuelSchedule::CURRENT.identity(),
            site: FuelChargeSite::Edge(edge),
            required_units: 1,
            remaining_units: 0,
        })
    );
    assert_eq!(
        execution.live_claim_frontier().collect::<Vec<_>>(),
        vec![claim]
    );
    meter.replenish(1).unwrap();
    assert_eq!(
        execution.resume(&mut meter).unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Structural(
            psi_terminal_interpreter::TerminalStructuralResult {
                value: argument,
                claims: vec![claim],
            }
        ))
    );
    assert!(execution.live_claim_frontier().next().is_none());
    assert_eq!(meter.usage().total_units(), 1);
}

#[test]
fn internal_structural_call_rebinds_claim_and_preserves_value_identity() {
    let module = internal_structural_call_module(false);
    let semantic = encode_module(&module).expect("structural call encodes");
    let proof = encode_proof_bundle(&ProofBundle::default()).expect("empty proof encodes");
    let argument = TerminalStructuralValue {
        opaque_identity: 0xc011,
        structural_type: structural_type_id(1),
        qualifications: vec![structural_domain_id(1)],
        path: Vec::new(),
    };

    let measured =
        psi_terminal_interpreter::interpret_terminal_artifact_with_effect_handler_measured(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            &[],
            std::slice::from_ref(&argument),
            &mut psi_terminal_interpreter::AcceptTerminalEffects,
        )
        .expect("whole-root structural call should interpret");

    assert_eq!(
        measured.value(),
        TerminalExecutionResult::Structural(psi_terminal_interpreter::TerminalStructuralResult {
            value: argument,
            claims: vec![claim_id(1)],
        })
    );
    assert_eq!(measured.usage().total_units(), 3);
}

#[test]
fn internal_structural_call_resumes_at_each_charge_without_replaying_custody() {
    let module = internal_structural_call_module(false);
    let semantic = encode_module(&module).expect("structural call encodes");
    let proof = encode_proof_bundle(&ProofBundle::default()).expect("empty proof encodes");
    let argument = TerminalStructuralValue {
        opaque_identity: 0xc012,
        structural_type: structural_type_id(1),
        qualifications: vec![structural_domain_id(1)],
        path: Vec::new(),
    };
    let mut execution = TerminalExecution::start_artifact_with_structural_arguments(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[],
        std::slice::from_ref(&argument),
    )
    .expect("verified structural call starts");
    let mut meter = TerminalFuelMeter::with_allowance(0);

    assert!(matches!(
        execution.resume(&mut meter).unwrap(),
        TerminalExecutionStatus::SponsorExhausted(FuelExhaustion {
            site: FuelChargeSite::Operation(operation),
            ..
        }) if operation == operation_id(1)
    ));
    assert_eq!(
        execution.live_claim_frontier().collect::<Vec<_>>(),
        vec![claim_id(1)]
    );
    assert_eq!(meter.usage().total_units(), 0);

    meter.replenish(1).unwrap();
    assert!(matches!(
        execution.resume(&mut meter).unwrap(),
        TerminalExecutionStatus::SponsorExhausted(FuelExhaustion {
            site: FuelChargeSite::Edge(edge),
            ..
        }) if edge == edge_id(2)
    ));
    assert_eq!(
        execution.live_claim_frontier().collect::<Vec<_>>(),
        vec![claim_id(1)]
    );
    assert_eq!(meter.usage().total_units(), 1);

    meter.replenish(1).unwrap();
    assert!(matches!(
        execution.resume(&mut meter).unwrap(),
        TerminalExecutionStatus::SponsorExhausted(FuelExhaustion {
            site: FuelChargeSite::Edge(edge),
            ..
        }) if edge == edge_id(1)
    ));
    assert_eq!(
        execution.live_claim_frontier().collect::<Vec<_>>(),
        vec![claim_id(1)]
    );
    assert_eq!(meter.usage().total_units(), 2);

    meter.replenish(1).unwrap();
    assert_eq!(
        execution.resume(&mut meter).unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Structural(
            psi_terminal_interpreter::TerminalStructuralResult {
                value: argument,
                claims: vec![claim_id(1)],
            }
        ))
    );
    assert!(execution.live_claim_frontier().next().is_none());
    assert_eq!(meter.usage().total_units(), 3);
}

#[test]
fn internal_multi_claim_structural_call_resumes_without_replaying_or_swapping_claims() {
    let module = multi_claim_internal_structural_call_module(false);
    let semantic = encode_module(&module).expect("multi-claim structural call encodes");
    let proof = encode_proof_bundle(&ProofBundle::default()).expect("empty proof encodes");
    let argument = TerminalStructuralValue {
        opaque_identity: 0xc014,
        structural_type: structural_type_id(1),
        qualifications: vec![structural_domain_id(1)],
        path: Vec::new(),
    };
    let mut execution = TerminalExecution::start_artifact_with_structural_arguments(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[],
        std::slice::from_ref(&argument),
    )
    .expect("verified multi-claim structural call starts");
    let mut meter = TerminalFuelMeter::with_allowance(0);

    for (expected_site, expected_usage) in [
        (FuelChargeSite::Operation(operation_id(1)), 0),
        (FuelChargeSite::Edge(edge_id(2)), 1),
        (FuelChargeSite::Edge(edge_id(1)), 2),
    ] {
        assert!(matches!(
            execution.resume(&mut meter).unwrap(),
            TerminalExecutionStatus::SponsorExhausted(FuelExhaustion { site, .. })
                if site == expected_site
        ));
        assert_eq!(
            execution.live_claim_frontier().collect::<Vec<_>>(),
            vec![claim_id(1), claim_id(2)]
        );
        assert_eq!(meter.usage().total_units(), expected_usage);
        meter.replenish(1).unwrap();
    }

    assert_eq!(
        execution.resume(&mut meter).unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Structural(
            psi_terminal_interpreter::TerminalStructuralResult {
                value: argument,
                claims: vec![claim_id(1), claim_id(2)],
            }
        ))
    );
    assert!(execution.live_claim_frontier().next().is_none());
    assert_eq!(meter.usage().total_units(), 3);
}

#[test]
fn crashing_structural_callee_never_produces_a_caller_result() {
    let module = internal_structural_call_module(true);
    let semantic = encode_module(&module).expect("crashing structural call encodes");
    let proof = encode_proof_bundle(&ProofBundle::default()).expect("empty proof encodes");
    let argument = TerminalStructuralValue {
        opaque_identity: 0xc013,
        structural_type: structural_type_id(1),
        qualifications: vec![structural_domain_id(1)],
        path: Vec::new(),
    };
    let mut execution = TerminalExecution::start_artifact_with_structural_arguments(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[],
        std::slice::from_ref(&argument),
    )
    .expect("verified crashing structural call starts");
    let mut meter = TerminalFuelMeter::unbounded();

    let crashed = execution.resume(&mut meter).unwrap();
    assert!(matches!(
        &crashed,
        TerminalExecutionStatus::Crashed(crash)
            if crash.edge == edge_id(2)
                && crash.frontier_lower_bound == vec![claim_id(1)]
    ));
    assert_eq!(execution.resume(&mut meter).unwrap(), crashed);
    assert_eq!(meter.usage().total_units(), 2);
}

#[test]
fn crashing_multi_claim_structural_callee_preserves_the_exact_abandonment_frontier() {
    let module = multi_claim_internal_structural_call_module(true);
    let semantic = encode_module(&module).expect("crashing multi-claim call encodes");
    let proof = encode_proof_bundle(&ProofBundle::default()).expect("empty proof encodes");
    let argument = TerminalStructuralValue {
        opaque_identity: 0xc015,
        structural_type: structural_type_id(1),
        qualifications: vec![structural_domain_id(1)],
        path: Vec::new(),
    };
    let mut execution = TerminalExecution::start_artifact_with_structural_arguments(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[],
        std::slice::from_ref(&argument),
    )
    .expect("verified crashing multi-claim call starts");
    let mut meter = TerminalFuelMeter::unbounded();

    let crashed = execution.resume(&mut meter).unwrap();
    assert!(matches!(
        &crashed,
        TerminalExecutionStatus::Crashed(crash)
            if crash.edge == edge_id(2)
                && crash.frontier_lower_bound == vec![claim_id(1), claim_id(2)]
    ));
    assert_eq!(execution.resume(&mut meter).unwrap(), crashed);
    assert_eq!(meter.usage().total_units(), 2);
}

#[test]
fn unit_return_performs_affine_discard_only_after_edge_charge() {
    let mut module = effect_module();
    let mut machine = module.machines.pop().expect("callee machine");
    machine.blocks[0].operations.clear();
    machine.structural_parameters[0].multiplicity = StructuralMultiplicity::Affine;
    machine.entry_claims.clear();
    let Terminator::ReturnUnit {
        trivial_affine_discards,
        ..
    } = &mut machine.blocks[0].terminator
    else {
        unreachable!()
    };
    *trivial_affine_discards = vec![place_id(2)];
    module.entry = machine.id;
    module.machines = vec![machine];
    module.root_service_reach = Default::default();
    let semantic = encode_module(&module).expect("affine cleanup module encodes");
    let proof = encode_proof_bundle(&ProofBundle::default()).expect("empty proof encodes");
    let mut execution = TerminalExecution::start_artifact_with_structural_arguments(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[],
        &[structural_value(48)],
    )
    .expect("verified affine cleanup should start");
    let mut meter = TerminalFuelMeter::with_allowance(0);

    assert!(matches!(
        execution.resume(&mut meter).unwrap(),
        TerminalExecutionStatus::SponsorExhausted(_)
    ));
    meter.replenish(1).unwrap();
    assert_eq!(
        execution.resume(&mut meter).unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
}

#[test]
fn partial_affine_return_charges_edge_before_exact_residual_cleanup() {
    let module = partial_affine_field_module();
    let semantic = encode_module(&module).expect("partial affine module encodes");
    let proof = encode_proof_bundle(&ProofBundle::default()).expect("empty proof encodes");
    let argument = TerminalStructuralValue {
        opaque_identity: 50,
        structural_type: structural_type_id(2),
        qualifications: Vec::new(),
        path: Vec::new(),
    };
    let mut execution = TerminalExecution::start_artifact_with_structural_arguments(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[],
        &[argument],
    )
    .expect("verified partial affine cleanup should start");
    // Call operation + callee return consume the first two units. The caller's
    // partial return then suspends before touching its residual path.
    let mut meter = TerminalFuelMeter::with_allowance(2);

    assert_eq!(
        execution.resume(&mut meter).unwrap(),
        TerminalExecutionStatus::SponsorExhausted(FuelExhaustion {
            schedule: TerminalFuelSchedule::CURRENT.identity(),
            site: FuelChargeSite::Edge(edge_id(1)),
            required_units: 1,
            remaining_units: 0,
        })
    );
    assert_eq!(
        execution
            .live_affine_frontier()
            .cloned()
            .collect::<Vec<_>>(),
        vec![StructuralAffineDiscard {
            place: place_id(1),
            path: vec![StructuralPathSegment::Field("left".into())],
            structural_type: structural_type_id(1),
        }]
    );

    meter.replenish(1).unwrap();
    assert_eq!(
        execution.resume(&mut meter).unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
    assert!(execution.live_affine_frontier().next().is_none());
    assert_eq!(meter.usage().total_units(), 3);
}

#[test]
fn nominal_affine_cleanup_resumes_across_both_edge_charges() {
    let mut module = nominal_affine_module();
    module.structural_types[0].shape = StructuralTypeShape::Record {
        fields: (1..=5)
            .map(|index| StructuralFieldDeclaration {
                identity: format!("payload_{index}"),
                id: psi_core::StructuralFieldId::new(index).unwrap(),
                field_type: StructuralFieldType::Scalar(ScalarType::Integer(
                    psi_core::IntegerType::new(psi_core::IntegerSign::Unsigned, 64).unwrap(),
                )),
                relevance: BindingRelevance::Relevant,
            })
            .collect(),
    };
    let semantic = encode_module(&module).expect("nominal cleanup encodes");
    let proof = encode_proof_bundle(&ProofBundle::default()).expect("empty proof encodes");
    let argument = TerminalStructuralValue {
        opaque_identity: 70,
        structural_type: structural_type_id(1),
        qualifications: Vec::new(),
        path: Vec::new(),
    };
    let mut execution = TerminalExecution::start_artifact_with_structural_arguments(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[],
        &[argument],
    )
    .expect("verified nominal cleanup should start");
    let mut meter = TerminalFuelMeter::with_allowance(0);

    assert!(matches!(
        execution.resume(&mut meter).unwrap(),
        TerminalExecutionStatus::SponsorExhausted(FuelExhaustion {
            site: FuelChargeSite::Edge(edge),
            ..
        }) if edge == edge_id(1)
    ));
    assert_eq!(execution.live_affine_frontier().count(), 1);

    meter.replenish(1).unwrap();
    assert!(matches!(
        execution.resume(&mut meter).unwrap(),
        TerminalExecutionStatus::SponsorExhausted(FuelExhaustion {
            site: FuelChargeSite::Edge(edge),
            ..
        }) if edge == edge_id(2)
    ));
    assert_eq!(meter.usage().total_units(), 1);

    meter.replenish(1).unwrap();
    assert_eq!(
        execution.resume(&mut meter).unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
    assert!(execution.live_affine_frontier().next().is_none());
    assert_eq!(meter.usage().total_units(), 2);
}

#[test]
fn ordered_nominal_affine_cleanups_run_in_reverse_parameter_order_after_one_root_charge() {
    let module = ordered_empty_nominal_affine_module(false);
    let semantic = encode_module(&module).expect("ordered nominal cleanups encode");
    let proof = encode_proof_bundle(&ProofBundle::default()).expect("empty proof encodes");
    let arguments = [
        TerminalStructuralValue {
            opaque_identity: 80,
            structural_type: structural_type_id(1),
            qualifications: Vec::new(),
            path: Vec::new(),
        },
        TerminalStructuralValue {
            opaque_identity: 81,
            structural_type: structural_type_id(2),
            qualifications: Vec::new(),
            path: Vec::new(),
        },
    ];
    let mut execution = TerminalExecution::start_artifact_with_structural_arguments(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[],
        &arguments,
    )
    .expect("verified ordered nominal cleanups should start");
    let mut meter = TerminalFuelMeter::with_allowance(0);

    for (consumed, site) in [
        FuelChargeSite::Edge(edge_id(1)),
        FuelChargeSite::Edge(edge_id(3)),
        FuelChargeSite::Edge(edge_id(2)),
    ]
    .into_iter()
    .enumerate()
    {
        assert!(matches!(
            execution.resume(&mut meter).unwrap(),
            TerminalExecutionStatus::SponsorExhausted(FuelExhaustion {
                site: exhausted_site,
                ..
            }) if exhausted_site == site
        ));
        assert_eq!(meter.usage().total_units(), consumed as u64);
        meter.replenish(1).unwrap();
    }

    assert_eq!(
        execution.resume(&mut meter).unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
    assert_eq!(meter.usage().total_units(), 3);
    assert_eq!(
        meter
            .usage()
            .at(FuelChargeSite::Edge(edge_id(1)))
            .unwrap()
            .executions(),
        1,
        "the root edge is charged once for the entire ordered cleanup list"
    );
}

#[test]
fn ordered_nominal_affine_cleanups_can_invoke_the_same_cleanup_machine_twice() {
    let module = ordered_empty_nominal_affine_module(true);
    let semantic = encode_module(&module).expect("same-target nominal cleanups encode");
    let proof = encode_proof_bundle(&ProofBundle::default()).expect("empty proof encodes");
    let arguments = [
        TerminalStructuralValue {
            opaque_identity: 82,
            structural_type: structural_type_id(1),
            qualifications: Vec::new(),
            path: Vec::new(),
        },
        TerminalStructuralValue {
            opaque_identity: 83,
            structural_type: structural_type_id(1),
            qualifications: Vec::new(),
            path: Vec::new(),
        },
    ];
    let mut execution = TerminalExecution::start_artifact_with_structural_arguments(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[],
        &arguments,
    )
    .expect("verified same-target nominal cleanups should start");
    let mut meter = TerminalFuelMeter::with_allowance(3);

    assert_eq!(
        execution.resume(&mut meter).unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
    assert_eq!(meter.usage().total_units(), 3);
    let root = meter
        .usage()
        .at(FuelChargeSite::Edge(edge_id(1)))
        .expect("root edge charged");
    let cleanup = meter
        .usage()
        .at(FuelChargeSite::Edge(edge_id(2)))
        .expect("shared cleanup edge charged");
    assert_eq!((root.executions(), root.units()), (1, 1));
    assert_eq!((cleanup.executions(), cleanup.units()), (2, 2));
}

#[test]
fn three_nominal_affine_cleanups_run_in_exact_reverse_parameter_order() {
    let module = three_ordered_empty_nominal_affine_module(false);
    let semantic = encode_module(&module).expect("three ordered nominal cleanups encode");
    let proof = encode_proof_bundle(&ProofBundle::default()).expect("empty proof encodes");
    let arguments = [
        TerminalStructuralValue {
            opaque_identity: 90,
            structural_type: structural_type_id(1),
            qualifications: Vec::new(),
            path: Vec::new(),
        },
        TerminalStructuralValue {
            opaque_identity: 91,
            structural_type: structural_type_id(2),
            qualifications: Vec::new(),
            path: Vec::new(),
        },
        TerminalStructuralValue {
            opaque_identity: 92,
            structural_type: structural_type_id(3),
            qualifications: Vec::new(),
            path: Vec::new(),
        },
    ];
    let mut execution = TerminalExecution::start_artifact_with_structural_arguments(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[],
        &arguments,
    )
    .expect("verified three-action nominal cleanup should start");
    let mut meter = TerminalFuelMeter::with_allowance(0);

    for (consumed, site) in [
        FuelChargeSite::Edge(edge_id(1)),
        FuelChargeSite::Edge(edge_id(4)),
        FuelChargeSite::Edge(edge_id(3)),
        FuelChargeSite::Edge(edge_id(2)),
    ]
    .into_iter()
    .enumerate()
    {
        assert!(matches!(
            execution.resume(&mut meter).unwrap(),
            TerminalExecutionStatus::SponsorExhausted(FuelExhaustion {
                site: exhausted_site,
                ..
            }) if exhausted_site == site
        ));
        assert_eq!(meter.usage().total_units(), consumed as u64);
        meter.replenish(1).unwrap();
    }
    assert_eq!(
        execution.resume(&mut meter).unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
    assert_eq!(meter.usage().total_units(), 4);
}

#[test]
fn ordered_nominal_affine_cleanups_run_one_executable_body_before_the_empty_action() {
    let module = ordered_one_executable_nominal_affine_module();
    let semantic = encode_module(&module).expect("ordered executable nominal cleanups encode");
    let proof = encode_proof_bundle(&ProofBundle::default()).expect("empty proof encodes");
    let arguments = [
        TerminalStructuralValue {
            opaque_identity: 84,
            structural_type: structural_type_id(1),
            qualifications: Vec::new(),
            path: Vec::new(),
        },
        TerminalStructuralValue {
            opaque_identity: 85,
            structural_type: structural_type_id(2),
            qualifications: Vec::new(),
            path: Vec::new(),
        },
    ];
    let mut execution = TerminalExecution::start_artifact_with_structural_arguments(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[],
        &arguments,
    )
    .expect("verified ordered executable nominal cleanups should start");
    let mut meter = TerminalFuelMeter::with_allowance(0);

    for (consumed, site) in [
        FuelChargeSite::Edge(edge_id(1)),
        FuelChargeSite::Operation(operation_id(1)),
        FuelChargeSite::Edge(edge_id(4)),
        FuelChargeSite::Edge(edge_id(3)),
        FuelChargeSite::Edge(edge_id(2)),
    ]
    .into_iter()
    .enumerate()
    {
        assert!(matches!(
            execution.resume(&mut meter).unwrap(),
            TerminalExecutionStatus::SponsorExhausted(FuelExhaustion {
                site: exhausted_site,
                ..
            }) if exhausted_site == site
        ));
        assert_eq!(meter.usage().total_units(), consumed as u64);
        meter.replenish(1).unwrap();
    }

    assert_eq!(
        execution.resume(&mut meter).unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
    assert_eq!(meter.usage().total_units(), 5);
}

#[test]
fn ordered_nominal_affine_cleanups_run_two_distinct_executable_bodies_in_order() {
    let module = ordered_two_distinct_executable_nominal_affine_module();
    let semantic = encode_module(&module).expect("two executable nominal cleanups encode");
    let proof = encode_proof_bundle(&ProofBundle::default()).expect("empty proof encodes");
    let arguments = [
        TerminalStructuralValue {
            opaque_identity: 86,
            structural_type: structural_type_id(1),
            qualifications: Vec::new(),
            path: Vec::new(),
        },
        TerminalStructuralValue {
            opaque_identity: 87,
            structural_type: structural_type_id(2),
            qualifications: Vec::new(),
            path: Vec::new(),
        },
    ];
    let mut execution = TerminalExecution::start_artifact_with_structural_arguments(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[],
        &arguments,
    )
    .expect("two distinct executable nominal cleanups should start");
    let mut meter = TerminalFuelMeter::with_allowance(0);

    for (consumed, site) in [
        FuelChargeSite::Edge(edge_id(1)),
        FuelChargeSite::Operation(operation_id(1)),
        FuelChargeSite::Edge(edge_id(4)),
        FuelChargeSite::Edge(edge_id(3)),
        FuelChargeSite::Operation(operation_id(2)),
        FuelChargeSite::Edge(edge_id(5)),
        FuelChargeSite::Edge(edge_id(2)),
    ]
    .into_iter()
    .enumerate()
    {
        assert!(matches!(
            execution.resume(&mut meter).unwrap(),
            TerminalExecutionStatus::SponsorExhausted(FuelExhaustion {
                site: exhausted_site,
                ..
            }) if exhausted_site == site
        ));
        assert_eq!(meter.usage().total_units(), consumed as u64);
        meter.replenish(1).unwrap();
    }
    assert_eq!(
        execution.resume(&mut meter).unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
    assert_eq!(meter.usage().total_units(), 7);
}

#[test]
fn ordered_nominal_affine_cleanups_repeat_a_shared_executable_target_and_helper() {
    let module = ordered_shared_executable_nominal_affine_module();
    let semantic = encode_module(&module).expect("shared executable nominal cleanup encodes");
    let proof = encode_proof_bundle(&ProofBundle::default()).expect("empty proof encodes");
    let arguments = [
        TerminalStructuralValue {
            opaque_identity: 88,
            structural_type: structural_type_id(1),
            qualifications: Vec::new(),
            path: Vec::new(),
        },
        TerminalStructuralValue {
            opaque_identity: 89,
            structural_type: structural_type_id(1),
            qualifications: Vec::new(),
            path: Vec::new(),
        },
    ];
    let mut execution = TerminalExecution::start_artifact_with_structural_arguments(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[],
        &arguments,
    )
    .expect("shared executable nominal cleanup should start");
    let mut meter = TerminalFuelMeter::with_allowance(7);
    assert_eq!(
        execution.resume(&mut meter).unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
    assert_eq!(meter.usage().total_units(), 7);
    for site in [
        FuelChargeSite::Operation(operation_id(1)),
        FuelChargeSite::Edge(edge_id(3)),
        FuelChargeSite::Edge(edge_id(2)),
    ] {
        assert_eq!(
            meter
                .usage()
                .at(site)
                .expect("shared site charged")
                .executions(),
            2,
            "the shared cleanup body is invoked once per cleanup action"
        );
    }
}

#[test]
fn three_nominal_affine_cleanups_repeat_a_shared_executable_body_three_times() {
    let module = three_ordered_shared_executable_nominal_affine_module();
    let semantic = encode_module(&module).expect("three shared nominal cleanups encode");
    let proof = encode_proof_bundle(&ProofBundle::default()).expect("empty proof encodes");
    let arguments = (93..96)
        .map(|opaque_identity| TerminalStructuralValue {
            opaque_identity,
            structural_type: structural_type_id(1),
            qualifications: Vec::new(),
            path: Vec::new(),
        })
        .collect::<Vec<_>>();
    let mut execution = TerminalExecution::start_artifact_with_structural_arguments(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[],
        &arguments,
    )
    .expect("three shared executable cleanups should start");
    let mut meter = TerminalFuelMeter::with_allowance(10);

    assert_eq!(
        execution.resume(&mut meter).unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
    assert_eq!(meter.usage().total_units(), 10);
    assert_eq!(
        meter
            .usage()
            .at(FuelChargeSite::Edge(edge_id(1)))
            .expect("root edge charged")
            .executions(),
        1
    );
    for site in [
        FuelChargeSite::Operation(operation_id(1)),
        FuelChargeSite::Edge(edge_id(3)),
        FuelChargeSite::Edge(edge_id(2)),
    ] {
        assert_eq!(
            meter
                .usage()
                .at(site)
                .expect("shared cleanup site charged")
                .executions(),
            3,
            "the shared body executes once per cleanup action"
        );
    }
}

#[test]
fn executable_nominal_affine_cleanup_charges_root_call_helper_and_drop_in_order() {
    let module = executable_nominal_affine_module();
    let semantic = encode_module(&module).expect("executable nominal cleanup encodes");
    let proof = encode_proof_bundle(&ProofBundle::default()).expect("empty proof encodes");
    let argument = TerminalStructuralValue {
        opaque_identity: 71,
        structural_type: structural_type_id(1),
        qualifications: Vec::new(),
        path: Vec::new(),
    };
    let mut execution = TerminalExecution::start_artifact_with_structural_arguments(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[],
        &[argument],
    )
    .expect("verified executable nominal cleanup should start");
    let mut meter = TerminalFuelMeter::with_allowance(0);

    for (site, consumed) in [
        (FuelChargeSite::Edge(edge_id(1)), 0),
        (FuelChargeSite::Operation(operation_id(1)), 1),
        (FuelChargeSite::Edge(edge_id(3)), 2),
        (FuelChargeSite::Edge(edge_id(2)), 3),
    ] {
        assert_eq!(
            execution.resume(&mut meter).unwrap(),
            TerminalExecutionStatus::SponsorExhausted(FuelExhaustion {
                schedule: TerminalFuelSchedule::CURRENT.identity(),
                site,
                required_units: 1,
                remaining_units: 0,
            })
        );
        assert_eq!(meter.usage().total_units(), consumed);
        meter.replenish(1).unwrap();
    }

    assert_eq!(
        execution.resume(&mut meter).unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
    assert!(execution.live_affine_frontier().next().is_none());
    assert_eq!(meter.usage().total_units(), 4);
    for site in [
        FuelChargeSite::Edge(edge_id(1)),
        FuelChargeSite::Operation(operation_id(1)),
        FuelChargeSite::Edge(edge_id(3)),
        FuelChargeSite::Edge(edge_id(2)),
    ] {
        let attribution = meter.usage().at(site).expect("site was charged once");
        assert_eq!(attribution.executions(), 1);
        assert_eq!(attribution.units(), 1);
    }
}

#[test]
fn two_helper_nominal_affine_cleanup_charges_all_six_sites_in_source_order() {
    let module = two_helper_nominal_affine_module();
    let semantic = encode_module(&module).expect("two-helper nominal cleanup encodes");
    let proof = encode_proof_bundle(&ProofBundle::default()).expect("empty proof encodes");
    let argument = TerminalStructuralValue {
        opaque_identity: 72,
        structural_type: structural_type_id(1),
        qualifications: Vec::new(),
        path: Vec::new(),
    };
    let mut execution = TerminalExecution::start_artifact_with_structural_arguments(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[],
        &[argument],
    )
    .expect("verified two-helper nominal cleanup should start");
    let mut meter = TerminalFuelMeter::with_allowance(0);
    let ordered_sites = [
        FuelChargeSite::Edge(edge_id(1)),
        FuelChargeSite::Operation(operation_id(1)),
        FuelChargeSite::Edge(edge_id(3)),
        FuelChargeSite::Operation(operation_id(2)),
        FuelChargeSite::Edge(edge_id(4)),
        FuelChargeSite::Edge(edge_id(2)),
    ];

    for (consumed, site) in ordered_sites.iter().copied().enumerate() {
        assert_eq!(
            execution.resume(&mut meter).unwrap(),
            TerminalExecutionStatus::SponsorExhausted(FuelExhaustion {
                schedule: TerminalFuelSchedule::CURRENT.identity(),
                site,
                required_units: 1,
                remaining_units: 0,
            })
        );
        assert_eq!(meter.usage().total_units(), consumed as u64);
        meter.replenish(1).unwrap();
    }

    assert_eq!(
        execution.resume(&mut meter).unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
    assert!(execution.live_affine_frontier().next().is_none());
    assert_eq!(meter.usage().total_units(), 6);
    for site in ordered_sites {
        let attribution = meter.usage().at(site).expect("site was charged once");
        assert_eq!(attribution.executions(), 1);
        assert_eq!(attribution.units(), 1);
    }
}

#[test]
fn three_helper_nominal_affine_cleanup_charges_all_eight_sites_in_source_order() {
    let module = three_helper_nominal_affine_module();
    let semantic = encode_module(&module).expect("three-helper nominal cleanup encodes");
    let proof = encode_proof_bundle(&ProofBundle::default()).expect("empty proof encodes");
    let argument = TerminalStructuralValue {
        opaque_identity: 73,
        structural_type: structural_type_id(1),
        qualifications: Vec::new(),
        path: Vec::new(),
    };
    let mut execution = TerminalExecution::start_artifact_with_structural_arguments(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[],
        &[argument],
    )
    .expect("verified three-helper nominal cleanup should start");
    let mut meter = TerminalFuelMeter::with_allowance(0);
    let ordered_sites = [
        FuelChargeSite::Edge(edge_id(1)),
        FuelChargeSite::Operation(operation_id(1)),
        FuelChargeSite::Edge(edge_id(3)),
        FuelChargeSite::Operation(operation_id(2)),
        FuelChargeSite::Edge(edge_id(4)),
        FuelChargeSite::Operation(operation_id(3)),
        FuelChargeSite::Edge(edge_id(5)),
        FuelChargeSite::Edge(edge_id(2)),
    ];

    for (consumed, site) in ordered_sites.iter().copied().enumerate() {
        assert!(matches!(
            execution.resume(&mut meter).unwrap(),
            TerminalExecutionStatus::SponsorExhausted(FuelExhaustion {
                site: exhausted_site,
                ..
            }) if exhausted_site == site
        ));
        assert_eq!(meter.usage().total_units(), consumed as u64);
        meter.replenish(1).unwrap();
    }
    assert_eq!(
        execution.resume(&mut meter).unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
    assert_eq!(meter.usage().total_units(), 8);
}

#[test]
fn scalar_return_performs_affine_discard_only_after_edge_charge() {
    let mut module = effect_module();
    let mut machine = module.machines.pop().expect("callee machine");
    machine.blocks[0].operations.clear();
    machine.structural_parameters[0].multiplicity = StructuralMultiplicity::Affine;
    machine.entry_claims.clear();
    machine.parameters = vec![ValueDeclaration {
        id: value_id(10),
        scalar_type: ScalarType::Boolean,
    }];
    machine.result = TerminalMachineResult::Scalar(ValueDeclaration {
        id: value_id(11),
        scalar_type: ScalarType::Boolean,
    });
    machine.blocks[0].terminator = Terminator::Return {
        edge: edge_id(2),
        value: value_id(10),
        cleanup_actions: vec![TerminalAffineCleanupAction::DiscardRoot(place_id(2))],
    };
    module.entry = machine.id;
    module.machines = vec![machine];
    module.root_service_reach = Default::default();
    let semantic = encode_module(&module).expect("scalar affine cleanup module encodes");
    let proof = encode_proof_bundle(&ProofBundle::default()).expect("empty proof encodes");
    let mut execution = TerminalExecution::start_artifact_with_structural_arguments(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[TerminalScalarValue::Boolean(true)],
        &[structural_value(49)],
    )
    .expect("verified scalar affine cleanup should start");
    let mut meter = TerminalFuelMeter::with_allowance(0);

    assert!(matches!(
        execution.resume(&mut meter).unwrap(),
        TerminalExecutionStatus::SponsorExhausted(_)
    ));
    meter.replenish(1).unwrap();
    assert_eq!(
        execution.resume(&mut meter).unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Scalar(
            TerminalScalarValue::Boolean(true)
        ))
    );
}

#[test]
fn jump_performs_affine_discard_only_after_edge_charge() {
    let mut module = effect_module();
    let mut machine = module.machines.pop().expect("callee machine");
    machine.structural_parameters[0].multiplicity = StructuralMultiplicity::Affine;
    machine.entry_claims.clear();
    machine.parameters = vec![ValueDeclaration {
        id: value_id(10),
        scalar_type: ScalarType::Boolean,
    }];
    machine.result = TerminalMachineResult::Scalar(ValueDeclaration {
        id: value_id(11),
        scalar_type: ScalarType::Boolean,
    });
    machine.blocks = vec![
        Block {
            id: block_id(2),
            parameters: Vec::new(),
            operations: Vec::new(),
            terminator: Terminator::Jump {
                edge: edge_id(2),
                target: block_id(3),
                arguments: vec![value_id(10)],
                trivial_affine_discards: vec![place_id(2)],
            },
        },
        Block {
            id: block_id(3),
            parameters: vec![ValueDeclaration {
                id: value_id(12),
                scalar_type: ScalarType::Boolean,
            }],
            operations: Vec::new(),
            terminator: Terminator::Return {
                edge: edge_id(3),
                value: value_id(12),
                cleanup_actions: Vec::new(),
            },
        },
    ];
    module.entry = machine.id;
    module.machines = vec![machine];
    module.root_service_reach = Default::default();
    let semantic = encode_module(&module).expect("jump affine cleanup module encodes");
    let proof = encode_proof_bundle(&ProofBundle::default()).expect("empty proof encodes");
    let mut execution = TerminalExecution::start_artifact_with_structural_arguments(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[TerminalScalarValue::Boolean(true)],
        &[structural_value(50)],
    )
    .expect("verified jump affine cleanup should start");
    let mut meter = TerminalFuelMeter::with_allowance(0);

    assert!(matches!(
        execution.resume(&mut meter).unwrap(),
        TerminalExecutionStatus::SponsorExhausted(_)
    ));
    meter.replenish(1).unwrap();
    assert!(matches!(
        execution.resume(&mut meter).unwrap(),
        TerminalExecutionStatus::SponsorExhausted(_)
    ));
    meter.replenish(1).unwrap();
    assert_eq!(
        execution.resume(&mut meter).unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Scalar(
            TerminalScalarValue::Boolean(true)
        ))
    );
}

#[test]
fn conditional_commits_only_the_selected_affine_cleanup_after_edge_charge() {
    let mut module = effect_module();
    let mut machine = module.machines.pop().expect("callee machine");
    machine.structural_parameters[0].multiplicity = StructuralMultiplicity::Affine;
    machine.entry_claims.clear();
    machine.parameters = vec![ValueDeclaration {
        id: value_id(10),
        scalar_type: ScalarType::Boolean,
    }];
    machine.result = TerminalMachineResult::Scalar(ValueDeclaration {
        id: value_id(11),
        scalar_type: ScalarType::Boolean,
    });
    machine.blocks = vec![
        Block {
            id: block_id(2),
            parameters: Vec::new(),
            operations: Vec::new(),
            terminator: Terminator::Conditional {
                condition: value_id(10),
                when_true: SuccessorEdge {
                    edge: edge_id(2),
                    target: block_id(3),
                    arguments: vec![value_id(10)],
                    trivial_affine_discards: vec![place_id(2)],
                },
                when_false: SuccessorEdge {
                    edge: edge_id(3),
                    target: block_id(4),
                    arguments: vec![value_id(10)],
                    trivial_affine_discards: Vec::new(),
                },
            },
        },
        Block {
            id: block_id(3),
            parameters: vec![ValueDeclaration {
                id: value_id(12),
                scalar_type: ScalarType::Boolean,
            }],
            operations: Vec::new(),
            terminator: Terminator::Return {
                edge: edge_id(4),
                value: value_id(12),
                cleanup_actions: Vec::new(),
            },
        },
        Block {
            id: block_id(4),
            parameters: vec![ValueDeclaration {
                id: value_id(13),
                scalar_type: ScalarType::Boolean,
            }],
            operations: Vec::new(),
            terminator: Terminator::Return {
                edge: edge_id(5),
                value: value_id(13),
                cleanup_actions: vec![TerminalAffineCleanupAction::DiscardRoot(place_id(2))],
            },
        },
    ];
    module.entry = machine.id;
    module.machines = vec![machine];
    module.root_service_reach = Default::default();
    let semantic = encode_module(&module).expect("conditional affine cleanup module encodes");
    let proof = encode_proof_bundle(&ProofBundle::default()).expect("empty proof encodes");

    for condition in [true, false] {
        let mut execution = TerminalExecution::start_artifact_with_structural_arguments(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            &[TerminalScalarValue::Boolean(condition)],
            &[structural_value(51)],
        )
        .expect("verified conditional affine cleanup should start");
        let mut meter = TerminalFuelMeter::with_allowance(0);
        assert!(matches!(
            execution.resume(&mut meter).unwrap(),
            TerminalExecutionStatus::SponsorExhausted(_)
        ));
        meter.replenish(1).unwrap();
        assert!(matches!(
            execution.resume(&mut meter).unwrap(),
            TerminalExecutionStatus::SponsorExhausted(_)
        ));
        meter.replenish(1).unwrap();
        assert_eq!(
            execution.resume(&mut meter).unwrap(),
            TerminalExecutionStatus::Complete(TerminalExecutionResult::Scalar(
                TerminalScalarValue::Boolean(condition)
            ))
        );
    }
}

#[test]
fn unit_calls_transfer_claims_and_effects_observe_exact_structural_arguments() {
    let (semantic, proof) = effect_artifact_sections();
    let argument = structural_value(41);
    let mut handler = RecordingHandler::default();
    let measured = interpret_terminal_artifact_with_effect_handler_measured(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[],
        &[argument.clone()],
        &mut handler,
    )
    .expect("verified Unit/effect artifact should execute");

    let expected = vec![
        TerminalEffect::BoundaryCall {
            operation: operation_id(3),
            result: None,
            boundary: boundary_id(1),
            arguments: Vec::new(),
            structural_arguments: vec![argument],
            byte_sequence_arguments: vec![None],
            completion_receipts: vec![CompletionReceipt {
                claim: claim_id(1),
                argument_index: 0,
            }],
        },
        TerminalEffect::PortWrite {
            operation: operation_id(2),
            service: service_id(1),
            port: 0x20,
            value: 0x20,
        },
    ];
    assert_eq!(handler.effects, expected);
    assert_eq!(measured.effects(), expected);
    assert_eq!(measured.value(), TerminalExecutionResult::Unit);
    assert_eq!(measured.usage().total_units(), 5);
}

#[test]
fn byte_sequence_literal_round_trips_non_utf8_and_reaches_boundary_exactly() {
    let module = byte_sequence_literal_module(vec![0x00, 0x7f, 0x80, 0xff]);
    let semantic = encode_module(&module).expect("byte literal semantics encode");
    assert_eq!(decode_module(&semantic), Ok(module));
    let proof = encode_proof_bundle(&ProofBundle::default()).expect("empty proof encodes");
    let mut handler = RecordingHandler::default();

    let measured = interpret_terminal_artifact_with_effect_handler_measured(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[],
        &[],
        &mut handler,
    )
    .expect("verified byte literal reaches the semantic boundary");

    assert_eq!(measured.value(), TerminalExecutionResult::Unit);
    assert!(matches!(
        handler.effects.as_slice(),
        [TerminalEffect::BoundaryCall {
            structural_arguments,
            byte_sequence_arguments,
            ..
        }] if structural_arguments.len() == 1
            && byte_sequence_arguments == &[Some(vec![0x00, 0x7f, 0x80, 0xff])]
    ));
}

#[test]
fn byte_sequence_literal_tampering_fails_closed() {
    let mut wrong_type = byte_sequence_literal_module(vec![0xff]);
    wrong_type.structural_types[0].shape = StructuralTypeShape::Record { fields: Vec::new() };
    assert!(encode_module(&wrong_type).is_err());

    let mut wrong_source = byte_sequence_literal_module(vec![0xff]);
    let OperationKind::BoundaryCall {
        structural_arguments,
        ..
    } = &mut wrong_source.machines[0].blocks[0].operations[1].kind
    else {
        panic!("fixture boundary call");
    };
    structural_arguments[0].place = place_id(99);
    assert!(encode_module(&wrong_source).is_err());

    let mut reordered = byte_sequence_literal_module(vec![0xff]);
    reordered.machines[0].blocks[0].operations.swap(0, 1);
    assert!(encode_module(&reordered).is_err());
}

#[test]
fn boundary_scalar_arguments_reach_effect_handlers_in_declared_order() {
    let module = scalar_boundary_effect_module();
    let semantic = encode_module(&module).expect("scalar boundary module encodes");
    let proof = encode_proof_bundle(&ProofBundle::default()).expect("empty proof encodes");
    let mut handler = RecordingHandler::default();
    let measured = interpret_terminal_artifact_with_effect_handler_measured(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[],
        &[],
        &mut handler,
    )
    .expect("verified scalar boundary call executes");

    let expected = TerminalEffect::BoundaryCall {
        operation: operation_id(3),
        boundary: boundary_id(1),
        arguments: vec![
            TerminalScalarValue::Boolean(true),
            TerminalScalarValue::Boolean(false),
        ],
        structural_arguments: Vec::new(),
        byte_sequence_arguments: Vec::new(),
        completion_receipts: Vec::new(),
        result: None,
    };
    assert_eq!(handler.effects, [expected.clone()]);
    assert_eq!(measured.effects(), &[expected]);
    assert_eq!(measured.value(), TerminalExecutionResult::Unit);
}

#[test]
fn boundary_scalar_argument_effect_rejection_is_fail_closed() {
    let module = scalar_boundary_effect_module();
    let semantic = encode_module(&module).expect("scalar boundary module encodes");
    let proof = encode_proof_bundle(&ProofBundle::default()).expect("empty proof encodes");
    let mut execution =
        TerminalExecution::start_artifact(&semantic, &proof, &AdmissionProfile::default(), &[])
            .expect("verified scalar boundary call starts");
    let mut meter = TerminalFuelMeter::unbounded();
    let mut handler = RejectScalarBoundaryArguments;

    assert!(matches!(
        execution.resume_with_effect_handler(&mut meter, &mut handler),
        Err(TerminalInterpretError::EffectRejected { operation, .. })
            if operation == operation_id(3)
    ));
    assert!(execution.effects().is_empty());
}

#[test]
fn unit_calls_transfer_numbered_record_field_claims() {
    let mut module = effect_module();
    module.structural_types.push(StructuralTypeDeclaration {
        id: structural_type_id(2),
        identity: "test::Token".into(),
        shape: StructuralTypeShape::Record { fields: Vec::new() },
    });
    let StructuralTypeShape::Record { fields } = &mut module.structural_types[0].shape else {
        panic!("expected record shape")
    };
    fields.push(StructuralFieldDeclaration {
        id: psi_core::StructuralFieldId::new(1).expect("field identity"),
        identity: "#7".into(),
        relevance: BindingRelevance::Relevant,
        field_type: StructuralFieldType::Structural(structural_type_id(2)),
    });
    for machine in &mut module.machines {
        machine.entry_claims[0].path = vec!["#7".into()];
    }
    let semantic = encode_module(&module).expect("numbered field-custody module encodes");
    let proof = encode_proof_bundle(&ProofBundle::default()).expect("empty proof encodes");
    let mut execution = TerminalExecution::start_artifact_with_structural_arguments(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[],
        &[structural_value(45)],
    )
    .expect("verified numbered field custody should start");
    let mut meter = TerminalFuelMeter::unbounded();
    assert_eq!(
        execution.resume(&mut meter).unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
}

#[test]
fn unit_calls_transfer_and_settle_nested_record_field_claims() {
    let mut module = effect_module();
    module.structural_types.extend([
        StructuralTypeDeclaration {
            id: structural_type_id(2),
            identity: "test::Pocket".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![StructuralFieldDeclaration {
                    id: psi_core::StructuralFieldId::new(1).expect("field identity"),
                    identity: "#9".into(),
                    relevance: BindingRelevance::Relevant,
                    field_type: StructuralFieldType::Structural(structural_type_id(3)),
                }],
            },
        },
        StructuralTypeDeclaration {
            id: structural_type_id(3),
            identity: "test::Token".into(),
            shape: StructuralTypeShape::Record { fields: Vec::new() },
        },
    ]);
    let StructuralTypeShape::Record { fields } = &mut module.structural_types[0].shape else {
        panic!("expected record shape")
    };
    fields.push(StructuralFieldDeclaration {
        id: psi_core::StructuralFieldId::new(1).expect("field identity"),
        identity: "#7".into(),
        relevance: BindingRelevance::Relevant,
        field_type: StructuralFieldType::Structural(structural_type_id(2)),
    });
    module.boundary_machines[0].structural_parameters[0].multiplicity =
        StructuralMultiplicity::Affine;
    for machine in &mut module.machines {
        machine.structural_parameters[0].multiplicity = StructuralMultiplicity::Affine;
        machine.entry_claims[0].path = vec!["#7".into(), "#9".into()];
    }

    let semantic = encode_module(&module).expect("nested field-custody module encodes");
    let proof = encode_proof_bundle(&ProofBundle::default()).expect("empty proof encodes");
    let argument = structural_value(47);
    let mut handler = RecordingHandler::default();
    let measured = interpret_terminal_artifact_with_effect_handler_measured(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[],
        &[argument.clone()],
        &mut handler,
    )
    .expect("verified nested field custody should execute");
    assert_eq!(measured.value(), TerminalExecutionResult::Unit);
    assert!(matches!(
        &handler.effects[0],
        TerminalEffect::BoundaryCall {
            structural_arguments,
            completion_receipts,
            ..
        } if structural_arguments == &[argument]
            && completion_receipts == &[
                CompletionReceipt { claim: claim_id(1), argument_index: 0 },
            ]
    ));
}

#[test]
fn unit_calls_transfer_and_settle_both_sibling_field_claims() {
    let mut module = effect_module();
    module.structural_types.push(StructuralTypeDeclaration {
        id: structural_type_id(2),
        identity: "test::Token".into(),
        shape: StructuralTypeShape::Record { fields: Vec::new() },
    });
    let StructuralTypeShape::Record { fields } = &mut module.structural_types[0].shape else {
        panic!("expected record shape")
    };
    fields.extend([
        StructuralFieldDeclaration {
            id: psi_core::StructuralFieldId::new(1).expect("field identity"),
            identity: "#7".into(),
            relevance: BindingRelevance::Relevant,
            field_type: StructuralFieldType::Structural(structural_type_id(2)),
        },
        StructuralFieldDeclaration {
            id: psi_core::StructuralFieldId::new(2).expect("field identity"),
            identity: "#9".into(),
            relevance: BindingRelevance::Relevant,
            field_type: StructuralFieldType::Structural(structural_type_id(2)),
        },
    ]);
    module.boundary_machines[0].structural_parameters[0].multiplicity =
        StructuralMultiplicity::Affine;
    for machine in &mut module.machines {
        machine.structural_parameters[0].multiplicity = StructuralMultiplicity::Affine;
        machine.entry_claims[0].path = vec!["#7".into()];
        machine.entry_claims.push(EntryClaim {
            claim: claim_id(2),
            input: machine.structural_parameters[0].place,
            path: vec!["#9".into()],
        });
    }
    let OperationKind::CallUnit {
        claim_transfers, ..
    } = &mut module.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    claim_transfers.push(ClaimTransfer {
        claim: claim_id(2),
        argument_index: 0,
    });
    let OperationKind::BoundaryCall {
        completion_receipts,
        ..
    } = &mut module.machines[1].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    completion_receipts.push(CompletionReceipt {
        claim: claim_id(2),
        argument_index: 0,
    });

    let semantic = encode_module(&module).expect("sibling field-custody module encodes");
    let proof = encode_proof_bundle(&ProofBundle::default()).expect("empty proof encodes");
    let argument = structural_value(46);
    let mut handler = RecordingHandler::default();
    let measured = interpret_terminal_artifact_with_effect_handler_measured(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[],
        &[argument.clone()],
        &mut handler,
    )
    .expect("verified sibling field custody should execute");
    assert_eq!(measured.value(), TerminalExecutionResult::Unit);
    assert!(matches!(
        &handler.effects[0],
        TerminalEffect::BoundaryCall {
            structural_arguments,
            completion_receipts,
            ..
        } if structural_arguments == &[argument]
            && completion_receipts == &[
                CompletionReceipt { claim: claim_id(1), argument_index: 0 },
                CompletionReceipt { claim: claim_id(2), argument_index: 0 },
            ]
    ));
}

#[test]
fn sponsor_exhaustion_does_not_replay_unit_calls_or_accepted_effects() {
    let (semantic, proof) = effect_artifact_sections();
    let mut execution = TerminalExecution::start_artifact_with_structural_arguments(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[],
        &[structural_value(42)],
    )
    .unwrap();
    let mut meter = TerminalFuelMeter::with_allowance(3);
    let mut handler = RecordingHandler::default();

    assert_eq!(
        execution
            .resume_with_effect_handler(&mut meter, &mut handler)
            .unwrap(),
        TerminalExecutionStatus::SponsorExhausted(FuelExhaustion {
            schedule: TerminalFuelSchedule::CURRENT.identity(),
            site: FuelChargeSite::Operation(operation_id(2)),
            required_units: 1,
            remaining_units: 0,
        })
    );
    assert_eq!(handler.effects.len(), 1);
    assert!(matches!(
        handler.effects[0],
        TerminalEffect::BoundaryCall { .. }
    ));

    meter.replenish(2).unwrap();
    assert_eq!(
        execution
            .resume_with_effect_handler(&mut meter, &mut handler)
            .unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
    assert_eq!(handler.effects.len(), 2);
    assert_eq!(execution.effects(), handler.effects);
    assert_eq!(meter.usage().total_units(), 5);
}

#[test]
fn structural_runtime_mismatches_and_effect_rejection_fail_closed() {
    let (semantic, proof) = effect_artifact_sections();
    let missing_qualification = TerminalStructuralValue {
        opaque_identity: 43,
        structural_type: structural_type_id(1),
        qualifications: Vec::new(),
        path: Vec::new(),
    };
    assert!(matches!(
        TerminalExecution::start_artifact_with_structural_arguments(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            &[],
            &[missing_qualification],
        ),
        Err(
            psi_terminal_interpreter::TerminalArtifactInterpretError::Execution(
                TerminalInterpretError::StructuralQualificationMissing(_)
            )
        )
    ));

    let mut rejecting = RejectingHandler;
    assert!(matches!(
        interpret_terminal_artifact_with_effect_handler_measured(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            &[],
            &[structural_value(44)],
            &mut rejecting,
        ),
        Err(psi_terminal_interpreter::TerminalArtifactInterpretError::Execution(
            TerminalInterpretError::EffectRejected { operation, .. }
        )) if operation == operation_id(3)
    ));

    let mut execution = TerminalExecution::start_artifact_with_structural_arguments(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[],
        &[structural_value(44)],
    )
    .expect("verified effect artifact should start");
    let mut meter = TerminalFuelMeter::unbounded();
    assert!(matches!(
        execution.resume_with_effect_handler(&mut meter, &mut rejecting),
        Err(TerminalInterpretError::EffectRejected { operation, .. })
            if operation == operation_id(3)
    ));
    assert!(execution.effects().is_empty());
    assert_eq!(
        execution.live_claim_frontier().collect::<Vec<_>>(),
        [claim_id(1)]
    );
}

#[derive(Default)]
struct RecordingHandler {
    effects: Vec<TerminalEffect>,
}

impl TerminalEffectHandler for RecordingHandler {
    fn handle_effect(&mut self, effect: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
        self.effects.push(effect.clone());
        Ok(())
    }
}

struct RejectingHandler;

impl TerminalEffectHandler for RejectingHandler {
    fn handle_effect(&mut self, _effect: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
        Err(TerminalEffectRejection::new("mock rejection"))
    }
}

struct RejectScalarBoundaryArguments;

impl TerminalEffectHandler for RejectScalarBoundaryArguments {
    fn handle_effect(&mut self, effect: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
        match effect {
            TerminalEffect::BoundaryCall { arguments, .. }
                if arguments
                    == &[
                        TerminalScalarValue::Boolean(true),
                        TerminalScalarValue::Boolean(false),
                    ] =>
            {
                Err(TerminalEffectRejection::new(
                    "mock policy rejects the scalar boundary argument",
                ))
            }
            _ => Err(TerminalEffectRejection::new(
                "scalar boundary arguments were not resolved in declaration order",
            )),
        }
    }
}

fn byte_sequence_literal_module(bytes: Vec<u8>) -> TerminalModule {
    let structural_type = structural_type_id(1);
    let literal = place_id(1);
    TerminalModule {
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: machine_id(1),
        structural_types: vec![StructuralTypeDeclaration {
            id: structural_type,
            identity: "test::BorrowedBytes".into(),
            shape: StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView),
        }],
        structural_domains: Vec::new(),
        services: Vec::new(),
        root_service_reach: Default::default(),
        boundary_machines: vec![BoundaryMachineDeclaration {
            id: boundary_id(1),
            identity: "test::write_line".into(),
            attachment: None,
            scalar_parameters: Vec::new(),
            structural_parameters: vec![StructuralParameterDeclaration {
                place: place_id(2),
                position: 0,
                is_self: false,
                structural_type,
                multiplicity: StructuralMultiplicity::Unrestricted,
                access: StructuralAccess::SharedBorrow,
                qualifications: Vec::new(),
            }],
            result: None,
            requires: Vec::new(),
            program_local_root_introductions: Vec::new(),
            content_guarantees: Vec::new(),
            published_service_ceiling: Vec::new(),
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
        machines: vec![TerminalMachine {
            id: machine_id(1),
            attachment: None,
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            result: TerminalMachineResult::Unit,
            structural_places: vec![StructuralPlaceDeclaration {
                id: literal,
                kind: psi_core::StructuralPlaceKind::ByteSequenceLiteral {
                    declaration_ordinal: 0,
                    structural_type,
                },
            }],
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
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
                        result: OperationResult::Unit,
                        kind: OperationKind::EstablishByteSequenceLiteral {
                            destination: literal,
                            bytes,
                        },
                    },
                    Operation {
                        id: operation_id(2),
                        result: OperationResult::Unit,
                        kind: OperationKind::BoundaryCall {
                            boundary: boundary_id(1),
                            arguments: Vec::new(),
                            structural_arguments: vec![StructuralArgument {
                                place: literal,
                                access: StructuralAccess::SharedBorrow,
                                path: Vec::new(),
                            }],
                            completion_receipts: Vec::new(),
                            requirement_obligations: Vec::new(),
                        },
                    },
                ],
                terminator: Terminator::ReturnUnit {
                    edge: edge_id(1),
                    trivial_affine_discards: Vec::new(),
                },
            }],
            contract: empty_contract(contract_id(1)),
        }],
    }
}

fn effect_artifact_sections() -> (Vec<u8>, Vec<u8>) {
    (
        encode_module(&effect_module()).expect("effect semantics encode"),
        encode_proof_bundle(&ProofBundle::default()).expect("empty proof encodes"),
    )
}

fn scalar_boundary_effect_module() -> TerminalModule {
    TerminalModule {
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: machine_id(1),
        structural_types: Vec::new(),
        structural_domains: Vec::new(),
        services: Vec::new(),
        root_service_reach: Default::default(),
        boundary_machines: vec![BoundaryMachineDeclaration {
            id: boundary_id(1),
            identity: "test::observe".into(),
            attachment: None,
            scalar_parameters: vec![ScalarType::Boolean, ScalarType::Boolean],
            structural_parameters: Vec::new(),
            result: None,
            requires: Vec::new(),
            program_local_root_introductions: Vec::new(),
            content_guarantees: Vec::new(),
            published_service_ceiling: Vec::new(),
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
        machines: vec![TerminalMachine {
            id: machine_id(1),
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
            entry: block_id(1),
            blocks: vec![Block {
                id: block_id(1),
                parameters: Vec::new(),
                operations: vec![
                    Operation {
                        id: operation_id(1),
                        result: OperationResult::Scalar(ValueDeclaration {
                            id: value_id(1),
                            scalar_type: ScalarType::Boolean,
                        }),
                        kind: OperationKind::BooleanConstant { value: true },
                    },
                    Operation {
                        id: operation_id(2),
                        result: OperationResult::Scalar(ValueDeclaration {
                            id: value_id(2),
                            scalar_type: ScalarType::Boolean,
                        }),
                        kind: OperationKind::BooleanConstant { value: false },
                    },
                    Operation {
                        id: operation_id(3),
                        result: OperationResult::Unit,
                        kind: OperationKind::BoundaryCall {
                            boundary: boundary_id(1),
                            arguments: vec![value_id(1), value_id(2)],
                            structural_arguments: Vec::new(),
                            completion_receipts: Vec::new(),
                            requirement_obligations: Vec::new(),
                        },
                    },
                ],
                terminator: Terminator::ReturnUnit {
                    edge: edge_id(1),
                    trivial_affine_discards: Vec::new(),
                },
            }],
            contract: empty_contract(contract_id(1)),
        }],
    }
}

fn effect_module() -> TerminalModule {
    let structural_type = structural_type_id(1);
    let domain = structural_domain_id(1);
    let service = service_id(1);
    TerminalModule {
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: machine_id(1),
        structural_types: vec![StructuralTypeDeclaration {
            id: structural_type,
            identity: "test::Device".into(),
            shape: StructuralTypeShape::Record { fields: Vec::new() },
        }],
        structural_domains: vec![StructuralDomainDeclaration {
            id: domain,
            semantic_domain: psi_core::DomainSemanticId::new(1).expect("semantic domain identity"),
            identity: "test::Ready".into(),
            carrier: structural_type,
            content_projection: None,
        }],
        services: vec![ServiceDeclaration {
            id: service,
            identity: "test::PortIo".into(),
            parents: Vec::new(),
        }],
        root_service_reach: psi_terminal::TerminalRootServiceReach {
            concrete: vec![service],
            installation_dependencies: Vec::new(),
        },
        boundary_machines: vec![BoundaryMachineDeclaration {
            id: boundary_id(1),
            identity: "test::acknowledge".into(),
            attachment: Some(structural_type),
            scalar_parameters: Vec::new(),
            structural_parameters: vec![structural_parameter(place_id(3), structural_type, domain)],
            result: None,
            requires: vec![StructuralDomainRequirement {
                argument_index: 0,
                domain,
            }],
            program_local_root_introductions: Vec::new(),
            content_guarantees: Vec::new(),
            published_service_ceiling: Vec::new(),
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
        machines: vec![
            TerminalMachine {
                id: machine_id(1),
                attachment: Some(structural_type),
                parameters: Vec::new(),
                structural_parameters: vec![structural_parameter(
                    place_id(1),
                    structural_type,
                    domain,
                )],
                result: TerminalMachineResult::Unit,
                structural_places: vec![structural_place(place_id(1))],
                entry_claims: vec![EntryClaim {
                    claim: claim_id(1),
                    input: place_id(1),
                    path: Vec::new(),
                }],
                published_service_ceiling: vec![service],
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
                            result: OperationResult::Unit,
                            kind: OperationKind::CallUnit {
                                callee: machine_id(2),
                                structural_arguments: vec![StructuralArgument {
                                    place: place_id(1),
                                    access: StructuralAccess::Owned,
                                    path: Vec::new(),
                                }],
                                claim_transfers: vec![ClaimTransfer {
                                    claim: claim_id(1),
                                    argument_index: 0,
                                }],
                                requirement_obligations: Vec::new(),
                                crash_continuations: Vec::new(),
                            },
                        },
                        Operation {
                            id: operation_id(2),
                            result: OperationResult::Unit,
                            kind: OperationKind::PortWrite {
                                service,
                                port: 0x20,
                                value: 0x20,
                            },
                        },
                    ],
                    terminator: Terminator::ReturnUnit {
                        edge: edge_id(1),
                        trivial_affine_discards: Vec::new(),
                    },
                }],
                contract: empty_contract(contract_id(1)),
            },
            TerminalMachine {
                id: machine_id(2),
                attachment: Some(structural_type),
                parameters: Vec::new(),
                structural_parameters: vec![structural_parameter(
                    place_id(2),
                    structural_type,
                    domain,
                )],
                result: TerminalMachineResult::Unit,
                structural_places: vec![structural_place(place_id(2))],
                entry_claims: vec![EntryClaim {
                    claim: claim_id(1),
                    input: place_id(2),
                    path: Vec::new(),
                }],
                published_service_ceiling: Vec::new(),
                content_entry_claims: Vec::new(),
                content_identity_reshuffles: Vec::new(),
                content_partition_compositions: Vec::new(),
                entry: block_id(2),
                blocks: vec![Block {
                    id: block_id(2),
                    parameters: Vec::new(),
                    operations: vec![Operation {
                        id: operation_id(3),
                        result: OperationResult::Unit,
                        kind: OperationKind::BoundaryCall {
                            boundary: boundary_id(1),
                            arguments: Vec::new(),
                            structural_arguments: vec![StructuralArgument {
                                place: place_id(2),
                                access: StructuralAccess::Owned,
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
                        edge: edge_id(2),
                        trivial_affine_discards: Vec::new(),
                    },
                }],
                contract: empty_contract(contract_id(2)),
            },
        ],
    }
}

fn structural_parameter(
    place: PlaceId,
    structural_type: StructuralTypeId,
    domain: StructuralDomainId,
) -> StructuralParameterDeclaration {
    StructuralParameterDeclaration {
        place,
        position: 0,
        is_self: true,
        structural_type,
        multiplicity: StructuralMultiplicity::Linear,
        access: StructuralAccess::Owned,
        qualifications: vec![domain],
    }
}

fn structural_place(id: PlaceId) -> StructuralPlaceDeclaration {
    StructuralPlaceDeclaration {
        id,
        kind: psi_core::StructuralPlaceKind::Parameter {
            position: 0,
            is_self: true,
        },
    }
}

fn structural_value(opaque_identity: u64) -> TerminalStructuralValue {
    TerminalStructuralValue {
        opaque_identity,
        structural_type: structural_type_id(1),
        qualifications: vec![structural_domain_id(1)],
        path: Vec::new(),
    }
}

fn empty_contract(id: ContractId) -> MachineContract {
    MachineContract {
        id,
        crash_routes: Vec::new(),
        requires: Vec::new(),
        ensures: Vec::new(),
        outcome_specific_ensures: Vec::new(),
    }
}

fn artifact_sections() -> (Vec<u8>, Vec<u8>) {
    (
        encode_module(&unit_module()).expect("unit semantics encode"),
        encode_proof_bundle(&ProofBundle::default()).expect("empty proof encodes"),
    )
}

fn unit_module() -> TerminalModule {
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
        proof_output_calls: Vec::new(),
        closed_conformance_applications: Vec::new(),
        machines: vec![TerminalMachine {
            id: machine_id(1),
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
            entry: block_id(1),
            blocks: vec![Block {
                id: block_id(1),
                parameters: Vec::new(),
                operations: Vec::new(),
                terminator: Terminator::ReturnUnit {
                    edge: edge_id(1),
                    trivial_affine_discards: Vec::new(),
                },
            }],
            contract: MachineContract {
                id: contract_id(1),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        }],
    }
}

fn nominal_affine_module() -> TerminalModule {
    let token = StructuralTypeDeclaration {
        id: structural_type_id(1),
        identity: "Token".into(),
        shape: StructuralTypeShape::Record { fields: Vec::new() },
    };
    TerminalModule {
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: machine_id(1),
        structural_types: vec![token.clone()],
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
        proof_output_calls: Vec::new(),
        closed_conformance_applications: Vec::new(),
        machines: vec![
            TerminalMachine {
                id: machine_id(1),
                attachment: None,
                parameters: Vec::new(),
                structural_parameters: vec![StructuralParameterDeclaration {
                    place: place_id(1),
                    position: 0,
                    is_self: false,
                    structural_type: token.id,
                    multiplicity: StructuralMultiplicity::Affine,
                    access: StructuralAccess::Owned,
                    qualifications: Vec::new(),
                }],
                result: TerminalMachineResult::Unit,
                structural_places: vec![StructuralPlaceDeclaration {
                    id: place_id(1),
                    kind: psi_core::StructuralPlaceKind::Parameter {
                        position: 0,
                        is_self: false,
                    },
                }],
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                content_entry_claims: Vec::new(),
                content_identity_reshuffles: Vec::new(),
                content_partition_compositions: Vec::new(),
                entry: block_id(1),
                blocks: vec![Block {
                    id: block_id(1),
                    parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: Terminator::ReturnUnitNominalAffine {
                        edge: edge_id(1),
                        cleanups: vec![NominalAffineCleanup {
                            place: place_id(1),
                            structural_type: token.id,
                            cleanup_machine: machine_id(2),
                            cleanup_receiver: None,
                            requirement_obligations: Vec::new(),
                        }],
                    },
                }],
                contract: empty_contract(contract_id(1)),
            },
            TerminalMachine {
                id: machine_id(2),
                attachment: Some(token.id),
                parameters: Vec::new(),
                structural_parameters: Vec::new(),
                result: TerminalMachineResult::Unit,
                structural_places: Vec::new(),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                content_entry_claims: Vec::new(),
                content_identity_reshuffles: Vec::new(),
                content_partition_compositions: Vec::new(),
                entry: block_id(2),
                blocks: vec![Block {
                    id: block_id(2),
                    parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: Terminator::ReturnUnit {
                        edge: edge_id(2),
                        trivial_affine_discards: Vec::new(),
                    },
                }],
                contract: empty_contract(contract_id(2)),
            },
        ],
    }
}

#[test]
fn scalar_return_materializes_result_then_runs_nominal_cleanup() {
    let mut module = nominal_affine_module();
    let caller = &mut module.machines[0];
    caller.parameters = vec![ValueDeclaration {
        id: value_id(10),
        scalar_type: ScalarType::Boolean,
    }];
    caller.result = TerminalMachineResult::Scalar(ValueDeclaration {
        id: value_id(11),
        scalar_type: ScalarType::Boolean,
    });
    let Terminator::ReturnUnitNominalAffine { edge, cleanups } = std::mem::replace(
        &mut caller.blocks[0].terminator,
        Terminator::ReturnUnit {
            edge: edge_id(99),
            trivial_affine_discards: Vec::new(),
        },
    ) else {
        unreachable!()
    };
    caller.blocks[0].terminator = Terminator::Return {
        edge,
        value: value_id(10),
        cleanup_actions: cleanups
            .into_iter()
            .map(TerminalAffineCleanupAction::InvokeNominal)
            .collect(),
    };

    let semantic = encode_module(&module).expect("scalar nominal cleanup encodes");
    let proof = encode_proof_bundle(&ProofBundle::default()).expect("empty proof encodes");
    let mut execution = TerminalExecution::start_artifact_with_structural_arguments(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[TerminalScalarValue::Boolean(true)],
        &[TerminalStructuralValue {
            opaque_identity: 71,
            structural_type: structural_type_id(1),
            qualifications: Vec::new(),
            path: Vec::new(),
        }],
    )
    .expect("verified scalar nominal cleanup starts");
    let mut meter = TerminalFuelMeter::with_allowance(1);

    assert!(matches!(
        execution.resume(&mut meter).unwrap(),
        TerminalExecutionStatus::SponsorExhausted(FuelExhaustion {
            site: FuelChargeSite::Edge(edge),
            ..
        }) if edge == edge_id(2)
    ));
    assert!(execution.live_affine_frontier().next().is_none());
    meter.replenish(1).unwrap();
    assert_eq!(
        execution.resume(&mut meter).unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Scalar(
            TerminalScalarValue::Boolean(true)
        ))
    );
}

#[test]
fn contextual_scalar_return_materializes_then_executes_reverse_ordered_cleanups() {
    let mut module = ordered_empty_nominal_affine_module(true);
    let first = psi_core::StructuralFieldId::new(1).expect("first field");
    let second = psi_core::StructuralFieldId::new(2).expect("second field");
    module.structural_types[0].shape = StructuralTypeShape::Record {
        fields: [first, second]
            .into_iter()
            .map(|id| StructuralFieldDeclaration {
                id,
                identity: format!("flag_{}", id.get()),
                relevance: BindingRelevance::Relevant,
                field_type: StructuralFieldType::Scalar(ScalarType::Boolean),
            })
            .collect(),
    };
    let receiver = place_id(99);
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
        id: value_id(30),
        scalar_type: ScalarType::Boolean,
    }];
    caller.result = TerminalMachineResult::Scalar(ValueDeclaration {
        id: value_id(31),
        scalar_type: ScalarType::Boolean,
    });
    caller.contract.requires = [place_id(1), place_id(2)]
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
            edge: edge_id(99),
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
        value: value_id(30),
        cleanup_actions: cleanups
            .into_iter()
            .map(TerminalAffineCleanupAction::InvokeNominal)
            .collect(),
    };
    let goals = [
        (obligation_id(3), place_id(2), first),
        (obligation_id(4), place_id(2), second),
        (obligation_id(1), place_id(1), first),
        (obligation_id(2), place_id(1), second),
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
    let proof_bundle = ProofBundle {
        evidence_producers: Vec::new(),
        evidence,
    };
    let semantic = encode_module(&module).expect("contextual scalar cleanup encodes");
    let proof = encode_proof_bundle(&proof_bundle).expect("contextual cleanup proof encodes");
    let structural_arguments = [
        TerminalStructuralValue {
            opaque_identity: 130,
            structural_type: structural_type_id(1),
            qualifications: Vec::new(),
            path: Vec::new(),
        },
        TerminalStructuralValue {
            opaque_identity: 131,
            structural_type: structural_type_id(1),
            qualifications: Vec::new(),
            path: Vec::new(),
        },
    ];
    let mut execution = TerminalExecution::start_artifact_with_structural_arguments(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[TerminalScalarValue::Boolean(false)],
        &structural_arguments,
    )
    .expect("proof-carrying contextual scalar cleanup starts");
    let mut meter = TerminalFuelMeter::with_allowance(0);

    for expected_site in [
        FuelChargeSite::Edge(edge_id(1)),
        FuelChargeSite::Edge(edge_id(2)),
        FuelChargeSite::Edge(edge_id(2)),
    ] {
        assert!(matches!(
            execution.resume(&mut meter).unwrap(),
            TerminalExecutionStatus::SponsorExhausted(FuelExhaustion { site, .. })
                if site == expected_site
        ));
        meter.replenish(1).unwrap();
    }
    assert_eq!(
        execution.resume(&mut meter).unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Scalar(
            TerminalScalarValue::Boolean(false)
        ))
    );
    assert_eq!(meter.usage().total_units(), 3);
    assert!(execution.live_affine_frontier().next().is_none());
}

#[test]
fn mixed_scalar_return_cleanup_resumes_nominal_work_around_a_no_code_discard() {
    let mut module = three_ordered_empty_nominal_affine_module(false);
    let caller = &mut module.machines[0];
    caller.parameters = vec![ValueDeclaration {
        id: value_id(20),
        scalar_type: ScalarType::Boolean,
    }];
    caller.result = TerminalMachineResult::Scalar(ValueDeclaration {
        id: value_id(21),
        scalar_type: ScalarType::Boolean,
    });
    let Terminator::ReturnUnitNominalAffine { edge, cleanups } = std::mem::replace(
        &mut caller.blocks[0].terminator,
        Terminator::ReturnUnit {
            edge: edge_id(99),
            trivial_affine_discards: Vec::new(),
        },
    ) else {
        unreachable!()
    };
    assert_eq!(cleanups.len(), 3);
    caller.blocks[0].terminator = Terminator::Return {
        edge,
        value: value_id(20),
        cleanup_actions: vec![
            TerminalAffineCleanupAction::InvokeNominal(cleanups[0].clone()),
            TerminalAffineCleanupAction::DiscardRoot(cleanups[1].place),
            TerminalAffineCleanupAction::InvokeNominal(cleanups[2].clone()),
        ],
    };

    let semantic = encode_module(&module).expect("mixed scalar cleanup stream encodes");
    let proof = encode_proof_bundle(&ProofBundle::default()).expect("empty proof encodes");
    let structural_arguments = [
        TerminalStructuralValue {
            opaque_identity: 120,
            structural_type: structural_type_id(1),
            qualifications: Vec::new(),
            path: Vec::new(),
        },
        TerminalStructuralValue {
            opaque_identity: 121,
            structural_type: structural_type_id(2),
            qualifications: Vec::new(),
            path: Vec::new(),
        },
        TerminalStructuralValue {
            opaque_identity: 122,
            structural_type: structural_type_id(3),
            qualifications: Vec::new(),
            path: Vec::new(),
        },
    ];
    let mut execution = TerminalExecution::start_artifact_with_structural_arguments(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[TerminalScalarValue::Boolean(true)],
        &structural_arguments,
    )
    .expect("verified mixed scalar cleanup starts");
    let mut meter = TerminalFuelMeter::with_allowance(0);

    for (consumed, expected_site) in [
        FuelChargeSite::Edge(edge_id(1)),
        FuelChargeSite::Edge(edge_id(4)),
        FuelChargeSite::Edge(edge_id(2)),
    ]
    .into_iter()
    .enumerate()
    {
        assert!(matches!(
            execution.resume(&mut meter).unwrap(),
            TerminalExecutionStatus::SponsorExhausted(FuelExhaustion { site, .. })
                if site == expected_site
        ));
        assert_eq!(meter.usage().total_units(), consumed as u64);
        meter.replenish(1).unwrap();
    }

    assert_eq!(
        execution.resume(&mut meter).unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Scalar(
            TerminalScalarValue::Boolean(true)
        ))
    );
    assert_eq!(meter.usage().total_units(), 3);
    assert!(execution.live_affine_frontier().next().is_none());
    assert_eq!(
        meter
            .usage()
            .at(FuelChargeSite::Edge(edge_id(1)))
            .unwrap()
            .executions(),
        1,
        "the mixed cleanup stream shares one scalar-return edge charge"
    );
}

fn ordered_empty_nominal_affine_module(same_target: bool) -> TerminalModule {
    let mut module = nominal_affine_module();
    let first_type = structural_type_id(1);
    let second_type = if same_target {
        first_type
    } else {
        let second_type = structural_type_id(2);
        module.structural_types.push(StructuralTypeDeclaration {
            id: second_type,
            identity: "SecondToken".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![StructuralFieldDeclaration {
                    identity: "payload".into(),
                    id: psi_core::StructuralFieldId::new(2).unwrap(),
                    field_type: StructuralFieldType::Scalar(ScalarType::Integer(
                        psi_core::IntegerType::new(psi_core::IntegerSign::Unsigned, 64).unwrap(),
                    )),
                    relevance: BindingRelevance::Relevant,
                }],
            },
        });
        second_type
    };
    module.structural_types[0].shape = StructuralTypeShape::Record {
        fields: vec![StructuralFieldDeclaration {
            identity: "payload".into(),
            id: psi_core::StructuralFieldId::new(1).unwrap(),
            field_type: StructuralFieldType::Scalar(ScalarType::Integer(
                psi_core::IntegerType::new(psi_core::IntegerSign::Unsigned, 32).unwrap(),
            )),
            relevance: BindingRelevance::Relevant,
        }],
    };
    let second_cleanup_machine = if same_target {
        machine_id(2)
    } else {
        let mut target = module.machines[1].clone();
        target.id = machine_id(3);
        target.attachment = Some(second_type);
        target.entry = block_id(3);
        target.blocks[0].id = block_id(3);
        target.blocks[0].terminator = Terminator::ReturnUnit {
            edge: edge_id(3),
            trivial_affine_discards: Vec::new(),
        };
        target.contract.id = contract_id(3);
        module.machines.push(target);
        machine_id(3)
    };
    let caller = &mut module.machines[0];
    caller
        .structural_parameters
        .push(StructuralParameterDeclaration {
            place: place_id(2),
            position: 1,
            is_self: false,
            structural_type: second_type,
            multiplicity: StructuralMultiplicity::Affine,
            access: StructuralAccess::Owned,
            qualifications: Vec::new(),
        });
    caller.structural_places.push(StructuralPlaceDeclaration {
        id: place_id(2),
        kind: psi_core::StructuralPlaceKind::Parameter {
            position: 1,
            is_self: false,
        },
    });
    caller.blocks[0].terminator = Terminator::ReturnUnitNominalAffine {
        edge: edge_id(1),
        cleanups: vec![
            NominalAffineCleanup {
                place: place_id(2),
                structural_type: second_type,
                cleanup_machine: second_cleanup_machine,
                cleanup_receiver: None,
                requirement_obligations: Vec::new(),
            },
            NominalAffineCleanup {
                place: place_id(1),
                structural_type: first_type,
                cleanup_machine: machine_id(2),
                cleanup_receiver: None,
                requirement_obligations: Vec::new(),
            },
        ],
    };
    module
}

fn ordered_one_executable_nominal_affine_module() -> TerminalModule {
    let mut module = ordered_empty_nominal_affine_module(false);
    let helper_type = structural_type_id(3);
    module.structural_types.push(StructuralTypeDeclaration {
        id: helper_type,
        identity: "Helper".into(),
        shape: StructuralTypeShape::Record { fields: Vec::new() },
    });
    let mut helper = module.machines[1].clone();
    helper.id = machine_id(4);
    helper.attachment = Some(helper_type);
    helper.entry = block_id(4);
    helper.blocks[0].id = block_id(4);
    helper.blocks[0].terminator = Terminator::ReturnUnit {
        edge: edge_id(4),
        trivial_affine_discards: Vec::new(),
    };
    helper.contract.id = contract_id(4);
    module.machines[2].blocks[0].operations.push(Operation {
        id: operation_id(1),
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

fn three_ordered_empty_nominal_affine_module(same_target: bool) -> TerminalModule {
    let mut module = ordered_empty_nominal_affine_module(same_target);
    let third_type = if same_target {
        structural_type_id(1)
    } else {
        let third_type = structural_type_id(3);
        module.structural_types.push(StructuralTypeDeclaration {
            id: third_type,
            identity: "ThirdToken".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![StructuralFieldDeclaration {
                    identity: "payload".into(),
                    id: psi_core::StructuralFieldId::new(3).unwrap(),
                    field_type: StructuralFieldType::Scalar(ScalarType::Integer(
                        psi_core::IntegerType::new(psi_core::IntegerSign::Unsigned, 64).unwrap(),
                    )),
                    relevance: BindingRelevance::Relevant,
                }],
            },
        });
        third_type
    };
    let third_cleanup_machine = if same_target {
        machine_id(2)
    } else {
        let mut target = module.machines[1].clone();
        target.id = machine_id(4);
        target.attachment = Some(third_type);
        target.entry = block_id(4);
        target.blocks[0].id = block_id(4);
        target.blocks[0].terminator = Terminator::ReturnUnit {
            edge: edge_id(4),
            trivial_affine_discards: Vec::new(),
        };
        target.contract.id = contract_id(4);
        module.machines.push(target);
        machine_id(4)
    };
    let caller = &mut module.machines[0];
    caller
        .structural_parameters
        .push(StructuralParameterDeclaration {
            place: place_id(3),
            position: 2,
            is_self: false,
            structural_type: third_type,
            multiplicity: StructuralMultiplicity::Affine,
            access: StructuralAccess::Owned,
            qualifications: Vec::new(),
        });
    caller.structural_places.push(StructuralPlaceDeclaration {
        id: place_id(3),
        kind: psi_core::StructuralPlaceKind::Parameter {
            position: 2,
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
            place: place_id(3),
            structural_type: third_type,
            cleanup_machine: third_cleanup_machine,
            cleanup_receiver: None,
            requirement_obligations: Vec::new(),
        },
    );
    module
}

fn ordered_two_distinct_executable_nominal_affine_module() -> TerminalModule {
    let mut module = ordered_one_executable_nominal_affine_module();
    let helper_type = structural_type_id(4);
    module.structural_types.push(StructuralTypeDeclaration {
        id: helper_type,
        identity: "SecondHelper".into(),
        shape: StructuralTypeShape::Record { fields: Vec::new() },
    });
    let mut helper = module.machines[3].clone();
    helper.id = machine_id(5);
    helper.attachment = Some(helper_type);
    helper.entry = block_id(5);
    helper.blocks[0].id = block_id(5);
    helper.blocks[0].terminator = Terminator::ReturnUnit {
        edge: edge_id(5),
        trivial_affine_discards: Vec::new(),
    };
    helper.contract.id = contract_id(5);
    module.machines[1].blocks[0].operations.push(Operation {
        id: operation_id(2),
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

fn ordered_shared_executable_nominal_affine_module() -> TerminalModule {
    let mut module = ordered_empty_nominal_affine_module(true);
    let helper_type = structural_type_id(2);
    module.structural_types.push(StructuralTypeDeclaration {
        id: helper_type,
        identity: "SharedHelper".into(),
        shape: StructuralTypeShape::Record { fields: Vec::new() },
    });
    let mut helper = module.machines[1].clone();
    helper.id = machine_id(3);
    helper.attachment = Some(helper_type);
    helper.entry = block_id(3);
    helper.blocks[0].id = block_id(3);
    helper.blocks[0].terminator = Terminator::ReturnUnit {
        edge: edge_id(3),
        trivial_affine_discards: Vec::new(),
    };
    helper.contract.id = contract_id(3);
    module.machines[1].blocks[0].operations.push(Operation {
        id: operation_id(1),
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

fn three_ordered_shared_executable_nominal_affine_module() -> TerminalModule {
    let mut module = ordered_shared_executable_nominal_affine_module();
    let caller = &mut module.machines[0];
    caller
        .structural_parameters
        .push(StructuralParameterDeclaration {
            place: place_id(3),
            position: 2,
            is_self: false,
            structural_type: structural_type_id(1),
            multiplicity: StructuralMultiplicity::Affine,
            access: StructuralAccess::Owned,
            qualifications: Vec::new(),
        });
    caller.structural_places.push(StructuralPlaceDeclaration {
        id: place_id(3),
        kind: psi_core::StructuralPlaceKind::Parameter {
            position: 2,
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
            place: place_id(3),
            structural_type: structural_type_id(1),
            cleanup_machine: machine_id(2),
            cleanup_receiver: None,
            requirement_obligations: Vec::new(),
        },
    );
    module
}

fn executable_nominal_affine_module() -> TerminalModule {
    let mut module = nominal_affine_module();
    let helper_type = StructuralTypeDeclaration {
        id: structural_type_id(2),
        identity: "Helper".into(),
        shape: StructuralTypeShape::Record { fields: Vec::new() },
    };
    module.structural_types.push(helper_type.clone());
    module.machines[1].blocks[0].operations.push(Operation {
        id: operation_id(1),
        result: OperationResult::Unit,
        kind: OperationKind::CallUnit {
            callee: machine_id(3),
            structural_arguments: Vec::new(),
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
    });
    module.machines.push(TerminalMachine {
        id: machine_id(3),
        attachment: Some(helper_type.id),
        parameters: Vec::new(),
        structural_parameters: Vec::new(),
        result: TerminalMachineResult::Unit,
        structural_places: Vec::new(),
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: block_id(3),
        blocks: vec![Block {
            id: block_id(3),
            parameters: Vec::new(),
            operations: Vec::new(),
            terminator: Terminator::ReturnUnit {
                edge: edge_id(3),
                trivial_affine_discards: Vec::new(),
            },
        }],
        contract: empty_contract(contract_id(3)),
    });
    module
}

fn two_helper_nominal_affine_module() -> TerminalModule {
    let mut module = executable_nominal_affine_module();
    let second_helper_type = StructuralTypeDeclaration {
        id: structural_type_id(3),
        identity: "SecondHelper".into(),
        shape: StructuralTypeShape::Record { fields: Vec::new() },
    };
    module.structural_types.push(second_helper_type.clone());
    module.machines[1].blocks[0].operations.push(Operation {
        id: operation_id(2),
        result: OperationResult::Unit,
        kind: OperationKind::CallUnit {
            callee: machine_id(4),
            structural_arguments: Vec::new(),
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
    });
    module.machines.push(TerminalMachine {
        id: machine_id(4),
        attachment: Some(second_helper_type.id),
        parameters: Vec::new(),
        structural_parameters: Vec::new(),
        result: TerminalMachineResult::Unit,
        structural_places: Vec::new(),
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: block_id(4),
        blocks: vec![Block {
            id: block_id(4),
            parameters: Vec::new(),
            operations: Vec::new(),
            terminator: Terminator::ReturnUnit {
                edge: edge_id(4),
                trivial_affine_discards: Vec::new(),
            },
        }],
        contract: empty_contract(contract_id(4)),
    });
    module
}

fn three_helper_nominal_affine_module() -> TerminalModule {
    let mut module = two_helper_nominal_affine_module();
    let third_helper_type = StructuralTypeDeclaration {
        id: structural_type_id(4),
        identity: "ThirdHelper".into(),
        shape: StructuralTypeShape::Record { fields: Vec::new() },
    };
    module.structural_types.push(third_helper_type.clone());
    module.machines[1].blocks[0].operations.push(Operation {
        id: operation_id(3),
        result: OperationResult::Unit,
        kind: OperationKind::CallUnit {
            callee: machine_id(5),
            structural_arguments: Vec::new(),
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
    });
    let mut third_helper = module.machines[2].clone();
    third_helper.id = machine_id(5);
    third_helper.attachment = Some(third_helper_type.id);
    third_helper.entry = block_id(5);
    third_helper.blocks[0].id = block_id(5);
    third_helper.blocks[0].terminator = Terminator::ReturnUnit {
        edge: edge_id(5),
        trivial_affine_discards: Vec::new(),
    };
    third_helper.contract.id = contract_id(5);
    module.machines.push(third_helper);
    module
}

fn partial_affine_field_module() -> TerminalModule {
    let token = StructuralTypeDeclaration {
        id: structural_type_id(1),
        identity: "Token".into(),
        shape: StructuralTypeShape::Record { fields: Vec::new() },
    };
    let pair = StructuralTypeDeclaration {
        id: structural_type_id(2),
        identity: "Pair".into(),
        shape: StructuralTypeShape::Record {
            fields: vec![
                StructuralFieldDeclaration {
                    id: psi_core::StructuralFieldId::new(1).unwrap(),
                    identity: "left".into(),
                    relevance: BindingRelevance::Relevant,
                    field_type: StructuralFieldType::Structural(token.id),
                },
                StructuralFieldDeclaration {
                    id: psi_core::StructuralFieldId::new(2).unwrap(),
                    identity: "right".into(),
                    relevance: BindingRelevance::Relevant,
                    field_type: StructuralFieldType::Structural(token.id),
                },
            ],
        },
    };
    let caller = TerminalMachine {
        id: machine_id(1),
        attachment: None,
        parameters: Vec::new(),
        structural_parameters: vec![StructuralParameterDeclaration {
            place: place_id(1),
            position: 0,
            is_self: false,
            structural_type: pair.id,
            multiplicity: StructuralMultiplicity::Affine,
            access: StructuralAccess::Owned,
            qualifications: Vec::new(),
        }],
        result: TerminalMachineResult::Unit,
        structural_places: vec![StructuralPlaceDeclaration {
            id: place_id(1),
            kind: psi_core::StructuralPlaceKind::Parameter {
                position: 0,
                is_self: false,
            },
        }],
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: block_id(1),
        blocks: vec![Block {
            id: block_id(1),
            parameters: Vec::new(),
            operations: vec![Operation {
                id: operation_id(1),
                result: OperationResult::Unit,
                kind: OperationKind::CallUnit {
                    callee: machine_id(2),
                    structural_arguments: vec![StructuralArgument {
                        place: place_id(1),
                        access: StructuralAccess::Owned,
                        path: vec![StructuralPathSegment::Field("right".into())],
                    }],
                    claim_transfers: Vec::new(),
                    requirement_obligations: Vec::new(),
                    crash_continuations: Vec::new(),
                },
            }],
            terminator: Terminator::ReturnUnitPartialAffine {
                edge: edge_id(1),
                trivial_affine_discards: Vec::new(),
                residual_affine_discards: vec![StructuralAffineDiscard {
                    place: place_id(1),
                    path: vec![StructuralPathSegment::Field("left".into())],
                    structural_type: token.id,
                }],
            },
        }],
        contract: empty_contract(contract_id(1)),
    };
    let callee = TerminalMachine {
        id: machine_id(2),
        attachment: None,
        parameters: Vec::new(),
        structural_parameters: vec![StructuralParameterDeclaration {
            place: place_id(2),
            position: 0,
            is_self: false,
            structural_type: token.id,
            multiplicity: StructuralMultiplicity::Affine,
            access: StructuralAccess::Owned,
            qualifications: Vec::new(),
        }],
        result: TerminalMachineResult::Unit,
        structural_places: vec![StructuralPlaceDeclaration {
            id: place_id(2),
            kind: psi_core::StructuralPlaceKind::Parameter {
                position: 0,
                is_self: false,
            },
        }],
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: block_id(2),
        blocks: vec![Block {
            id: block_id(2),
            parameters: Vec::new(),
            operations: Vec::new(),
            terminator: Terminator::ReturnUnit {
                edge: edge_id(2),
                trivial_affine_discards: vec![place_id(2)],
            },
        }],
        contract: empty_contract(contract_id(2)),
    };
    TerminalModule {
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: caller.id,
        structural_types: vec![token, pair],
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
        proof_output_calls: Vec::new(),
        closed_conformance_applications: Vec::new(),
        machines: vec![caller, callee],
    }
}

fn internal_structural_call_module(crashes: bool) -> TerminalModule {
    let structural_type = structural_type_id(1);
    let domain = structural_domain_id(1);
    let caller_source = place_id(1);
    let caller_result = place_id(2);
    let operation_result = place_id(3);
    let callee_source = place_id(4);
    let callee_result = place_id(5);
    let claim = claim_id(1);
    let crash_route = CrashRouteBucket {
        cause: CrashCause::Trap,
        alternatives: vec![CrashRouteGuard::Truth],
    };
    let caller = TerminalMachine {
        id: machine_id(1),
        attachment: None,
        parameters: Vec::new(),
        structural_parameters: vec![StructuralParameterDeclaration {
            place: caller_source,
            position: 0,
            is_self: false,
            structural_type,
            multiplicity: StructuralMultiplicity::Linear,
            access: StructuralAccess::Owned,
            qualifications: vec![domain],
        }],
        result: TerminalMachineResult::Structural(StructuralResultDeclaration {
            place: caller_result,
            structural_type,
            multiplicity: StructuralMultiplicity::Linear,
            qualifications: vec![domain],
        }),
        structural_places: vec![
            StructuralPlaceDeclaration {
                id: caller_source,
                kind: psi_core::StructuralPlaceKind::Parameter {
                    position: 0,
                    is_self: false,
                },
            },
            StructuralPlaceDeclaration {
                id: caller_result,
                kind: psi_core::StructuralPlaceKind::Result,
            },
            StructuralPlaceDeclaration {
                id: operation_result,
                kind: psi_core::StructuralPlaceKind::OperationResult {
                    producer: operation_id(1),
                    structural_type,
                },
            },
        ],
        entry_claims: vec![EntryClaim {
            claim,
            input: caller_source,
            path: Vec::new(),
        }],
        published_service_ceiling: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: block_id(1),
        blocks: vec![Block {
            id: block_id(1),
            parameters: Vec::new(),
            operations: vec![Operation {
                id: operation_id(1),
                result: OperationResult::Structural(StructuralOperationResult {
                    place: operation_result,
                    structural_type,
                    multiplicity: StructuralMultiplicity::Linear,
                    qualifications: vec![domain],
                    claims: vec![StructuralResultClaimBinding {
                        claim,
                        path: Vec::new(),
                    }],
                }),
                kind: OperationKind::CallStructural {
                    callee: machine_id(2),
                    structural_arguments: vec![StructuralArgument {
                        place: caller_source,
                        path: Vec::new(),
                        access: StructuralAccess::Owned,
                    }],
                    claim_transfers: vec![ClaimTransfer {
                        claim,
                        argument_index: 0,
                    }],
                    returned_claim_transfers: vec![StructuralResultClaimTransfer {
                        callee_claim: claim,
                        caller_claim: claim,
                    }],
                    requirement_obligations: Vec::new(),
                    crash_continuations: crashes
                        .then(|| vec![crash_route.clone()])
                        .unwrap_or_default(),
                },
            }],
            terminator: Terminator::ReturnStructural {
                edge: edge_id(1),
                source: operation_result,
                returned_claims: vec![claim],
                trivial_affine_discards: Vec::new(),
            },
        }],
        contract: MachineContract {
            id: contract_id(1),
            crash_routes: crashes
                .then(|| vec![crash_route.clone()])
                .unwrap_or_default(),
            requires: Vec::new(),
            ensures: Vec::new(),
            outcome_specific_ensures: Vec::new(),
        },
    };
    let callee = TerminalMachine {
        id: machine_id(2),
        attachment: None,
        parameters: Vec::new(),
        structural_parameters: vec![StructuralParameterDeclaration {
            place: callee_source,
            position: 0,
            is_self: false,
            structural_type,
            multiplicity: StructuralMultiplicity::Linear,
            access: StructuralAccess::Owned,
            qualifications: vec![domain],
        }],
        result: TerminalMachineResult::Structural(StructuralResultDeclaration {
            place: callee_result,
            structural_type,
            multiplicity: StructuralMultiplicity::Linear,
            qualifications: vec![domain],
        }),
        structural_places: vec![
            StructuralPlaceDeclaration {
                id: callee_source,
                kind: psi_core::StructuralPlaceKind::Parameter {
                    position: 0,
                    is_self: false,
                },
            },
            StructuralPlaceDeclaration {
                id: callee_result,
                kind: psi_core::StructuralPlaceKind::Result,
            },
        ],
        entry_claims: vec![EntryClaim {
            claim,
            input: callee_source,
            path: Vec::new(),
        }],
        published_service_ceiling: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: block_id(2),
        blocks: vec![Block {
            id: block_id(2),
            parameters: Vec::new(),
            operations: Vec::new(),
            terminator: if crashes {
                Terminator::Crash {
                    edge: edge_id(2),
                    cause: CrashCause::Trap,
                    site_guard: Vec::new(),
                    frontier_lower_bound: vec![claim],
                }
            } else {
                Terminator::ReturnStructural {
                    edge: edge_id(2),
                    source: callee_source,
                    returned_claims: vec![claim],
                    trivial_affine_discards: Vec::new(),
                }
            },
        }],
        contract: MachineContract {
            id: contract_id(2),
            crash_routes: crashes.then(|| vec![crash_route]).unwrap_or_default(),
            requires: Vec::new(),
            ensures: Vec::new(),
            outcome_specific_ensures: Vec::new(),
        },
    };
    TerminalModule {
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: caller.id,
        structural_types: vec![StructuralTypeDeclaration {
            id: structural_type,
            identity: "test::Resource".into(),
            shape: StructuralTypeShape::Record { fields: Vec::new() },
        }],
        structural_domains: vec![StructuralDomainDeclaration {
            id: domain,
            semantic_domain: psi_core::DomainSemanticId::new(1).expect("semantic domain identity"),
            identity: "test::Owned".into(),
            carrier: structural_type,
            content_projection: None,
        }],
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
        proof_output_calls: Vec::new(),
        closed_conformance_applications: Vec::new(),
        machines: vec![caller, callee],
    }
}

fn multi_claim_internal_structural_call_module(crashes: bool) -> TerminalModule {
    let mut module = internal_structural_call_module(crashes);
    let element_type = structural_type_id(2);
    module.structural_types[0].shape = StructuralTypeShape::FixedArray {
        element: element_type,
        length: 2,
    };
    module.structural_types.push(StructuralTypeDeclaration {
        id: element_type,
        identity: "test::ResourceElement".into(),
        shape: StructuralTypeShape::Record { fields: Vec::new() },
    });
    let paths = [
        vec![psi_terminal::StructuralPathSegment::FixedIndex(0)],
        vec![psi_terminal::StructuralPathSegment::FixedIndex(1)],
    ];

    for machine in &mut module.machines {
        machine.entry_claims = paths
            .iter()
            .enumerate()
            .map(|(index, path)| EntryClaim {
                claim: claim_id(index as u64 + 1),
                input: machine.structural_parameters[0].place,
                path: path.clone(),
            })
            .collect();
        match &mut machine.blocks[0].terminator {
            Terminator::ReturnStructural {
                returned_claims, ..
            } => *returned_claims = vec![claim_id(1), claim_id(2)],
            Terminator::Crash {
                frontier_lower_bound,
                ..
            } => *frontier_lower_bound = vec![claim_id(1), claim_id(2)],
            _ => unreachable!(),
        }
    }

    let operation = &mut module.machines[0].blocks[0].operations[0];
    let OperationResult::Structural(result) = &mut operation.result else {
        unreachable!()
    };
    result.claims = paths
        .iter()
        .enumerate()
        .map(|(index, path)| StructuralResultClaimBinding {
            claim: claim_id(index as u64 + 1),
            path: path.clone(),
        })
        .collect();
    let OperationKind::CallStructural {
        claim_transfers,
        returned_claim_transfers,
        ..
    } = &mut operation.kind
    else {
        unreachable!()
    };
    *claim_transfers = vec![
        ClaimTransfer {
            claim: claim_id(1),
            argument_index: 0,
        },
        ClaimTransfer {
            claim: claim_id(2),
            argument_index: 0,
        },
    ];
    *returned_claim_transfers = vec![
        StructuralResultClaimTransfer {
            callee_claim: claim_id(1),
            caller_claim: claim_id(1),
        },
        StructuralResultClaimTransfer {
            callee_claim: claim_id(2),
            caller_claim: claim_id(2),
        },
    ];
    module
}

fn machine_id(raw: u64) -> MachineId {
    MachineId::new(raw).unwrap()
}

fn block_id(raw: u64) -> BlockId {
    BlockId::new(raw).unwrap()
}

fn edge_id(raw: u64) -> EdgeId {
    EdgeId::new(raw).unwrap()
}

fn contract_id(raw: u64) -> ContractId {
    ContractId::new(raw).unwrap()
}

fn boundary_id(raw: u64) -> BoundaryMachineId {
    BoundaryMachineId::new(raw).unwrap()
}

fn operation_id(raw: u64) -> OperationId {
    OperationId::new(raw).unwrap()
}

fn obligation_id(raw: u64) -> ObligationId {
    ObligationId::new(raw).unwrap()
}

fn place_id(raw: u64) -> PlaceId {
    PlaceId::new(raw).unwrap()
}

fn value_id(raw: u64) -> ValueId {
    ValueId::new(raw).unwrap()
}

fn claim_id(raw: u64) -> ClaimId {
    ClaimId::new(raw).unwrap()
}

fn structural_type_id(raw: u64) -> StructuralTypeId {
    StructuralTypeId::new(raw).unwrap()
}

fn structural_case_id(raw: u64) -> StructuralCaseId {
    StructuralCaseId::new(raw).unwrap()
}

fn structural_domain_id(raw: u64) -> StructuralDomainId {
    StructuralDomainId::new(raw).unwrap()
}

fn service_id(raw: u64) -> ServiceId {
    ServiceId::new(raw).unwrap()
}
