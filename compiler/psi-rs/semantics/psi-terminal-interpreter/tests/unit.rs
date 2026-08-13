use psi_core::{
    BlockId, BoundaryMachineId, ClaimId, ContractId, EdgeId, MachineId, OperationId, PlaceId,
    ScalarType, ServiceId, StructuralDomainId, StructuralTypeId, ValueId,
};
use psi_proof_kernel::AdmissionProfile;
use psi_terminal::{
    BindingRelevance, Block, BoundaryMachineDeclaration, ClaimTransfer, CompletionReceipt,
    EntryClaim, MachineContract, NominalAffineCleanup, Operation, OperationKind, OperationResult,
    ServiceDeclaration, StructuralAffineDiscard, StructuralArgument, StructuralDomainDeclaration,
    StructuralDomainRequirement, StructuralFieldDeclaration, StructuralFieldType,
    StructuralMultiplicity, StructuralParameterDeclaration, StructuralPathSegment,
    StructuralPlaceDeclaration, StructuralResultDeclaration, StructuralTypeDeclaration,
    StructuralTypeShape, SuccessorEdge, TerminalMachine, TerminalMachineResult, TerminalModule,
    Terminator, ValueDeclaration, VocabularyMarker,
};
use psi_terminal_codec::{encode_module, encode_proof_bundle};
use psi_terminal_fuel::{FuelChargeSite, FuelExhaustion, TerminalFuelMeter, TerminalFuelSchedule};
use psi_terminal_interpreter::{
    TerminalEffect, TerminalEffectHandler, TerminalEffectRejection, TerminalExecution,
    TerminalExecutionResult, TerminalExecutionStatus, TerminalInterpretError, TerminalScalarValue,
    TerminalStructuralValue, interpret_terminal_artifact_measured,
    interpret_terminal_artifact_with_effect_handler_measured,
};
use psi_terminal_verifier::ProofBundle;

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
            identity: "test::Owned".into(),
            carrier: structural_type,
        }],
        services: Vec::new(),
        boundary_machines: Vec::new(),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
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
        trivial_affine_discards: vec![place_id(2)],
    };
    module.entry = machine.id;
    module.machines = vec![machine];
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
                trivial_affine_discards: Vec::new(),
            },
        },
    ];
    module.entry = machine.id;
    module.machines = vec![machine];
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
                trivial_affine_discards: Vec::new(),
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
                trivial_affine_discards: vec![place_id(2)],
            },
        },
    ];
    module.entry = machine.id;
    module.machines = vec![machine];
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
        TerminalEffect::BoundaryCallUnit {
            operation: operation_id(3),
            boundary: boundary_id(1),
            structural_arguments: vec![argument],
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
        TerminalEffect::BoundaryCallUnit {
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
    let OperationKind::BoundaryCallUnit {
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
        TerminalEffect::BoundaryCallUnit {
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
        TerminalEffect::BoundaryCallUnit { .. }
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

fn effect_artifact_sections() -> (Vec<u8>, Vec<u8>) {
    (
        encode_module(&effect_module()).expect("effect semantics encode"),
        encode_proof_bundle(&ProofBundle::default()).expect("empty proof encodes"),
    )
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
            identity: "test::Ready".into(),
            carrier: structural_type,
        }],
        services: vec![ServiceDeclaration {
            id: service,
            identity: "test::PortIo".into(),
            parents: Vec::new(),
        }],
        boundary_machines: vec![BoundaryMachineDeclaration {
            id: boundary_id(1),
            identity: "test::acknowledge".into(),
            attachment: Some(structural_type),
            structural_parameters: vec![structural_parameter(place_id(3), structural_type, domain)],
            requires: vec![StructuralDomainRequirement {
                argument_index: 0,
                domain,
            }],
            published_service_ceiling: Vec::new(),
        }],
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
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
                        kind: OperationKind::BoundaryCallUnit {
                            boundary: boundary_id(1),
                            structural_arguments: vec![StructuralArgument {
                                place: place_id(2),
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
        boundary_machines: Vec::new(),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
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
            },
            NominalAffineCleanup {
                place: place_id(1),
                structural_type: first_type,
                cleanup_machine: machine_id(2),
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
        boundary_machines: Vec::new(),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        machines: vec![caller, callee],
    }
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

fn structural_domain_id(raw: u64) -> StructuralDomainId {
    StructuralDomainId::new(raw).unwrap()
}

fn service_id(raw: u64) -> ServiceId {
    ServiceId::new(raw).unwrap()
}
