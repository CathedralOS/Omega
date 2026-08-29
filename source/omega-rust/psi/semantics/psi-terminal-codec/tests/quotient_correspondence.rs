use psi_core::{BlockId, ContractId, EdgeId, MachineId};
use psi_language_semantics::quotient_correspondence::{
    CanonicalQuotientCorrespondence, QuotientCallableIdentity, QuotientContractFactCoordinate,
    QuotientContractOwner, QuotientCorrespondenceOperationKind, QuotientCrashCertificate,
    QuotientDefineRuntimePosition, QuotientDirectResultFlow, QuotientMachineApplication,
    QuotientPositionalRelation, QuotientPurityCertificate, QuotientRelationIdentity,
    QuotientRepresentativeApplication, QuotientRepresentativeEligibility,
    QuotientStaticApplication, QuotientTerminationCertificate, QuotientTheoremConclusion,
    QuotientTheoremCorrespondence, QuotientTheoremEligibility, QuotientTheoremParameter,
    QuotientTheoremParameterRole, QuotientTheoremRelationPremise,
};
use psi_terminal::{
    Block, MachineContract, RetainedQuotientCorrespondence, TerminalMachine, TerminalMachineResult,
    TerminalModule, Terminator, VocabularyMarker, retain_non_executable_quotient_correspondence,
};
use psi_terminal_codec::{CodecError, decode_module, encode_module, semantic_fingerprint};
use psi_terminal_verifier::{
    ModuleError, QuotientCorrespondenceReplayError, validate_module_representation,
};

fn callable(name: &str) -> QuotientCallableIdentity {
    QuotientCallableIdentity {
        declaration: format!("package:{}::{name}", "01".repeat(32)),
        overload: format!("named-callable:{name}"),
    }
}

fn relation(name: &str) -> QuotientRelationIdentity {
    QuotientRelationIdentity {
        quotient_declaration: format!("package:{}::{name}", "02".repeat(32)),
        quotient_type: format!("package:{}::{name}", "02".repeat(32)),
        carrier_type: format!("package:{}::{name}Carrier", "03".repeat(32)),
        relation: format!("package:{}::{name}Relation", "04".repeat(32)),
    }
}

fn coordinate(fact_position: u32) -> QuotientContractFactCoordinate {
    QuotientContractFactCoordinate {
        owner: QuotientContractOwner::State,
        contract_position: 0,
        fact_position,
    }
}

fn correspondence(owner: &str) -> RetainedQuotientCorrespondence {
    let input = relation("Value");
    let result = relation("Result");
    retain_non_executable_quotient_correspondence(CanonicalQuotientCorrespondence {
        operation_kind: QuotientCorrespondenceOperationKind::Define,
        public_operation: callable(owner),
        representative: QuotientMachineApplication {
            callable: callable("Carrier::apply"),
            static_application: QuotientStaticApplication { bindings: vec![] },
        },
        selected_theorem: QuotientMachineApplication {
            callable: callable("apply_respects"),
            static_application: QuotientStaticApplication { bindings: vec![] },
        },
        input_relations: vec![QuotientPositionalRelation::Quotient(input.clone())],
        result_relation: result.clone(),
        runtime_positions: vec![QuotientDefineRuntimePosition {
            public_position: 0,
            representative_position: 0,
        }],
        theorem: QuotientTheoremCorrespondence {
            parameters: vec![
                QuotientTheoremParameter {
                    theorem_position: 0,
                    role: QuotientTheoremParameterRole::QuotientLeft { input_position: 0 },
                },
                QuotientTheoremParameter {
                    theorem_position: 1,
                    role: QuotientTheoremParameterRole::QuotientRight { input_position: 0 },
                },
            ],
            relation_premises: vec![QuotientTheoremRelationPremise {
                expected_position: 0,
                actual: coordinate(0),
                relation: input.relation,
                left_parameter: 0,
                right_parameter: 1,
            }],
            legality_premises: vec![],
            conclusion: QuotientTheoremConclusion {
                actual: coordinate(1),
                relation: result.relation,
                left: QuotientRepresentativeApplication { arguments: vec![0] },
                right: QuotientRepresentativeApplication { arguments: vec![1] },
            },
        },
        representative_eligibility: QuotientRepresentativeEligibility {
            purity: QuotientPurityCertificate::PureClosure,
            termination: QuotientTerminationCertificate::Unconditional,
        },
        theorem_eligibility: QuotientTheoremEligibility {
            purity: QuotientPurityCertificate::PureClosure,
            termination: QuotientTerminationCertificate::Unconditional,
            crash: QuotientCrashCertificate::CrashFree,
        },
        result_flow: QuotientDirectResultFlow {
            state_position: 0,
            statement_position: 0,
        },
    })
}

fn module_with(quotient_correspondences: Vec<RetainedQuotientCorrespondence>) -> TerminalModule {
    let machine = MachineId::new(1).unwrap();
    TerminalModule {
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: machine,
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
        quotient_correspondences,
        machines: vec![TerminalMachine {
            id: machine,
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
            entry: BlockId::new(1).unwrap(),
            blocks: vec![Block {
                id: BlockId::new(1).unwrap(),
                parameters: Vec::new(),
                operations: Vec::new(),
                terminator: Terminator::ReturnUnit {
                    edge: EdgeId::new(1).unwrap(),
                    trivial_affine_discards: Vec::new(),
                },
            }],
            contract: MachineContract {
                id: ContractId::new(1).unwrap(),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        }],
    }
}

#[test]
fn quotient_correspondence_round_trips_and_enters_module_identity() {
    let module = module_with(vec![correspondence("Public::apply")]);
    validate_module_representation(&module).expect("representation replay");
    let bytes = encode_module(&module).expect("quotient correspondence encodes");
    assert_eq!(&bytes[8..10], 36_u16.to_le_bytes());
    assert_eq!(
        &bytes[10..12],
        psi_terminal::VocabularyMarker::CURRENT.get().to_le_bytes()
    );
    assert_eq!(decode_module(&bytes), Ok(module.clone()));
    assert_ne!(
        semantic_fingerprint(&module).unwrap(),
        semantic_fingerprint(&module_with(Vec::new())).unwrap()
    );
}

#[test]
fn encoding_rejects_tampered_retained_identity() {
    let mut module = module_with(vec![correspondence("Public::apply")]);
    module.quotient_correspondences[0].identity.0.push(0);
    assert!(matches!(
        encode_module(&module),
        Err(CodecError::InvalidModule(
            ModuleError::InvalidQuotientCorrespondence {
                error: QuotientCorrespondenceReplayError::IdentityMismatch { .. },
                ..
            }
        ))
    ));
}

#[test]
fn encoding_rejects_noncanonical_quotient_correspondence_order() {
    let mut rows = vec![
        correspondence("Public::apply"),
        correspondence("Public::other"),
    ];
    rows.sort_by(|left, right| left.identity.cmp(&right.identity));
    rows.reverse();
    assert_eq!(
        encode_module(&module_with(rows)),
        Err(CodecError::NonCanonicalOrder(
            "quotient correspondences by canonical identity"
        ))
    );
}
