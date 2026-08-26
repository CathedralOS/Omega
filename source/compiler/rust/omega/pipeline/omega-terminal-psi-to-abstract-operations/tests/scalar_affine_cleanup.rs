use omega_terminal_abstract_operations::{
    TerminalAbstractFunctionResult, TerminalAbstractOperation,
};
use omega_terminal_psi_to_abstract_operations::{
    ArtifactLoweringError, LoweringError, lower_artifact_sections,
};
use psi_core::{
    BlockId, ClaimId, ContractId, EdgeId, EvidenceIdentity, MachineId, ObligationId, PlaceId,
    Proposition, ScalarTerm, ScalarType, StructuralDomainId, StructuralPlaceKind, StructuralTypeId,
    ValueId,
};
use psi_proof_admission::{
    AdmissionProfile, CertificateEnvelope, EvidenceRoute, ProofNode, ProofRule, ProofSystemMarker,
};
use psi_terminal::{
    Block, CrashCause, CrashRouteBucket, CrashRouteGuard, EntryClaim, MachineContract,
    NominalAffineCleanup, Operation, OperationKind, OperationResult, StructuralAccess,
    StructuralDomainDeclaration, StructuralFieldDeclaration, StructuralFieldType,
    StructuralMultiplicity, StructuralParameterDeclaration, StructuralPlaceDeclaration,
    StructuralResultDeclaration, StructuralTypeDeclaration, StructuralTypeShape,
    TerminalAffineCleanupAction, TerminalMachine, TerminalMachineResult, TerminalModule,
    Terminator, ValueDeclaration, VocabularyMarker,
};
use psi_terminal_codec::{encode_module, encode_proof_bundle};
use psi_terminal_verifier::{ObligationEvidence, ProofBundle};

#[test]
fn omega_consumes_verified_jump_affine_cleanup_without_emitting_an_operation() {
    let place = place_id(1);
    let module = TerminalModule {
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: machine_id(1),
        structural_types: vec![StructuralTypeDeclaration {
            id: structural_type_id(1),
            identity: "test::AffineToken".into(),
            shape: StructuralTypeShape::Record { fields: Vec::new() },
        }],
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
        proof_output_calls: Vec::new(),
        evidence_contract_lanes: Vec::new(),
        closed_conformance_applications: Vec::new(),
        machines: vec![TerminalMachine {
            id: machine_id(1),
            attachment: None,
            parameters: vec![ValueDeclaration {
                id: value_id(1),
                scalar_type: ScalarType::Boolean,
            }],
            structural_parameters: vec![StructuralParameterDeclaration {
                access: StructuralAccess::Owned,
                place,
                position: 0,
                is_self: false,
                structural_type: structural_type_id(1),
                multiplicity: StructuralMultiplicity::Affine,
                qualifications: Vec::new(),
            }],
            result: TerminalMachineResult::Scalar(ValueDeclaration {
                id: value_id(2),
                scalar_type: ScalarType::Boolean,
            }),
            structural_places: vec![StructuralPlaceDeclaration {
                id: place,
                kind: StructuralPlaceKind::Parameter {
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
            blocks: vec![
                Block {
                    id: block_id(1),
                    parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: Terminator::Jump {
                        edge: edge_id(1),
                        target: block_id(2),
                        arguments: vec![value_id(1)],
                        trivial_affine_discards: vec![place],
                    },
                },
                Block {
                    id: block_id(2),
                    parameters: vec![ValueDeclaration {
                        id: value_id(3),
                        scalar_type: ScalarType::Boolean,
                    }],
                    operations: Vec::new(),
                    terminator: Terminator::Return {
                        edge: edge_id(2),
                        value: value_id(3),
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
        }],
    };
    let semantics = encode_module(&module).expect("exact jump affine cleanup should encode");
    let proof = encode_proof_bundle(&ProofBundle::default()).expect("empty proof should encode");

    let plan = lower_artifact_sections(&semantics, &proof, &AdmissionProfile::default())
        .expect("verified jump affine cleanup should lower through Omega");
    let [function] = plan.functions.as_slice() else {
        panic!("fixture has one terminal function")
    };
    let [
        TerminalAbstractOperation::Jump {
            psi_edge: jump_edge,
            target,
            bindings,
        },
        TerminalAbstractOperation::Return {
            psi_edge,
            result,
            value,
            scalar_type,
            ..
        },
    ] = function.operations.as_slice()
    else {
        panic!("no-code cleanup must not add an abstract operation")
    };
    assert_eq!(*jump_edge, edge_id(1));
    assert_eq!(*target, block_id(2));
    assert_eq!(bindings.len(), 1);
    assert_eq!(bindings[0].parameter, value_id(3));
    assert_eq!(bindings[0].argument, value_id(1));
    assert_eq!(*psi_edge, edge_id(2));
    assert_eq!(*result, value_id(2));
    assert_eq!(*value, value_id(3));
    assert_eq!(*scalar_type, ScalarType::Boolean);
}

#[test]
fn omega_projects_verified_scalar_cleanup_proofs_without_regrouping_actions() {
    let (module, proof) = contextual_mixed_scalar_cleanup_module();
    let caller = &module.machines[0];
    let Terminator::Return {
        cleanup_actions, ..
    } = &caller.blocks[0].terminator
    else {
        panic!("contextual scalar fixture returns a value")
    };
    let [
        TerminalAffineCleanupAction::DiscardRoot(no_code_place),
        TerminalAffineCleanupAction::InvokeNominal(verified_cleanup),
    ] = cleanup_actions.as_slice()
    else {
        panic!("fixture retains one mixed ordered cleanup stream")
    };
    assert_eq!(*no_code_place, place_id(2));
    assert_eq!(verified_cleanup.cleanup_receiver, Some(place_id(99)));
    assert_eq!(verified_cleanup.requirement_obligations, [obligation_id(1)]);

    let semantics = encode_module(&module).expect("contextual scalar cleanup encodes");
    let proof_bytes = encode_proof_bundle(&proof).expect("contextual scalar proof encodes");
    assert!(matches!(
        lower_artifact_sections(
            &semantics,
            &encode_proof_bundle(&ProofBundle::default()).expect("empty proof encodes"),
            &AdmissionProfile::default(),
        ),
        Err(ArtifactLoweringError::Verification(
            psi_terminal_verifier::VerificationError::MissingEvidence(obligation)
        )) if obligation == obligation_id(1)
    ));

    let plan = lower_artifact_sections(&semantics, &proof_bytes, &AdmissionProfile::default())
        .expect("verified contextual scalar cleanup enters Omega");
    let lowered_caller = plan
        .functions
        .iter()
        .find(|function| function.machine == caller.id)
        .expect("scalar caller remains in the verified closure");
    let [
        TerminalAbstractOperation::BooleanConstant { .. },
        TerminalAbstractOperation::Return {
            cleanup_actions, ..
        },
    ] = lowered_caller.operations.as_slice()
    else {
        panic!("scalar caller retains its constant and return")
    };
    let [
        TerminalAffineCleanupAction::DiscardRoot(projected_no_code),
        TerminalAffineCleanupAction::InvokeNominal(projected_cleanup),
    ] = cleanup_actions.as_slice()
    else {
        panic!("Omega retains the exact mixed action order")
    };
    assert_eq!(*projected_no_code, *no_code_place);
    assert_eq!(projected_cleanup.place, verified_cleanup.place);
    assert_eq!(
        projected_cleanup.structural_type,
        verified_cleanup.structural_type
    );
    assert_eq!(
        projected_cleanup.cleanup_machine,
        verified_cleanup.cleanup_machine
    );
    assert!(projected_cleanup.cleanup_receiver.is_none());
    assert!(projected_cleanup.requirement_obligations.is_empty());
}

#[test]
fn omega_preserves_exact_singleton_structural_return_custody() {
    let source = place_id(1);
    let result_place = place_id(2);
    let claim = claim_id(1);
    let structural_type = structural_type_id(1);
    let structural_domain = structural_domain_id(1);
    let edge = edge_id(1);
    let module = TerminalModule {
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: machine_id(1),
        structural_types: vec![StructuralTypeDeclaration {
            id: structural_type,
            identity: "test::LinearToken".into(),
            shape: StructuralTypeShape::Record { fields: Vec::new() },
        }],
        structural_domains: vec![StructuralDomainDeclaration {
            id: structural_domain,
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
        proof_output_calls: Vec::new(),
        evidence_contract_lanes: Vec::new(),
        closed_conformance_applications: Vec::new(),
        machines: vec![TerminalMachine {
            id: machine_id(1),
            attachment: None,
            parameters: Vec::new(),
            structural_parameters: vec![StructuralParameterDeclaration {
                access: StructuralAccess::Owned,
                place: source,
                position: 0,
                is_self: false,
                structural_type,
                multiplicity: StructuralMultiplicity::Linear,
                qualifications: vec![structural_domain],
            }],
            result: TerminalMachineResult::Structural(StructuralResultDeclaration {
                place: result_place,
                structural_type,
                multiplicity: StructuralMultiplicity::Linear,
                qualifications: vec![structural_domain],
            }),
            structural_places: vec![
                StructuralPlaceDeclaration {
                    id: source,
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
            contract: MachineContract {
                id: contract_id(1),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
            },
        }],
    };
    let semantics = encode_module(&module).expect("structural return should encode");
    let proof = encode_proof_bundle(&ProofBundle::default()).expect("empty proof should encode");

    let plan = lower_artifact_sections(&semantics, &proof, &AdmissionProfile::default())
        .expect("exact structural custody return should enter Omega");
    let [function] = plan.functions.as_slice() else {
        panic!("fixture has one terminal function")
    };
    assert_eq!(
        function.structural_parameters,
        module.machines[0].structural_parameters
    );
    assert_eq!(function.entry_claims, module.machines[0].entry_claims);
    assert_eq!(
        function.result,
        TerminalAbstractFunctionResult::Structural(StructuralResultDeclaration {
            place: result_place,
            structural_type,
            multiplicity: StructuralMultiplicity::Linear,
            qualifications: vec![structural_domain],
        })
    );
    assert_eq!(
        function
            .result
            .structural()
            .expect("structural result")
            .place,
        result_place
    );
    assert!(matches!(
        function.operations.as_slice(),
        [TerminalAbstractOperation::ReturnStructural {
            psi_edge,
            source: actual_source,
            returned_claims,
            trivial_affine_discards,
            ..
        }] if *psi_edge == edge
            && *actual_source == source
            && returned_claims.as_slice() == [claim]
            && trivial_affine_discards.is_empty()
    ));

    let mut crash_only = module.clone();
    crash_only.machines[0].contract.crash_routes = vec![CrashRouteBucket {
        cause: CrashCause::Abort,
        alternatives: vec![CrashRouteGuard::Truth],
    }];
    crash_only.machines[0].blocks[0].terminator = Terminator::Crash {
        edge,
        cause: CrashCause::Abort,
        site_guard: Vec::new(),
        frontier_lower_bound: vec![claim],
    };
    let semantics = encode_module(&crash_only).expect("structural crash-only machine encodes");
    assert!(matches!(
        lower_artifact_sections(&semantics, &proof, &AdmissionProfile::default()),
        Err(ArtifactLoweringError::Lowering(
            LoweringError::UnsupportedStructuralResult(machine)
        )) if machine == machine_id(1)
    ));

    let extra = place_id(3);
    let mut wider_cleanup = module;
    wider_cleanup.machines[0]
        .structural_parameters
        .push(StructuralParameterDeclaration {
            access: StructuralAccess::Owned,
            place: extra,
            position: 1,
            is_self: false,
            structural_type,
            multiplicity: StructuralMultiplicity::Affine,
            qualifications: Vec::new(),
        });
    wider_cleanup.machines[0]
        .structural_places
        .push(StructuralPlaceDeclaration {
            id: extra,
            kind: StructuralPlaceKind::Parameter {
                position: 1,
                is_self: false,
            },
        });
    let Terminator::ReturnStructural {
        trivial_affine_discards,
        ..
    } = &mut wider_cleanup.machines[0].blocks[0].terminator
    else {
        unreachable!()
    };
    trivial_affine_discards.push(extra);
    let semantics = encode_module(&wider_cleanup).expect("wider cleanup return should encode");
    let plan = lower_artifact_sections(&semantics, &proof, &AdmissionProfile::default())
        .expect("one exact affine cleanup should enter Omega abstract operations");
    let [function] = plan.functions.as_slice() else {
        panic!("fixture has one terminal function")
    };
    assert_eq!(function.structural_parameters.len(), 2);
    assert!(matches!(
        function.operations.as_slice(),
        [TerminalAbstractOperation::ReturnStructural {
            trivial_affine_discards,
            ..
        }] if trivial_affine_discards == &[extra]
    ));

    let second_extra = place_id(4);
    wider_cleanup.machines[0]
        .structural_parameters
        .push(StructuralParameterDeclaration {
            access: StructuralAccess::Owned,
            place: second_extra,
            position: 2,
            is_self: false,
            structural_type,
            multiplicity: StructuralMultiplicity::Affine,
            qualifications: Vec::new(),
        });
    wider_cleanup.machines[0]
        .structural_places
        .push(StructuralPlaceDeclaration {
            id: second_extra,
            kind: StructuralPlaceKind::Parameter {
                position: 2,
                is_self: false,
            },
        });
    let Terminator::ReturnStructural {
        trivial_affine_discards,
        ..
    } = &mut wider_cleanup.machines[0].blocks[0].terminator
    else {
        unreachable!()
    };
    *trivial_affine_discards = vec![second_extra, extra];
    let semantics = encode_module(&wider_cleanup).expect("two affine cleanups should encode");
    let plan = lower_artifact_sections(&semantics, &proof, &AdmissionProfile::default())
        .expect("a finite exact affine cleanup tail should enter Omega abstract operations");
    let [function] = plan.functions.as_slice() else {
        panic!("fixture has one terminal function")
    };
    assert_eq!(function.structural_parameters.len(), 3);
    assert!(matches!(
        function.operations.as_slice(),
        [TerminalAbstractOperation::ReturnStructural {
            trivial_affine_discards,
            ..
        }] if trivial_affine_discards == &[second_extra, extra]
    ));
}

fn contextual_mixed_scalar_cleanup_module() -> (TerminalModule, ProofBundle) {
    let token_type = structural_type_id(1);
    let no_code_type = structural_type_id(2);
    let field = psi_core::StructuralFieldId::new(1).expect("field");
    let caller_place = place_id(1);
    let no_code_place = place_id(2);
    let cleanup_receiver = place_id(99);
    let obligation = obligation_id(1);
    let caller_requirement = Proposition::Equal(
        ScalarTerm::boolean(true),
        ScalarTerm::boolean_field(caller_place, field),
    );
    let module = TerminalModule {
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: machine_id(1),
        structural_types: vec![
            StructuralTypeDeclaration {
                id: token_type,
                identity: "test::Token".into(),
                shape: StructuralTypeShape::Record {
                    fields: vec![StructuralFieldDeclaration {
                        id: field,
                        identity: "ready".into(),
                        relevance: psi_terminal::BindingRelevance::Relevant,
                        field_type: StructuralFieldType::Scalar(ScalarType::Boolean),
                    }],
                },
            },
            StructuralTypeDeclaration {
                id: no_code_type,
                identity: "test::NoCode".into(),
                shape: StructuralTypeShape::Record { fields: Vec::new() },
            },
        ],
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
        proof_output_calls: Vec::new(),
        evidence_contract_lanes: Vec::new(),
        closed_conformance_applications: Vec::new(),
        machines: vec![
            TerminalMachine {
                id: machine_id(1),
                attachment: None,
                parameters: Vec::new(),
                structural_parameters: vec![
                    StructuralParameterDeclaration {
                        access: StructuralAccess::Owned,
                        place: caller_place,
                        position: 0,
                        is_self: false,
                        structural_type: token_type,
                        multiplicity: StructuralMultiplicity::Affine,
                        qualifications: Vec::new(),
                    },
                    StructuralParameterDeclaration {
                        access: StructuralAccess::Owned,
                        place: no_code_place,
                        position: 1,
                        is_self: false,
                        structural_type: no_code_type,
                        multiplicity: StructuralMultiplicity::Affine,
                        qualifications: Vec::new(),
                    },
                ],
                result: TerminalMachineResult::Scalar(ValueDeclaration {
                    id: value_id(1),
                    scalar_type: ScalarType::Boolean,
                }),
                structural_places: vec![
                    StructuralPlaceDeclaration {
                        id: caller_place,
                        kind: StructuralPlaceKind::Parameter {
                            position: 0,
                            is_self: false,
                        },
                    },
                    StructuralPlaceDeclaration {
                        id: no_code_place,
                        kind: StructuralPlaceKind::Parameter {
                            position: 1,
                            is_self: false,
                        },
                    },
                ],
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
                        id: psi_core::OperationId::new(1).expect("operation"),
                        result: OperationResult::Scalar(ValueDeclaration {
                            id: value_id(2),
                            scalar_type: ScalarType::Boolean,
                        }),
                        kind: OperationKind::BooleanConstant { value: true },
                    }],
                    terminator: Terminator::Return {
                        edge: edge_id(1),
                        value: value_id(2),
                        cleanup_actions: vec![
                            TerminalAffineCleanupAction::DiscardRoot(no_code_place),
                            TerminalAffineCleanupAction::InvokeNominal(NominalAffineCleanup {
                                place: caller_place,
                                structural_type: token_type,
                                cleanup_machine: machine_id(2),
                                cleanup_receiver: Some(cleanup_receiver),
                                requirement_obligations: vec![obligation],
                            }),
                        ],
                    },
                }],
                contract: MachineContract {
                    id: contract_id(1),
                    crash_routes: Vec::new(),
                    requires: vec![caller_requirement.clone()],
                    ensures: Vec::new(),
                },
            },
            TerminalMachine {
                id: machine_id(2),
                attachment: Some(token_type),
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
                contract: MachineContract {
                    id: contract_id(2),
                    crash_routes: Vec::new(),
                    requires: vec![Proposition::Equal(
                        ScalarTerm::boolean(true),
                        ScalarTerm::boolean_field(cleanup_receiver, field),
                    )],
                    ensures: Vec::new(),
                },
            },
        ],
    };
    let proof = ProofBundle {
        evidence_producers: Vec::new(),
        evidence: vec![ObligationEvidence {
            obligation,
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: EvidenceIdentity::new(1).expect("certificate"),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof: ProofNode {
                    conclusion: caller_requirement,
                    rule: ProofRule::Assumption { index: 0 },
                },
            }),
        }],
    };
    (module, proof)
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

fn value_id(raw: u64) -> ValueId {
    ValueId::new(raw).unwrap()
}

fn place_id(raw: u64) -> PlaceId {
    PlaceId::new(raw).unwrap()
}

fn structural_type_id(raw: u64) -> StructuralTypeId {
    StructuralTypeId::new(raw).unwrap()
}

fn structural_domain_id(raw: u64) -> StructuralDomainId {
    StructuralDomainId::new(raw).unwrap()
}

fn claim_id(raw: u64) -> ClaimId {
    ClaimId::new(raw).unwrap()
}

fn obligation_id(raw: u64) -> ObligationId {
    ObligationId::new(raw).unwrap()
}
