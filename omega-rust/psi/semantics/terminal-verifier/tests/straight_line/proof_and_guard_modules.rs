//! Proof-recursive, unit and payloadless guard fixtures.

use proof_admission::{
    CertificateEnvelope, EvidenceRoute, ProofNode, ProofRule, ProofSystemMarker,
    RecursiveComponentCertificate, RecursiveEdgeCertificate,
};
use semantic_vocabulary::{
    BlockId, ContractId, EdgeId, EvidenceIdentity, MachineId, OperationId, PlaceId, ScalarType,
    StructuralCaseId, StructuralPlaceKind, StructuralTypeId, ValueId,
};
use terminal_psi::{
    Block, MachineContract, Operation, OperationKind, OperationResult, StructuralCaseDeclaration,
    StructuralMultiplicity, StructuralOperationResult, StructuralPlaceDeclaration,
    StructuralResultDeclaration, StructuralTypeDeclaration, StructuralTypeShape, SuccessorEdge,
    TerminalMachine, TerminalMachineResult, TerminalModule, TerminalProofRankingRelation,
    TerminalProofRecursiveCallSite, TerminalProofRecursiveComponent, TerminalProofRecursiveEdge,
    TerminalProofRecursiveField, TerminalProofRecursiveMember, TerminalProofRecursiveType,
    Terminator, ValueDeclaration, VocabularyMarker,
};
use terminal_verifier::{
    ProofBundle, RecursiveComponentEvidence, proof_recursive_component_identity,
    reconstruct_proof_recursive_component_obligations,
};

pub(super) fn contract_id(raw: u64) -> ContractId {
    ContractId::new(raw).expect("nonzero contract identity")
}

pub(super) fn proof_recursive_module() -> TerminalModule {
    let mut module = unit_module();
    module.proof_recursive_components = vec![TerminalProofRecursiveComponent {
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
                contract: contract_id(1001),
                machine_identity: "package::left".into(),
                rank_parameter_identity: "package::left::node".into(),
            },
            TerminalProofRecursiveMember {
                contract: contract_id(1002),
                machine_identity: "package::right".into(),
                rank_parameter_identity: "package::right::node".into(),
            },
        ],
        edges: vec![
            TerminalProofRecursiveEdge {
                caller: contract_id(1001),
                callee: contract_id(1002),
                site: TerminalProofRecursiveCallSite::Statement {
                    state_identity: "package::left::step".into(),
                    statement_index: 0,
                },
                strict_member_path: vec!["package::Node::left".into()],
            },
            TerminalProofRecursiveEdge {
                caller: contract_id(1001),
                callee: contract_id(1002),
                site: TerminalProofRecursiveCallSite::Statement {
                    state_identity: "package::left::step".into(),
                    statement_index: 1,
                },
                strict_member_path: vec!["package::Node::right".into()],
            },
            TerminalProofRecursiveEdge {
                caller: contract_id(1002),
                callee: contract_id(1001),
                site: TerminalProofRecursiveCallSite::Statement {
                    state_identity: "package::right::step".into(),
                    statement_index: 0,
                },
                strict_member_path: vec!["package::Node::left".into()],
            },
            TerminalProofRecursiveEdge {
                caller: contract_id(1002),
                callee: contract_id(1001),
                site: TerminalProofRecursiveCallSite::Statement {
                    state_identity: "package::right::step".into(),
                    statement_index: 1,
                },
                strict_member_path: vec!["package::Node::right".into()],
            },
        ],
    }];
    module
}

pub(super) fn proof_recursive_bundle(module: &TerminalModule) -> ProofBundle {
    let [component] = module.proof_recursive_components.as_slice() else {
        panic!("one recursive component")
    };
    let mut reconstructed =
        reconstruct_proof_recursive_component_obligations(module).expect("canonical component");
    let obligation = reconstructed.pop().expect("one recursive obligation");
    let route = |identity, obligation: &proof_admission::CertificateObligation| {
        EvidenceRoute::CertificateDerived(CertificateEnvelope {
            identity: EvidenceIdentity::new(identity).expect("nonzero certificate identity"),
            proof_system_marker: ProofSystemMarker::CURRENT,
            proof: ProofNode {
                conclusion: obligation.obligation.proposition.clone(),
                rule: ProofRule::SemanticAxiom { index: 0 },
            },
        })
    };
    ProofBundle {
        evidence: Vec::new(),
        control_cycles: Vec::new(),
        recursive_components: vec![RecursiveComponentEvidence {
            component: proof_recursive_component_identity(component),
            certificate: RecursiveComponentCertificate {
                identity: EvidenceIdentity::new(2000).unwrap(),
                ranking_relation: obligation.ranking_relation.expect("measured component"),
                well_foundedness: route(2001, &obligation.well_foundedness),
                edges: obligation
                    .edges
                    .iter()
                    .enumerate()
                    .map(|(index, edge)| RecursiveEdgeCertificate {
                        obligation: edge.decrease.obligation.id,
                        evidence: route(
                            2010 + u64::try_from(index).expect("edge index fits u64"),
                            &edge.decrease,
                        ),
                    })
                    .collect(),
            },
        }],
        evidence_producers: Vec::new(),
    }
}

pub(super) fn unit_module() -> TerminalModule {
    TerminalModule {
        scalar_qualifications: Default::default(),
        scalar_block_invariants: Vec::new(),
        operation_crash_contracts: Vec::new(),
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: MachineId::new(900).unwrap(),
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
            id: MachineId::new(900).unwrap(),
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
            entry: BlockId::new(900).unwrap(),
            blocks: vec![Block {
                erased_scalar_formals: Vec::new(),
                structural_parameters: Vec::new(),
                id: BlockId::new(900).unwrap(),
                parameters: Vec::new(),
                operations: Vec::new(),
                terminator: Terminator::ReturnUnit {
                    edge: EdgeId::new(900).unwrap(),
                    trivial_affine_discards: Vec::new(),
                },
            }],
            contract: MachineContract {
                erased_scalar_formals: Vec::new(),
                id: ContractId::new(900).unwrap(),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        }],
    }
}

pub(super) fn payloadless_guard_module()
-> (TerminalModule, StructuralCaseId, StructuralCaseId, PlaceId) {
    let mut module = unit_module();
    let operation = OperationId::new(920).expect("operation");
    let operation_place = PlaceId::new(920).expect("operation place");
    let result_place = PlaceId::new(921).expect("result place");
    let structural_type = StructuralTypeId::new(920).expect("result type");
    let success = StructuralCaseId::new(920).expect("success case");
    let failure = StructuralCaseId::new(921).expect("failure case");
    module.structural_types = vec![StructuralTypeDeclaration {
        id: structural_type,
        identity: "Outcome".to_owned(),
        shape: StructuralTypeShape::Sum {
            cases: vec![
                StructuralCaseDeclaration {
                    id: success,
                    identity: "Success".to_owned(),
                    fields: Vec::new(),
                },
                StructuralCaseDeclaration {
                    id: failure,
                    identity: "Failure".to_owned(),
                    fields: Vec::new(),
                },
            ],
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
            kind: StructuralPlaceKind::OperationResult {
                producer: operation,
                structural_type,
            },
        },
        StructuralPlaceDeclaration {
            id: result_place,
            kind: StructuralPlaceKind::Result,
        },
    ];
    machine.blocks[0].operations = vec![Operation {
        static_reach_binding: None,
        id: operation,
        result: OperationResult::Structural(StructuralOperationResult {
            place: operation_place,
            structural_type,
            multiplicity: StructuralMultiplicity::Unrestricted,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            claims: Vec::new(),
        }),
        kind: OperationKind::EstablishScalarCase {
            result_case: success,
            fields: vec![],
        },
    }];
    machine.blocks[0].terminator = Terminator::ReturnStructural {
        edge: EdgeId::new(920).expect("return edge"),
        source: operation_place,
        returned_claims: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };
    (module, success, failure, result_place)
}

pub(super) fn multi_exit_payloadless_guard_module() -> (
    TerminalModule,
    StructuralCaseId,
    StructuralCaseId,
    StructuralCaseId,
    PlaceId,
) {
    let mut module = unit_module();
    let first = ValueId::new(930).expect("first condition");
    let second = ValueId::new(931).expect("second condition");
    let structural_type = StructuralTypeId::new(930).expect("result type");
    let success = StructuralCaseId::new(930).expect("success case");
    let failure = StructuralCaseId::new(931).expect("failure case");
    let absent = StructuralCaseId::new(932).expect("absent case");
    let success_one_place = PlaceId::new(930).expect("first Success place");
    let success_two_place = PlaceId::new(931).expect("second Success place");
    let failure_place = PlaceId::new(932).expect("Failure place");
    let result_place = PlaceId::new(933).expect("result place");
    let success_one_operation = OperationId::new(930).expect("first Success operation");
    let success_two_operation = OperationId::new(931).expect("second Success operation");
    let failure_operation = OperationId::new(932).expect("Failure operation");
    module.structural_types = vec![StructuralTypeDeclaration {
        id: structural_type,
        identity: "Outcome".to_owned(),
        shape: StructuralTypeShape::Sum {
            cases: vec![
                StructuralCaseDeclaration {
                    id: success,
                    identity: "Success".to_owned(),
                    fields: Vec::new(),
                },
                StructuralCaseDeclaration {
                    id: failure,
                    identity: "Failure".to_owned(),
                    fields: Vec::new(),
                },
                StructuralCaseDeclaration {
                    id: absent,
                    identity: "Absent".to_owned(),
                    fields: Vec::new(),
                },
            ],
        },
    }];
    let machine = &mut module.machines[0];
    machine.parameters = vec![
        ValueDeclaration {
            qualifications: Default::default(),
            id: first,
            scalar_type: ScalarType::Boolean,
        },
        ValueDeclaration {
            qualifications: Default::default(),
            id: second,
            scalar_type: ScalarType::Boolean,
        },
    ];
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
            id: success_one_place,
            kind: StructuralPlaceKind::OperationResult {
                producer: success_one_operation,
                structural_type,
            },
        },
        StructuralPlaceDeclaration {
            id: success_two_place,
            kind: StructuralPlaceKind::OperationResult {
                producer: success_two_operation,
                structural_type,
            },
        },
        StructuralPlaceDeclaration {
            id: failure_place,
            kind: StructuralPlaceKind::OperationResult {
                producer: failure_operation,
                structural_type,
            },
        },
        StructuralPlaceDeclaration {
            id: result_place,
            kind: StructuralPlaceKind::Result,
        },
    ];
    let return_block = |block_raw, operation, place, result_case, edge_raw| Block {
        erased_scalar_formals: Vec::new(),
        structural_parameters: Vec::new(),
        id: BlockId::new(block_raw).unwrap(),
        parameters: Vec::new(),
        operations: vec![Operation {
            static_reach_binding: None,
            id: operation,
            result: OperationResult::Structural(StructuralOperationResult {
                place,
                structural_type,
                multiplicity: StructuralMultiplicity::Unrestricted,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                claims: Vec::new(),
            }),
            kind: OperationKind::EstablishScalarCase {
                result_case,
                fields: vec![],
            },
        }],
        terminator: Terminator::ReturnStructural {
            edge: EdgeId::new(edge_raw).unwrap(),
            source: place,
            returned_claims: Vec::new(),
            trivial_affine_discards: Vec::new(),
        },
    };
    machine.entry = BlockId::new(930).expect("entry block");
    machine.blocks = vec![
        Block {
            erased_scalar_formals: Vec::new(),
            structural_parameters: Vec::new(),
            id: BlockId::new(930).unwrap(),
            parameters: Vec::new(),
            operations: Vec::new(),
            terminator: Terminator::Conditional {
                condition: first,
                when_true: SuccessorEdge {
                    structural_arguments: Vec::new(),
                    edge: EdgeId::new(930).unwrap(),
                    target: BlockId::new(931).unwrap(),
                    arguments: Vec::new(),
                    erased_arguments: Vec::new(),
                    trivial_affine_discards: Vec::new(),
                },
                when_false: SuccessorEdge {
                    structural_arguments: Vec::new(),
                    edge: EdgeId::new(931).unwrap(),
                    target: BlockId::new(934).unwrap(),
                    arguments: Vec::new(),
                    erased_arguments: Vec::new(),
                    trivial_affine_discards: Vec::new(),
                },
            },
        },
        Block {
            erased_scalar_formals: Vec::new(),
            structural_parameters: Vec::new(),
            id: BlockId::new(931).unwrap(),
            parameters: Vec::new(),
            operations: Vec::new(),
            terminator: Terminator::Conditional {
                condition: second,
                when_true: SuccessorEdge {
                    structural_arguments: Vec::new(),
                    edge: EdgeId::new(932).unwrap(),
                    target: BlockId::new(932).unwrap(),
                    arguments: Vec::new(),
                    erased_arguments: Vec::new(),
                    trivial_affine_discards: Vec::new(),
                },
                when_false: SuccessorEdge {
                    structural_arguments: Vec::new(),
                    edge: EdgeId::new(933).unwrap(),
                    target: BlockId::new(933).unwrap(),
                    arguments: Vec::new(),
                    erased_arguments: Vec::new(),
                    trivial_affine_discards: Vec::new(),
                },
            },
        },
        return_block(932, success_one_operation, success_one_place, success, 934),
        return_block(933, success_two_operation, success_two_place, success, 935),
        return_block(934, failure_operation, failure_place, failure, 936),
    ];
    (module, success, failure, absent, result_place)
}
