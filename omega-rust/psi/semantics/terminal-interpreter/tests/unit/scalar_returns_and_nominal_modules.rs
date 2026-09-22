use super::{
    edge_id, nominal_affine_module, obligation_id, ordered_empty_nominal_affine_module, place_id,
    structural_type_id, three_ordered_empty_nominal_affine_module, value_id,
};
use proof_admission::{
    AdmissionProfile, CertificateEnvelope, EvidenceRoute, ProofNode, ProofRule, ProofSystemMarker,
};
use semantic_vocabulary::{EvidenceIdentity, Proposition, ScalarTerm, ScalarType};
use terminal_codec::{encode_module, encode_proof_section};
use terminal_fuel::{FuelChargeSite, FuelExhaustion, TerminalFuelMeter};
use terminal_interpreter::AcceptTerminalEffects;
use terminal_interpreter::TerminalStructuralInputs;
use terminal_interpreter::{
    TerminalExecution, TerminalExecutionResult, TerminalExecutionStatus, TerminalScalarValue,
    TerminalStructuralValue,
};
use terminal_psi::{
    BindingRelevance, StructuralFieldDeclaration, StructuralFieldType, StructuralTypeShape,
    TerminalAffineCleanupAction, TerminalMachineResult, Terminator, ValueDeclaration,
};
use terminal_verifier::{ObligationEvidence, ProofBundle};

#[test]
fn scalar_return_materializes_result_then_runs_nominal_cleanup() {
    let mut module = nominal_affine_module();
    let caller = &mut module.machines[0];
    caller.parameters = vec![ValueDeclaration {
        qualifications: Default::default(),
        id: value_id(10),
        scalar_type: ScalarType::Boolean,
    }];
    caller.result = TerminalMachineResult::Scalar(ValueDeclaration {
        qualifications: Default::default(),
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
    let proof =
        encode_proof_section(&module, &ProofBundle::default()).expect("empty proof encodes");
    let mut execution = TerminalExecution::start_artifact(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[TerminalScalarValue::Boolean(true)],
        TerminalStructuralInputs {
            arguments: &[TerminalStructuralValue {
                opaque_identity: 71,
                structural_type: structural_type_id(1),
                qualifications: Vec::new(),
                path: Vec::new(),
            }],
            ..Default::default()
        },
    )
    .expect("verified scalar nominal cleanup starts");
    let mut meter = TerminalFuelMeter::with_allowance(1);

    assert!(matches!(
        execution.resume(&mut meter, &mut AcceptTerminalEffects).unwrap(),
        TerminalExecutionStatus::SponsorExhausted(FuelExhaustion {
            site: FuelChargeSite::Edge(edge),
            ..
        }) if edge == edge_id(2)
    ));
    assert!(execution.live_affine_frontier().next().is_none());
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
fn contextual_scalar_return_materializes_then_executes_reverse_ordered_cleanups() {
    let mut module = ordered_empty_nominal_affine_module(true);
    let first = semantic_vocabulary::StructuralFieldId::new(1).expect("first field");
    let second = semantic_vocabulary::StructuralFieldId::new(2).expect("second field");
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
        qualifications: Default::default(),
        id: value_id(30),
        scalar_type: ScalarType::Boolean,
    }];
    caller.result = TerminalMachineResult::Scalar(ValueDeclaration {
        qualifications: Default::default(),
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
    let reconstructed = terminal_verifier::reconstruct_terminal_obligations(&module)
        .expect("contextual cleanup obligations reconstruct");
    let mut evidence = goals
        .into_iter()
        .enumerate()
        .map(|(index, (obligation, root, field))| {
            let conclusion = Proposition::Equal(
                ScalarTerm::boolean(true),
                ScalarTerm::boolean_field(root, field),
            );
            let site = reconstructed
                .obligations()
                .iter()
                .find(|site| site.obligation.id == obligation)
                .expect("exact cleanup obligation exists");
            assert_eq!(site.obligation.proposition, conclusion);
            assert_eq!(
                site.owner,
                terminal_verifier::ReconstructedTerminalObligationOwner::NominalCleanupRequires {
                    machine: module.machines[0].id,
                    edge,
                    cleanup_position: (index / 2) as u32,
                    requirement_position: (index % 2) as u32,
                }
            );
            assert!(
                !site.requirements.contains(&conclusion),
                "owned storage is not a permanent entry snapshot"
            );
            let axiom_index = site
                .semantic_axioms
                .iter()
                .position(|fact| fact == &conclusion)
                .expect("unchanged owned field observation reaches this cleanup");
            ObligationEvidence {
                obligation,
                route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                    identity: EvidenceIdentity::new(index as u64 + 1).expect("certificate"),
                    proof_system_marker: ProofSystemMarker::CURRENT,
                    proof: ProofNode {
                        rule: ProofRule::SemanticAxiom { index: axiom_index },
                        conclusion,
                    },
                }),
            }
        })
        .collect::<Vec<_>>();
    evidence.sort_by_key(|evidence| evidence.obligation);
    let proof_bundle = ProofBundle {
        crash_obligations: Vec::new(),
        recursive_components: Vec::new(),
        control_cycles: Vec::new(),
        evidence_producers: Vec::new(),
        evidence,
    };
    let semantic = encode_module(&module).expect("contextual scalar cleanup encodes");
    let proof =
        encode_proof_section(&module, &proof_bundle).expect("contextual cleanup proof encodes");
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
    let mut execution = TerminalExecution::start_artifact(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[TerminalScalarValue::Boolean(false)],
        TerminalStructuralInputs {
            arguments: &structural_arguments,
            ..Default::default()
        },
    )
    .expect("proof-carrying contextual scalar cleanup starts");
    let mut meter = TerminalFuelMeter::with_allowance(0);

    for expected_site in [
        FuelChargeSite::Edge(edge_id(1)),
        FuelChargeSite::Edge(edge_id(2)),
        FuelChargeSite::Edge(edge_id(2)),
    ] {
        assert!(matches!(
            execution.resume(&mut meter, &mut AcceptTerminalEffects).unwrap(),
            TerminalExecutionStatus::SponsorExhausted(FuelExhaustion { site, .. })
                if site == expected_site
        ));
        meter.replenish(1).unwrap();
    }
    assert_eq!(
        execution
            .resume(&mut meter, &mut AcceptTerminalEffects)
            .unwrap(),
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
        qualifications: Default::default(),
        id: value_id(20),
        scalar_type: ScalarType::Boolean,
    }];
    caller.result = TerminalMachineResult::Scalar(ValueDeclaration {
        qualifications: Default::default(),
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
    let proof =
        encode_proof_section(&module, &ProofBundle::default()).expect("empty proof encodes");
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
    let mut execution = TerminalExecution::start_artifact(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[TerminalScalarValue::Boolean(true)],
        TerminalStructuralInputs {
            arguments: &structural_arguments,
            ..Default::default()
        },
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
            execution.resume(&mut meter, &mut AcceptTerminalEffects).unwrap(),
            TerminalExecutionStatus::SponsorExhausted(FuelExhaustion { site, .. })
                if site == expected_site
        ));
        assert_eq!(meter.usage().total_units(), consumed as u64);
        meter.replenish(1).unwrap();
    }

    assert_eq!(
        execution
            .resume(&mut meter, &mut AcceptTerminalEffects)
            .unwrap(),
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
