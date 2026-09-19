//! Fixtures shared by the artifact codec tests: canonical artifacts,
//! recursive components and primitive types.

#[path = "artifact/artifact_envelope_custody.rs"]
mod artifact_envelope_custody;
#[path = "artifact/content_and_component_proof_formats.rs"]
mod content_and_component_proof_formats;
#[path = "artifact/control_cycles.rs"]
mod control_cycles;
#[path = "artifact/debug_map_custody.rs"]
mod debug_map_custody;
#[path = "artifact/float_meaning_custody.rs"]
mod float_meaning_custody;
#[path = "artifact/integer_proof_formats.rs"]
mod integer_proof_formats;
#[path = "artifact/obligation_ledger_custody.rs"]
mod obligation_ledger_custody;
#[path = "artifact/operation_crash_contract_custody.rs"]
mod operation_crash_contract_custody;
#[path = "artifact/optimization_execution_custody.rs"]
mod optimization_execution_custody;
#[path = "artifact/pcc.rs"]
mod pcc;
#[path = "artifact/pcc_custody.rs"]
mod pcc_custody;
#[path = "artifact/placed_view_input_custody.rs"]
mod placed_view_input_custody;
#[path = "artifact/proof_section.rs"]
mod proof_section;
#[path = "artifact/proof_section_custody.rs"]
mod proof_section_custody;
#[path = "artifact/proposition_vocabulary_custody.rs"]
mod proposition_vocabulary_custody;
#[path = "artifact/reborrow_restored_call_use_custody.rs"]
mod reborrow_restored_call_use_custody;
#[path = "artifact/reborrow_root_handoff_custody.rs"]
mod reborrow_root_handoff_custody;
#[path = "artifact/recursive_component_custody.rs"]
mod recursive_component_custody;
#[path = "artifact/trace_profile_custody.rs"]
mod trace_profile_custody;
#[path = "artifact/transport_round_trips.rs"]
mod transport_round_trips;

use proof_admission::{
    AdmissionEvidence, AdmissionKind, CertificateEnvelope, EvidenceRoute, PrimitiveJudgment,
    ProofNode, ProofRule, ProofSystemMarker, RecursiveComponentCertificate,
    RecursiveEdgeCertificate,
};
use semantic_vocabulary::{
    AdmissionSiteId, BlockId, ContractId, EdgeId, EvidenceIdentity, EvidenceTermId, IntegerSign,
    IntegerType, IntegerValue, MachineId, ObligationId, OperationId, PlaceId, ProfileDecisionId,
    Proposition, PropositionId, ScalarTerm, ScalarType, StructuralCaseId, StructuralFieldId,
    StructuralTypeId, ValueId,
};
use terminal_codec::{CanonicalTerminalArtifact, build_identity_optimization_execution_record};
use terminal_psi::{
    Block, ContractClause, MachineContract, Operation, OperationKind, TerminalMachine,
    TerminalMachineResult, TerminalModule, TerminalProofRankingRelation,
    TerminalProofRecursiveCallSite, TerminalProofRecursiveComponent, TerminalProofRecursiveEdge,
    TerminalProofRecursiveField, TerminalProofRecursiveMember, TerminalProofRecursiveType,
    Terminator, ValueDeclaration, VocabularyMarker,
};

fn canonical_artifact(
    module: &TerminalModule,
    proof: &ProofBundle,
    debug: Option<&terminal_codec::TerminalDebugMap>,
) -> CanonicalTerminalArtifact {
    let optimization = build_identity_optimization_execution_record(module, proof)
        .expect("identity optimization execution");
    CanonicalTerminalArtifact::from_parts(module, proof, &optimization, debug)
        .expect("canonical Terminal artifact")
}

use terminal_verifier::{
    ObligationEvidence, ProofBundle, RecursiveComponentEvidence,
    proof_recursive_component_identity, reconstruct_proof_recursive_component_obligations,
};

fn representative_bundle() -> ProofBundle {
    let equality = Proposition::Equal(
        ScalarTerm::value(value_id(2), ScalarType::Integer(i32_type())),
        ScalarTerm::value(value_id(1), ScalarType::Integer(i32_type())),
    );
    ProofBundle {
        recursive_components: Vec::new(),
        control_cycles: Vec::new(),
        evidence_producers: Vec::new(),
        evidence: vec![
            ObligationEvidence {
                obligation: obligation_id(1),
                route: EvidenceRoute::KernelDerived(PrimitiveJudgment::Truth),
            },
            ObligationEvidence {
                obligation: obligation_id(2),
                route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                    identity: evidence_id(3),
                    proof_system_marker: ProofSystemMarker::CURRENT,
                    proof: ProofNode {
                        conclusion: equality.clone(),
                        rule: ProofRule::EqualityTransitivity {
                            left_equals_middle: Box::new(ProofNode {
                                conclusion: equality.clone(),
                                rule: ProofRule::SemanticAxiom { index: 7 },
                            }),
                            middle_equals_right: Box::new(ProofNode {
                                conclusion: equality,
                                rule: ProofRule::Assumption { index: 5 },
                            }),
                        },
                    },
                }),
            },
            ObligationEvidence {
                obligation: obligation_id(3),
                route: EvidenceRoute::Admitted(AdmissionEvidence {
                    site: AdmissionSiteId::new(4).unwrap(),
                    kind: AdmissionKind::ProviderFact,
                    authority_identity: evidence_id(5),
                    evidence_identity: evidence_id(6),
                    profile_decision: ProfileDecisionId::new(7).unwrap(),
                }),
            },
        ],
    }
}

fn proof_recursive_component() -> TerminalProofRecursiveComponent {
    let contract = |raw| ContractId::new(raw).expect("nonzero proof contract identity");
    TerminalProofRecursiveComponent {
        ranking_relation: TerminalProofRankingRelation::StructuralSubterm,
        rank_type_identity: "package::Node".into(),
        types: vec![TerminalProofRecursiveType {
            identity: "package::Node".into(),
            fields: vec![
                TerminalProofRecursiveField {
                    identity: "package::Node::left".into(),
                    type_identity: "package::Node".into(),
                },
                TerminalProofRecursiveField {
                    identity: "package::Node::right".into(),
                    type_identity: "package::Node".into(),
                },
            ],
        }],
        members: vec![
            TerminalProofRecursiveMember {
                contract: contract(1001),
                machine_identity: "package::left".into(),
                rank_parameter_identity: "package::left::node".into(),
            },
            TerminalProofRecursiveMember {
                contract: contract(1002),
                machine_identity: "package::right".into(),
                rank_parameter_identity: "package::right::node".into(),
            },
        ],
        edges: vec![
            TerminalProofRecursiveEdge {
                caller: contract(1001),
                callee: contract(1002),
                site: TerminalProofRecursiveCallSite::Statement {
                    state_identity: "package::left::step".into(),
                    statement_index: 0,
                },
                strict_member_path: vec!["package::Node::left".into()],
            },
            TerminalProofRecursiveEdge {
                caller: contract(1001),
                callee: contract(1002),
                site: TerminalProofRecursiveCallSite::Statement {
                    state_identity: "package::left::step".into(),
                    statement_index: 1,
                },
                strict_member_path: vec!["package::Node::right".into()],
            },
            TerminalProofRecursiveEdge {
                caller: contract(1002),
                callee: contract(1001),
                site: TerminalProofRecursiveCallSite::Statement {
                    state_identity: "package::right::step".into(),
                    statement_index: 0,
                },
                strict_member_path: vec!["package::Node::left".into()],
            },
            TerminalProofRecursiveEdge {
                caller: contract(1002),
                callee: contract(1001),
                site: TerminalProofRecursiveCallSite::Statement {
                    state_identity: "package::right::step".into(),
                    statement_index: 1,
                },
                strict_member_path: vec!["package::Node::right".into()],
            },
        ],
    }
}

fn proof_recursive_evidence(module: &TerminalModule) -> Vec<RecursiveComponentEvidence> {
    let obligations = reconstruct_proof_recursive_component_obligations(module)
        .expect("canonical recursive component obligations");
    module
        .proof_recursive_components
        .iter()
        .zip(obligations)
        .enumerate()
        .map(|(component_index, (component, obligation))| {
            let route = |identity, obligation: &proof_admission::CertificateObligation| {
                EvidenceRoute::CertificateDerived(CertificateEnvelope {
                    identity: evidence_id(identity),
                    proof_system_marker: ProofSystemMarker::CURRENT,
                    proof: ProofNode {
                        conclusion: obligation.obligation.proposition.clone(),
                        rule: ProofRule::SemanticAxiom { index: 0 },
                    },
                })
            };
            let base = 9_000 + u64::try_from(component_index).unwrap() * 100;
            RecursiveComponentEvidence {
                component: proof_recursive_component_identity(component),
                certificate: RecursiveComponentCertificate {
                    identity: evidence_id(base),
                    ranking_relation: obligation.ranking_relation.expect("measured component"),
                    well_foundedness: route(base + 1, &obligation.well_foundedness),
                    edges: obligation
                        .edges
                        .iter()
                        .enumerate()
                        .map(|(edge_index, edge)| RecursiveEdgeCertificate {
                            obligation: edge.decrease.obligation.id,
                            evidence: route(
                                base + 2 + u64::try_from(edge_index).unwrap(),
                                &edge.decrease,
                            ),
                        })
                        .collect(),
                },
            }
        })
        .collect()
}

fn semantic_module() -> TerminalModule {
    let integer = i32_type();
    let scalar_type = ScalarType::Integer(integer);
    let literal = ScalarTerm::integer(integer, IntegerValue::Signed(7)).unwrap();
    let goal = Proposition::Equal(literal.clone(), literal);
    TerminalModule {
        scalar_qualifications: Default::default(),
        scalar_block_invariants: Vec::new(),
        operation_crash_contracts: Vec::new(),
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: machine_id(1),
        structural_types: Vec::new(),
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
        evidence_contract_lanes: Vec::new(),
        proof_output_calls: Vec::new(),
        proof_recursive_components: Vec::new(),
        closed_conformance_applications: Vec::new(),
        dynamic_dispatch: Default::default(),
        suspension_call_plan_count: 0,
        suspension_call_sites: Vec::new(),
        suspension_call_plans: Vec::new(),
        quotient_correspondences: Vec::new(),
        machines: vec![TerminalMachine {
            closed_reach_application: None,
            declared_service_reach: Vec::new(),
            id: machine_id(1),
            attachment: None,
            structural_parameters: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            parameters: Vec::new(),
            ranked_scc: None,
            result: TerminalMachineResult::Scalar(ValueDeclaration {
                qualifications: Default::default(),
                id: value_id(2),
                scalar_type,
            }),
            structural_places: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: block_id(1),
            blocks: vec![Block {
                erased_scalar_formals: Vec::new(),
                structural_parameters: Vec::new(),
                id: block_id(1),
                parameters: Vec::new(),
                operations: vec![Operation {
                    static_reach_binding: None,
                    id: operation_id(1),
                    result: terminal_psi::OperationResult::Scalar(ValueDeclaration {
                        qualifications: Default::default(),
                        id: value_id(1),
                        scalar_type,
                    }),
                    kind: OperationKind::IntegerConstant {
                        value: IntegerValue::Signed(7),
                    },
                }],
                terminator: Terminator::Return {
                    cleanup_actions: Vec::new(),
                    edge: edge_id(1),
                    value: value_id(1),
                },
            }],
            contract: MachineContract {
                erased_scalar_formals: Vec::new(),
                id: contract_id(1),
                crash_routes: Vec::new(),
                requires: vec![goal.clone()],
                ensures: vec![ContractClause {
                    obligation: obligation_id(1),
                    proposition: goal,
                }],
                outcome_specific_ensures: Vec::new(),
            },
        }],
    }
}

fn kernel_bundle() -> ProofBundle {
    ProofBundle {
        recursive_components: Vec::new(),
        control_cycles: Vec::new(),
        evidence_producers: Vec::new(),
        evidence: vec![ObligationEvidence {
            obligation: obligation_id(1),
            route: EvidenceRoute::KernelDerived(PrimitiveJudgment::ClosedIntegerRelation),
        }],
    }
}

fn certificate_bundle() -> ProofBundle {
    let integer = i32_type();
    let literal = ScalarTerm::integer(integer, IntegerValue::Signed(7)).unwrap();
    let goal = Proposition::Equal(literal.clone(), literal);
    ProofBundle {
        recursive_components: Vec::new(),
        control_cycles: Vec::new(),
        evidence_producers: Vec::new(),
        evidence: vec![ObligationEvidence {
            obligation: obligation_id(1),
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: evidence_id(9),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof: ProofNode {
                    conclusion: goal,
                    rule: ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation),
                },
            }),
        }],
    }
}

fn i32_type() -> IntegerType {
    IntegerType::new(IntegerSign::Signed, 32).unwrap()
}

macro_rules! id_constructor {
    ($function:ident, $type:ty) => {
        fn $function(raw: u64) -> $type {
            <$type>::new(raw).expect("test identities are nonzero")
        }
    };
}

id_constructor!(value_id, ValueId);
id_constructor!(place_id, PlaceId);
id_constructor!(structural_field_id, StructuralFieldId);
id_constructor!(structural_case_id, StructuralCaseId);
id_constructor!(structural_type_id, StructuralTypeId);
id_constructor!(machine_id, MachineId);
id_constructor!(block_id, BlockId);
id_constructor!(operation_id, OperationId);
id_constructor!(edge_id, EdgeId);
id_constructor!(contract_id, ContractId);
id_constructor!(obligation_id, ObligationId);
id_constructor!(evidence_id, EvidenceIdentity);
id_constructor!(proposition_id, PropositionId);
id_constructor!(evidence_term_id, EvidenceTermId);
