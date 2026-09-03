//! Proof-bearing scalar cleanup projection and optimizer-context custody.

use omega_abstract_operations::AbstractOperation;
use omega_optimization_unit::{
    OwnershipFrontierOwnedPlace, OwnershipFrontierSite, ProofQuestionOwner,
};
use omega_psi_to_abstract_operations::{
    ArtifactLoweringError, build_verified_psi_optimization_unit, lower_artifact_sections,
    lower_artifact_sections_for_optimization,
};
use psi_core::{EvidenceIdentity, Proposition, ScalarTerm, ScalarType, StructuralPlaceKind};
use psi_proof_admission::{
    AdmissionProfile, CertificateEnvelope, EvidenceRoute, ProofNode, ProofRule, ProofSystemMarker,
};
use psi_terminal::{
    Block, MachineContract, NominalAffineCleanup, Operation, OperationKind, OperationResult,
    StructuralAccess, StructuralFieldDeclaration, StructuralFieldType, StructuralMultiplicity,
    StructuralParameterDeclaration, StructuralPlaceDeclaration, StructuralTypeDeclaration,
    StructuralTypeShape, TerminalAffineCleanupAction, TerminalMachine, TerminalMachineResult,
    TerminalModule, Terminator, ValueDeclaration, VocabularyMarker,
};
use psi_terminal_codec::{encode_module, encode_proof_bundle, proof_bundle_fingerprint};
use psi_terminal_verifier::{ObligationEvidence, ProofBundle};

use super::support::{
    block_id, contract_id, edge_id, machine_id, obligation_id, place_id, structural_type_id,
    value_id,
};

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
    let optimizer_input = lower_artifact_sections_for_optimization(
        &semantics,
        &proof_bytes,
        &AdmissionProfile::default(),
    )
    .expect("verified contextual scalar cleanup retains optimizer context");
    assert_eq!(optimizer_input.plan(), &plan);
    let optimizer_context = optimizer_input.context();
    assert_eq!(optimizer_context.module(), &module);
    assert_eq!(optimizer_context.proof_bundle(), &proof);
    assert_eq!(
        optimizer_context.proof_bundle_fingerprint(),
        proof_bundle_fingerprint(&proof).expect("canonical proof fingerprint")
    );
    assert_eq!(
        optimizer_context
            .reconstructed_obligations()
            .obligations()
            .iter()
            .map(|row| row.obligation.id)
            .collect::<Vec<_>>(),
        vec![obligation_id(1)]
    );
    let caller_frontiers = optimizer_context
        .structural_frontiers()
        .machine(caller.id)
        .expect("optimizer context retains caller ownership frontiers");
    assert!(caller_frontiers.block_entry(caller.entry).is_some());
    assert!(
        caller_frontiers
            .edge_entry(caller.blocks[0].terminator.edge())
            .is_some()
    );
    assert_eq!(
        optimizer_context
            .accepted_facts()
            .iter()
            .map(|fact| fact.obligation)
            .collect::<Vec<_>>(),
        vec![obligation_id(1)]
    );
    let verified_unit = build_verified_psi_optimization_unit(
        optimizer_input,
        psi_terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
    )
    .expect("verified optimizer unit reconstruction");
    assert_eq!(verified_unit.unit().psi, plan.psi);
    let [proof_question] = verified_unit.unit().proof_questions.as_slice() else {
        panic!("complete cleanup proof question is retained exactly once")
    };
    assert_eq!(
        proof_question.owner,
        ProofQuestionOwner::NominalCleanupRequires {
            machine: caller.id,
            edge: edge_id(1),
            cleanup_position: 1,
            requirement_position: 0,
        }
    );
    assert_eq!(proof_question.obligation, obligation_id(1));
    assert_eq!(
        proof_question.proof_bundle_fingerprint,
        *verified_unit
            .input()
            .context()
            .proof_bundle_fingerprint()
            .as_bytes()
    );
    let reconstructed = &verified_unit
        .input()
        .context()
        .reconstructed_obligations()
        .obligations()[0];
    assert_eq!(
        proof_question.requirements,
        reconstructed
            .requirements
            .iter()
            .map(psi_terminal_codec::canonical_proposition_order_key)
            .collect::<Result<Vec<_>, _>>()
            .expect("canonical retained requirements")
    );
    assert_eq!(
        proof_question.semantic_axioms,
        reconstructed
            .semantic_axioms
            .iter()
            .map(psi_terminal_codec::canonical_proposition_order_key)
            .collect::<Result<Vec<_>, _>>()
            .expect("canonical retained semantic axioms")
    );
    assert!(!proof_question.canonical_certificate);
    let unit_frontiers = &verified_unit.unit().ownership_frontier_facts;
    assert!(!unit_frontiers.is_empty());
    assert!(
        unit_frontiers
            .windows(2)
            .all(|pair| { (pair[0].machine, pair[0].site) < (pair[1].machine, pair[1].site) })
    );
    let caller_fact = |site| {
        unit_frontiers
            .iter()
            .find(|fact| fact.machine == caller.id && fact.site == site)
            .expect("verified caller frontier is projected into the unit")
    };
    assert_eq!(
        caller_fact(OwnershipFrontierSite::BlockEntry(block_id(1)))
            .snapshot
            .owned_places,
        vec![
            OwnershipFrontierOwnedPlace {
                place: place_id(1),
                multiplicity: StructuralMultiplicity::Affine,
            },
            OwnershipFrontierOwnedPlace {
                place: place_id(2),
                multiplicity: StructuralMultiplicity::Affine,
            },
        ]
    );
    caller_fact(OwnershipFrontierSite::OperationEntry(
        psi_core::OperationId::new(1).unwrap(),
    ));
    caller_fact(OwnershipFrontierSite::OperationExit(
        psi_core::OperationId::new(1).unwrap(),
    ));
    caller_fact(OwnershipFrontierSite::EdgeEntry(edge_id(1)));
    assert!(!unit_frontiers.iter().any(|fact| {
        fact.machine == caller.id && fact.site == OwnershipFrontierSite::EdgeExit(edge_id(1))
    }));
    assert_eq!(
        verified_unit
            .input()
            .context()
            .accepted_facts()
            .first()
            .map(|fact| fact.obligation),
        Some(obligation_id(1))
    );
    let lowered_caller = plan
        .functions
        .iter()
        .find(|function| function.machine == caller.id)
        .expect("scalar caller remains in the verified closure");
    let [
        AbstractOperation::BooleanConstant { .. },
        AbstractOperation::Return {
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
        placed_view_inputs: Vec::new(),
        reborrow_root_handoffs: Vec::new(),
        reborrow_restored_call_uses: Vec::new(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        float_meaning_projections: Vec::new(),
        float_meaning_equalities: Vec::new(),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        evidence_terms: Vec::new(),
        proof_output_calls: Vec::new(),
        proof_recursive_components: Vec::new(),
        evidence_contract_lanes: Vec::new(),
        closed_conformance_applications: Vec::new(),
        dynamic_dispatch: Default::default(),
        suspension_call_plan_count: 0,
        suspension_call_sites: Vec::new(),
        suspension_call_plans: Vec::new(),
        quotient_correspondences: Vec::new(),
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
                        projected_qualifications: Vec::new(),
                    },
                    StructuralParameterDeclaration {
                        access: StructuralAccess::Owned,
                        place: no_code_place,
                        position: 1,
                        is_self: false,
                        structural_type: no_code_type,
                        multiplicity: StructuralMultiplicity::Affine,
                        qualifications: Vec::new(),
                        projected_qualifications: Vec::new(),
                    },
                ],
                ranked_scc: None,
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
                    outcome_specific_ensures: Vec::new(),
                },
            },
            TerminalMachine {
                id: machine_id(2),
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
                    outcome_specific_ensures: Vec::new(),
                },
            },
        ],
    };
    let proof = ProofBundle {
        recursive_components: Vec::new(),
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
