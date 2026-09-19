use super::{
    BoundaryResultHandler, RecordingHandler, RejectScalarBoundaryArguments, block_id, boundary_id,
    byte_sequence_literal_module, claim_id, edge_id, effect_artifact_sections, effect_module,
    executable_nominal_affine_module, nominal_affine_module, operation_id,
    ordered_empty_nominal_affine_module, ordered_one_executable_nominal_affine_module,
    ordered_shared_executable_nominal_affine_module,
    ordered_two_distinct_executable_nominal_affine_module, partial_affine_field_module, place_id,
    scalar_boundary_effect_module, service_id, structural_boundary_effect_module,
    structural_domain_id, structural_type_id, structural_value, three_helper_nominal_affine_module,
    three_ordered_empty_nominal_affine_module,
    three_ordered_shared_executable_nominal_affine_module, two_helper_nominal_affine_module,
    value_id,
};
use proof_admission::AdmissionProfile;
use semantic_vocabulary::ScalarType;
use terminal_codec::{decode_module, encode_module, encode_proof_section};
use terminal_fuel::{FuelChargeSite, FuelExhaustion, TerminalFuelMeter, TerminalFuelSchedule};
use terminal_interpreter::AcceptTerminalEffects;
use terminal_interpreter::TerminalStructuralInputs;
use terminal_interpreter::{
    TerminalEffect, TerminalEffectRejection, TerminalEffectResult, TerminalExecution,
    TerminalExecutionResult, TerminalExecutionStatus, TerminalInterpretError, TerminalScalarValue,
    TerminalStructuralValue, interpret_terminal_artifact_measured,
};
use terminal_psi::{
    BindingRelevance, Block, CompletionReceipt, OperationKind, StructuralAffineDiscard,
    StructuralFieldDeclaration, StructuralFieldType, StructuralMultiplicity, StructuralPathSegment,
    StructuralTypeDeclaration, StructuralTypeShape, SuccessorEdge, TerminalAffineCleanupAction,
    TerminalMachineResult, Terminator, ValueDeclaration,
};
use terminal_verifier::ProofBundle;

#[test]
fn partial_affine_return_charges_edge_before_exact_residual_cleanup() {
    let module = partial_affine_field_module();
    let semantic = encode_module(&module).expect("partial affine module encodes");
    let proof =
        encode_proof_section(&module, &ProofBundle::default()).expect("empty proof encodes");
    let argument = TerminalStructuralValue {
        opaque_identity: 50,
        structural_type: structural_type_id(2),
        qualifications: Vec::new(),
        path: Vec::new(),
    };
    let mut execution = TerminalExecution::start_artifact(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs {
            arguments: &[argument],
            ..Default::default()
        },
    )
    .expect("verified partial affine cleanup should start");
    // Call operation + callee return consume the first two units. The caller's
    // partial return then suspends before touching its residual path.
    let mut meter = TerminalFuelMeter::with_allowance(2);

    assert_eq!(
        execution
            .resume(&mut meter, &mut AcceptTerminalEffects)
            .unwrap(),
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
        execution
            .resume(&mut meter, &mut AcceptTerminalEffects)
            .unwrap(),
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
                id: semantic_vocabulary::StructuralFieldId::new(index).unwrap(),
                field_type: StructuralFieldType::Scalar(ScalarType::Integer(
                    semantic_vocabulary::IntegerType::new(
                        semantic_vocabulary::IntegerSign::Unsigned,
                        64,
                    )
                    .unwrap(),
                )),
                relevance: BindingRelevance::Relevant,
            })
            .collect(),
    };
    let semantic = encode_module(&module).expect("nominal cleanup encodes");
    let proof =
        encode_proof_section(&module, &ProofBundle::default()).expect("empty proof encodes");
    let argument = TerminalStructuralValue {
        opaque_identity: 70,
        structural_type: structural_type_id(1),
        qualifications: Vec::new(),
        path: Vec::new(),
    };
    let mut execution = TerminalExecution::start_artifact(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs {
            arguments: &[argument],
            ..Default::default()
        },
    )
    .expect("verified nominal cleanup should start");
    let mut meter = TerminalFuelMeter::with_allowance(0);

    assert!(matches!(
        execution.resume(&mut meter, &mut AcceptTerminalEffects).unwrap(),
        TerminalExecutionStatus::SponsorExhausted(FuelExhaustion {
            site: FuelChargeSite::Edge(edge),
            ..
        }) if edge == edge_id(1)
    ));
    assert_eq!(execution.live_affine_frontier().count(), 1);

    meter.replenish(1).unwrap();
    assert!(matches!(
        execution.resume(&mut meter, &mut AcceptTerminalEffects).unwrap(),
        TerminalExecutionStatus::SponsorExhausted(FuelExhaustion {
            site: FuelChargeSite::Edge(edge),
            ..
        }) if edge == edge_id(2)
    ));
    assert_eq!(meter.usage().total_units(), 1);

    meter.replenish(1).unwrap();
    assert_eq!(
        execution
            .resume(&mut meter, &mut AcceptTerminalEffects)
            .unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
    assert!(execution.live_affine_frontier().next().is_none());
    assert_eq!(meter.usage().total_units(), 2);
}

#[test]
fn ordered_nominal_affine_cleanups_run_in_reverse_parameter_order_after_one_root_charge() {
    let module = ordered_empty_nominal_affine_module(false);
    let semantic = encode_module(&module).expect("ordered nominal cleanups encode");
    let proof =
        encode_proof_section(&module, &ProofBundle::default()).expect("empty proof encodes");
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
    let mut execution = TerminalExecution::start_artifact(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs {
            arguments: &arguments,
            ..Default::default()
        },
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
            execution.resume(&mut meter, &mut AcceptTerminalEffects).unwrap(),
            TerminalExecutionStatus::SponsorExhausted(FuelExhaustion {
                site: exhausted_site,
                ..
            }) if exhausted_site == site
        ));
        assert_eq!(meter.usage().total_units(), consumed as u64);
        meter.replenish(1).unwrap();
    }

    assert_eq!(
        execution
            .resume(&mut meter, &mut AcceptTerminalEffects)
            .unwrap(),
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
    let proof =
        encode_proof_section(&module, &ProofBundle::default()).expect("empty proof encodes");
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
    let mut execution = TerminalExecution::start_artifact(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs {
            arguments: &arguments,
            ..Default::default()
        },
    )
    .expect("verified same-target nominal cleanups should start");
    let mut meter = TerminalFuelMeter::with_allowance(3);

    assert_eq!(
        execution
            .resume(&mut meter, &mut AcceptTerminalEffects)
            .unwrap(),
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
    let proof =
        encode_proof_section(&module, &ProofBundle::default()).expect("empty proof encodes");
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
    let mut execution = TerminalExecution::start_artifact(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs {
            arguments: &arguments,
            ..Default::default()
        },
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
            execution.resume(&mut meter, &mut AcceptTerminalEffects).unwrap(),
            TerminalExecutionStatus::SponsorExhausted(FuelExhaustion {
                site: exhausted_site,
                ..
            }) if exhausted_site == site
        ));
        assert_eq!(meter.usage().total_units(), consumed as u64);
        meter.replenish(1).unwrap();
    }
    assert_eq!(
        execution
            .resume(&mut meter, &mut AcceptTerminalEffects)
            .unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
    assert_eq!(meter.usage().total_units(), 4);
}

#[test]
fn ordered_nominal_affine_cleanups_run_one_executable_body_before_the_empty_action() {
    let module = ordered_one_executable_nominal_affine_module();
    let semantic = encode_module(&module).expect("ordered executable nominal cleanups encode");
    let proof =
        encode_proof_section(&module, &ProofBundle::default()).expect("empty proof encodes");
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
    let mut execution = TerminalExecution::start_artifact(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs {
            arguments: &arguments,
            ..Default::default()
        },
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
            execution.resume(&mut meter, &mut AcceptTerminalEffects).unwrap(),
            TerminalExecutionStatus::SponsorExhausted(FuelExhaustion {
                site: exhausted_site,
                ..
            }) if exhausted_site == site
        ));
        assert_eq!(meter.usage().total_units(), consumed as u64);
        meter.replenish(1).unwrap();
    }

    assert_eq!(
        execution
            .resume(&mut meter, &mut AcceptTerminalEffects)
            .unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
    assert_eq!(meter.usage().total_units(), 5);
}

#[test]
fn ordered_nominal_affine_cleanups_run_two_distinct_executable_bodies_in_order() {
    let module = ordered_two_distinct_executable_nominal_affine_module();
    let semantic = encode_module(&module).expect("two executable nominal cleanups encode");
    let proof =
        encode_proof_section(&module, &ProofBundle::default()).expect("empty proof encodes");
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
    let mut execution = TerminalExecution::start_artifact(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs {
            arguments: &arguments,
            ..Default::default()
        },
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
            execution.resume(&mut meter, &mut AcceptTerminalEffects).unwrap(),
            TerminalExecutionStatus::SponsorExhausted(FuelExhaustion {
                site: exhausted_site,
                ..
            }) if exhausted_site == site
        ));
        assert_eq!(meter.usage().total_units(), consumed as u64);
        meter.replenish(1).unwrap();
    }
    assert_eq!(
        execution
            .resume(&mut meter, &mut AcceptTerminalEffects)
            .unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
    assert_eq!(meter.usage().total_units(), 7);
}

#[test]
fn ordered_nominal_affine_cleanups_repeat_a_shared_executable_target_and_helper() {
    let module = ordered_shared_executable_nominal_affine_module();
    let semantic = encode_module(&module).expect("shared executable nominal cleanup encodes");
    let proof =
        encode_proof_section(&module, &ProofBundle::default()).expect("empty proof encodes");
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
    let mut execution = TerminalExecution::start_artifact(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs {
            arguments: &arguments,
            ..Default::default()
        },
    )
    .expect("shared executable nominal cleanup should start");
    let mut meter = TerminalFuelMeter::with_allowance(7);
    assert_eq!(
        execution
            .resume(&mut meter, &mut AcceptTerminalEffects)
            .unwrap(),
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
    let proof =
        encode_proof_section(&module, &ProofBundle::default()).expect("empty proof encodes");
    let arguments = (93..96)
        .map(|opaque_identity| TerminalStructuralValue {
            opaque_identity,
            structural_type: structural_type_id(1),
            qualifications: Vec::new(),
            path: Vec::new(),
        })
        .collect::<Vec<_>>();
    let mut execution = TerminalExecution::start_artifact(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs {
            arguments: &arguments,
            ..Default::default()
        },
    )
    .expect("three shared executable cleanups should start");
    let mut meter = TerminalFuelMeter::with_allowance(10);

    assert_eq!(
        execution
            .resume(&mut meter, &mut AcceptTerminalEffects)
            .unwrap(),
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
    let proof =
        encode_proof_section(&module, &ProofBundle::default()).expect("empty proof encodes");
    let argument = TerminalStructuralValue {
        opaque_identity: 71,
        structural_type: structural_type_id(1),
        qualifications: Vec::new(),
        path: Vec::new(),
    };
    let mut execution = TerminalExecution::start_artifact(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs {
            arguments: &[argument],
            ..Default::default()
        },
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
            execution
                .resume(&mut meter, &mut AcceptTerminalEffects)
                .unwrap(),
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
        execution
            .resume(&mut meter, &mut AcceptTerminalEffects)
            .unwrap(),
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
    let proof =
        encode_proof_section(&module, &ProofBundle::default()).expect("empty proof encodes");
    let argument = TerminalStructuralValue {
        opaque_identity: 72,
        structural_type: structural_type_id(1),
        qualifications: Vec::new(),
        path: Vec::new(),
    };
    let mut execution = TerminalExecution::start_artifact(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs {
            arguments: &[argument],
            ..Default::default()
        },
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
            execution
                .resume(&mut meter, &mut AcceptTerminalEffects)
                .unwrap(),
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
        execution
            .resume(&mut meter, &mut AcceptTerminalEffects)
            .unwrap(),
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
    let proof =
        encode_proof_section(&module, &ProofBundle::default()).expect("empty proof encodes");
    let argument = TerminalStructuralValue {
        opaque_identity: 73,
        structural_type: structural_type_id(1),
        qualifications: Vec::new(),
        path: Vec::new(),
    };
    let mut execution = TerminalExecution::start_artifact(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs {
            arguments: &[argument],
            ..Default::default()
        },
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
            execution.resume(&mut meter, &mut AcceptTerminalEffects).unwrap(),
            TerminalExecutionStatus::SponsorExhausted(FuelExhaustion {
                site: exhausted_site,
                ..
            }) if exhausted_site == site
        ));
        assert_eq!(meter.usage().total_units(), consumed as u64);
        meter.replenish(1).unwrap();
    }
    assert_eq!(
        execution
            .resume(&mut meter, &mut AcceptTerminalEffects)
            .unwrap(),
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
        qualifications: Default::default(),
        id: value_id(10),
        scalar_type: ScalarType::Boolean,
    }];
    machine.result = TerminalMachineResult::Scalar(ValueDeclaration {
        qualifications: Default::default(),
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
    let proof =
        encode_proof_section(&module, &ProofBundle::default()).expect("empty proof encodes");
    let mut execution = TerminalExecution::start_artifact(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[TerminalScalarValue::Boolean(true)],
        TerminalStructuralInputs {
            arguments: &[structural_value(49)],
            ..Default::default()
        },
    )
    .expect("verified scalar affine cleanup should start");
    let mut meter = TerminalFuelMeter::with_allowance(0);

    assert!(matches!(
        execution
            .resume(&mut meter, &mut AcceptTerminalEffects)
            .unwrap(),
        TerminalExecutionStatus::SponsorExhausted(_)
    ));
    meter.replenish(1).unwrap();
    assert_eq!(
        execution
            .resume(&mut meter, &mut AcceptTerminalEffects)
            .unwrap(),
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
        qualifications: Default::default(),
        id: value_id(10),
        scalar_type: ScalarType::Boolean,
    }];
    machine.result = TerminalMachineResult::Scalar(ValueDeclaration {
        qualifications: Default::default(),
        id: value_id(11),
        scalar_type: ScalarType::Boolean,
    });
    machine.blocks = vec![
        Block {
            erased_scalar_formals: Vec::new(),
            structural_parameters: Vec::new(),
            id: block_id(2),
            parameters: Vec::new(),
            operations: Vec::new(),
            terminator: Terminator::Jump {
                structural_arguments: Vec::new(),
                edge: edge_id(2),
                target: block_id(3),
                arguments: vec![value_id(10)],
                erased_arguments: Vec::new(),
                residual_affine_discards: Vec::new(),
                trivial_affine_discards: vec![place_id(2)],
            },
        },
        Block {
            erased_scalar_formals: Vec::new(),
            structural_parameters: Vec::new(),
            id: block_id(3),
            parameters: vec![ValueDeclaration {
                qualifications: Default::default(),
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
    let proof =
        encode_proof_section(&module, &ProofBundle::default()).expect("empty proof encodes");
    let mut execution = TerminalExecution::start_artifact(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[TerminalScalarValue::Boolean(true)],
        TerminalStructuralInputs {
            arguments: &[structural_value(50)],
            ..Default::default()
        },
    )
    .expect("verified jump affine cleanup should start");
    let mut meter = TerminalFuelMeter::with_allowance(0);

    assert!(matches!(
        execution
            .resume(&mut meter, &mut AcceptTerminalEffects)
            .unwrap(),
        TerminalExecutionStatus::SponsorExhausted(_)
    ));
    meter.replenish(1).unwrap();
    assert!(matches!(
        execution
            .resume(&mut meter, &mut AcceptTerminalEffects)
            .unwrap(),
        TerminalExecutionStatus::SponsorExhausted(_)
    ));
    meter.replenish(1).unwrap();
    assert_eq!(
        execution
            .resume(&mut meter, &mut AcceptTerminalEffects)
            .unwrap(),
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
        qualifications: Default::default(),
        id: value_id(10),
        scalar_type: ScalarType::Boolean,
    }];
    machine.result = TerminalMachineResult::Scalar(ValueDeclaration {
        qualifications: Default::default(),
        id: value_id(11),
        scalar_type: ScalarType::Boolean,
    });
    machine.blocks = vec![
        Block {
            erased_scalar_formals: Vec::new(),
            structural_parameters: Vec::new(),
            id: block_id(2),
            parameters: Vec::new(),
            operations: Vec::new(),
            terminator: Terminator::Conditional {
                condition: value_id(10),
                when_true: SuccessorEdge {
                    structural_arguments: Vec::new(),
                    edge: edge_id(2),
                    target: block_id(3),
                    arguments: vec![value_id(10)],
                    erased_arguments: Vec::new(),
                    trivial_affine_discards: vec![place_id(2)],
                },
                when_false: SuccessorEdge {
                    structural_arguments: Vec::new(),
                    edge: edge_id(3),
                    target: block_id(4),
                    arguments: vec![value_id(10)],
                    erased_arguments: Vec::new(),
                    trivial_affine_discards: Vec::new(),
                },
            },
        },
        Block {
            erased_scalar_formals: Vec::new(),
            structural_parameters: Vec::new(),
            id: block_id(3),
            parameters: vec![ValueDeclaration {
                qualifications: Default::default(),
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
            erased_scalar_formals: Vec::new(),
            structural_parameters: Vec::new(),
            id: block_id(4),
            parameters: vec![ValueDeclaration {
                qualifications: Default::default(),
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
    let proof =
        encode_proof_section(&module, &ProofBundle::default()).expect("empty proof encodes");

    for condition in [true, false] {
        let mut execution = TerminalExecution::start_artifact(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            &[TerminalScalarValue::Boolean(condition)],
            TerminalStructuralInputs {
                arguments: &[structural_value(51)],
                ..Default::default()
            },
        )
        .expect("verified conditional affine cleanup should start");
        let mut meter = TerminalFuelMeter::with_allowance(0);
        assert!(matches!(
            execution
                .resume(&mut meter, &mut AcceptTerminalEffects)
                .unwrap(),
            TerminalExecutionStatus::SponsorExhausted(_)
        ));
        meter.replenish(1).unwrap();
        assert!(matches!(
            execution
                .resume(&mut meter, &mut AcceptTerminalEffects)
                .unwrap(),
            TerminalExecutionStatus::SponsorExhausted(_)
        ));
        meter.replenish(1).unwrap();
        assert_eq!(
            execution
                .resume(&mut meter, &mut AcceptTerminalEffects)
                .unwrap(),
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
    let measured = interpret_terminal_artifact_measured(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs {
            arguments: std::slice::from_ref(&argument),
            ..Default::default()
        },
        &mut handler,
    )
    .expect("verified Unit/effect artifact should execute");

    let expected = vec![
        TerminalEffect::BoundaryCall {
            operation: operation_id(3),
            result: terminal_psi::BoundaryMachineResult::Unit,
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
    let proof =
        encode_proof_section(&module, &ProofBundle::default()).expect("empty proof encodes");
    assert_eq!(decode_module(&semantic), Ok(module));
    let mut handler = RecordingHandler::default();

    let measured = interpret_terminal_artifact_measured(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs::default(),
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
    let proof =
        encode_proof_section(&module, &ProofBundle::default()).expect("empty proof encodes");
    let mut handler = RecordingHandler::default();
    let measured = interpret_terminal_artifact_measured(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs::default(),
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
        result: terminal_psi::BoundaryMachineResult::Unit,
    };
    assert_eq!(handler.effects, std::slice::from_ref(&expected));
    assert_eq!(measured.effects(), &[expected]);
    assert_eq!(measured.value(), TerminalExecutionResult::Unit);
}

#[test]
fn boundary_scalar_argument_effect_rejection_is_fail_closed() {
    let module = scalar_boundary_effect_module();
    let semantic = encode_module(&module).expect("scalar boundary module encodes");
    let proof =
        encode_proof_section(&module, &ProofBundle::default()).expect("empty proof encodes");
    let mut execution = TerminalExecution::start_artifact(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs::default(),
    )
    .expect("verified scalar boundary call starts");
    let mut meter = TerminalFuelMeter::unbounded();
    let mut handler = RejectScalarBoundaryArguments;

    assert!(matches!(
        execution.resume(&mut meter, &mut handler),
        Err(TerminalInterpretError::EffectRejected { operation, .. })
            if operation == operation_id(3)
    ));
    assert!(execution.effects().is_empty());
}

#[test]
fn unsupported_structural_boundary_result_rejects_before_the_effect() {
    let module = structural_boundary_effect_module();
    let semantic = encode_module(&module).expect("structural boundary module encodes");
    let proof =
        encode_proof_section(&module, &ProofBundle::default()).expect("empty proof encodes");
    let mut execution = TerminalExecution::start_artifact(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs::default(),
    )
    .expect("verified structural boundary call starts");
    let mut meter = TerminalFuelMeter::unbounded();
    let mut handler = RecordingHandler::default();

    assert!(matches!(
        execution.resume(&mut meter, &mut handler),
        Err(TerminalInterpretError::EffectRejected { operation, .. })
            if operation == operation_id(3)
    ));
    assert!(handler.effects.is_empty());
    assert!(execution.effects().is_empty());
}

#[test]
fn structural_boundary_result_establishes_affine_custody_once_before_cleanup() {
    let module = structural_boundary_effect_module();
    let semantic = encode_module(&module).expect("structural result module encodes");
    let proof =
        encode_proof_section(&module, &ProofBundle::default()).expect("empty proof encodes");
    let mut execution = TerminalExecution::start_artifact(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs::default(),
    )
    .expect("verified structural result starts");
    let mut handler = BoundaryResultHandler {
        result: Ok(TerminalEffectResult::Structural(TerminalStructuralValue {
            opaque_identity: 71,
            structural_type: structural_type_id(1),
            qualifications: Vec::new(),
            path: Vec::new(),
        })),
        requests: 0,
        effects: Vec::new(),
    };
    let mut meter = TerminalFuelMeter::with_allowance(3);
    assert!(matches!(
        execution.resume(&mut meter, &mut handler).unwrap(),
        TerminalExecutionStatus::SponsorExhausted(_)
    ));
    assert_eq!(handler.requests, 1);
    assert_eq!(execution.effects(), handler.effects);
    assert_eq!(execution.effects().len(), 1);
    assert_eq!(
        execution
            .live_affine_frontier()
            .cloned()
            .collect::<Vec<_>>(),
        vec![StructuralAffineDiscard {
            place: place_id(1),
            path: Vec::new(),
            structural_type: structural_type_id(1),
        }]
    );
    assert!(execution.live_claim_frontier().next().is_none());
    meter.replenish(1).unwrap();
    assert_eq!(
        execution.resume(&mut meter, &mut handler).unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
    assert!(execution.live_affine_frontier().next().is_none());
    assert_eq!(
        handler.requests, 1,
        "cleanup does not replay the producer effect"
    );
    assert_eq!(execution.effects(), handler.effects);
}

#[test]
fn malformed_structural_boundary_results_establish_no_runtime_custody() {
    let valid = TerminalStructuralValue {
        opaque_identity: 72,
        structural_type: structural_type_id(1),
        qualifications: Vec::new(),
        path: Vec::new(),
    };
    let mut wrong_type = valid.clone();
    wrong_type.structural_type = structural_type_id(2);
    let mut wrong_qualification = valid.clone();
    wrong_qualification
        .qualifications
        .push(structural_domain_id(1));
    let mut wrong_path = valid;
    wrong_path
        .path
        .push(StructuralPathSegment::Field("unexpected".into()));
    for result in [
        TerminalEffectResult::Unit,
        TerminalEffectResult::Scalar(TerminalScalarValue::Boolean(false)),
        TerminalEffectResult::Structural(wrong_type),
        TerminalEffectResult::Structural(wrong_qualification),
        TerminalEffectResult::Structural(wrong_path),
    ] {
        let module = structural_boundary_effect_module();
        let semantic = encode_module(&module).expect("structural result module encodes");
        let proof =
            encode_proof_section(&module, &ProofBundle::default()).expect("empty proof encodes");
        let mut execution = TerminalExecution::start_artifact(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            &[],
            TerminalStructuralInputs::default(),
        )
        .expect("verified structural result starts");
        let mut handler = BoundaryResultHandler {
            result: Ok(result.clone()),
            requests: 0,
            effects: Vec::new(),
        };
        assert_eq!(
            execution.resume(&mut TerminalFuelMeter::unbounded(), &mut handler),
            Err(TerminalInterpretError::VerifiedOperationMalformed),
            "{result:?}"
        );
        assert_eq!(handler.requests, 1);
        assert!(
            execution.effects().is_empty(),
            "malformed response is not a committed effect"
        );
        assert!(execution.live_affine_frontier().next().is_none());
        assert!(execution.live_claim_frontier().next().is_none());
    }
}

#[test]
fn rejected_structural_boundary_result_establishes_no_value_or_effect() {
    let module = structural_boundary_effect_module();
    let semantic = encode_module(&module).expect("structural result module encodes");
    let proof =
        encode_proof_section(&module, &ProofBundle::default()).expect("empty proof encodes");
    let mut execution = TerminalExecution::start_artifact(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs::default(),
    )
    .expect("verified structural result starts");
    let mut handler = BoundaryResultHandler {
        result: Err(TerminalEffectRejection::new(
            "structural result rejected by host",
        )),
        requests: 0,
        effects: Vec::new(),
    };
    assert!(matches!(
        execution.resume(&mut TerminalFuelMeter::unbounded(), &mut handler),
        Err(TerminalInterpretError::EffectRejected { operation, .. })
            if operation == operation_id(3)
    ));
    assert_eq!(handler.requests, 1);
    assert!(handler.effects.is_empty());
    assert!(execution.effects().is_empty());
    assert!(execution.live_affine_frontier().next().is_none());
    assert!(execution.live_claim_frontier().next().is_none());
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
        id: semantic_vocabulary::StructuralFieldId::new(1).expect("field identity"),
        identity: "#7".into(),
        relevance: BindingRelevance::Relevant,
        field_type: StructuralFieldType::Structural(structural_type_id(2)),
    });
    for machine in &mut module.machines {
        machine.entry_claims[0].path = vec!["#7".into()];
    }
    let semantic = encode_module(&module).expect("numbered field-custody module encodes");
    let proof =
        encode_proof_section(&module, &ProofBundle::default()).expect("empty proof encodes");
    let mut execution = TerminalExecution::start_artifact(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs {
            arguments: &[structural_value(45)],
            ..Default::default()
        },
    )
    .expect("verified numbered field custody should start");
    let mut meter = TerminalFuelMeter::unbounded();
    assert_eq!(
        execution
            .resume(&mut meter, &mut AcceptTerminalEffects)
            .unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
}
